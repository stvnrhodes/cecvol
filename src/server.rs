use crate::action::devices::FulfillmentRequest;
use crate::action::fulfillment::FulfillmentHandler;
use crate::oauth::OAuthServer;
use log::{error, info, warn};
use serde::Serialize;
use serde_json::json;
use std::io::{Cursor, Read};
use std::sync::Arc;
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

const MAX_OAUTH_BODY_BYTES: usize = 16 * 1024;
const MAX_FULFILLMENT_BODY_BYTES: usize = 64 * 1024;

#[derive(Clone)]
pub struct AppState {
    pub fulfillment: FulfillmentHandler,
    pub oauth: Option<Arc<OAuthServer>>,
}

enum BodyReadError {
    TooLarge,
    Invalid(String),
}

fn read_limited_body(request: &mut Request, max_bytes: usize) -> Result<String, BodyReadError> {
    let mut buf = Vec::new();
    request
        .as_reader()
        .take((max_bytes + 1) as u64)
        .read_to_end(&mut buf)
        .map_err(|e| BodyReadError::Invalid(e.to_string()))?;
    if buf.len() > max_bytes {
        return Err(BodyReadError::TooLarge);
    }
    String::from_utf8(buf).map_err(|e| BodyReadError::Invalid(e.to_string()))
}

fn index() -> Response<Cursor<Vec<u8>>> {
    Response::from_string(include_str!("index.html"))
        .with_header(Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap())
}

fn manifest() -> Response<Cursor<Vec<u8>>> {
    Response::from_data(include_str!("manifest.json").as_bytes().to_vec())
        .with_header(Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap())
}

fn json_response<T: Serialize>(data: &T) -> Response<Cursor<Vec<u8>>> {
    let s = serde_json::to_string(data).unwrap();
    Response::from_string(s)
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
}

fn header_value<'a>(request: &'a Request, name: &str) -> Option<&'a str> {
    request
        .headers()
        .iter()
        .find(|h| h.field.as_str().as_str().eq_ignore_ascii_case(name))
        .map(|h| h.value.as_str())
}

fn fulfillment(
    app_state: &AppState,
    request: &mut Request,
    agent_user_id: &str,
) -> Response<Cursor<Vec<u8>>> {
    let content = match read_limited_body(request, MAX_FULFILLMENT_BODY_BYTES) {
        Ok(s) => s,
        Err(BodyReadError::TooLarge) => {
            warn!("Rejected oversized fulfillment request body");
            return Response::from_string("Payload Too Large").with_status_code(StatusCode(413));
        }
        Err(BodyReadError::Invalid(e)) => {
            error!("Failed to read request body: {}", e);
            return Response::from_string(e).with_status_code(StatusCode(400));
        }
    };

    let req: FulfillmentRequest = match serde_json::from_str(&content) {
        Ok(r) => r,
        Err(e) => {
            let snippet: String = content.chars().take(512).collect();
            error!("Failed to parse JSON: {} (body={})", e, snippet);
            return Response::from_string(e.to_string()).with_status_code(StatusCode(400));
        }
    };

    let response = app_state.fulfillment.handle(&req, agent_user_id);
    json_response(&response)
}

fn oauth_authorize(app_state: &AppState, request: &Request) -> Response<Cursor<Vec<u8>>> {
    let Some(oauth) = &app_state.oauth else {
        return json_response(&json!({
            "error": "server_error",
            "error_description": "OAuth is not configured"
        }))
        .with_status_code(StatusCode(503));
    };

    let Some(user) = header_value(request, "Remote-Email")
        .or_else(|| header_value(request, "Remote-User"))
        .filter(|s| !s.trim().is_empty())
    else {
        warn!("Rejected /oauth/authorize without Remote-Email or Remote-User header");
        return Response::from_string("Unauthorized").with_status_code(StatusCode(401));
    };

    let query = request
        .url()
        .split_once('?')
        .map(|(_, q)| q)
        .unwrap_or("");

    match oauth.authorize(query, user.trim()) {
        Ok(location) => {
            info!("Issued OAuth authorization code for user {}", user.trim());
            Response::from_data(Vec::new())
                .with_status_code(StatusCode(302))
                .with_header(Header::from_bytes("Location", location).unwrap())
                .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
        }
        Err(e) => {
            warn!(
                "OAuth authorize error ({}): {}",
                e.error_code(),
                e.description()
            );
            json_response(&json!({
                "error": e.error_code(),
                "error_description": e.description()
            }))
            .with_status_code(StatusCode(400))
        }
    }
}

fn oauth_token(app_state: &AppState, request: &mut Request) -> Response<Cursor<Vec<u8>>> {
    let Some(oauth) = &app_state.oauth else {
        return json_response(&json!({
            "error": "server_error",
            "error_description": "OAuth is not configured"
        }))
        .with_status_code(StatusCode(503));
    };

    let auth_header = header_value(request, "Authorization").map(str::to_string);
    let body = match read_limited_body(request, MAX_OAUTH_BODY_BYTES) {
        Ok(s) => s,
        Err(BodyReadError::TooLarge) => {
            warn!("Rejected oversized /oauth/token request body");
            return json_response(&json!({
                "error": "invalid_request",
                "error_description": "request body too large"
            }))
            .with_status_code(StatusCode(413));
        }
        Err(BodyReadError::Invalid(e)) => {
            error!("Failed to read OAuth token request body: {}", e);
            return json_response(&json!({
                "error": "invalid_request",
                "error_description": "failed to read body"
            }))
            .with_status_code(StatusCode(400));
        }
    };

    match oauth.token(&body, auth_header.as_deref()) {
        Ok(tok) => json_response(&tok)
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
            .with_header(Header::from_bytes("Pragma", "no-cache").unwrap()),
        Err(e) => {
            warn!(
                "OAuth token error ({}): {}",
                e.error_code(),
                e.description()
            );
            json_response(&json!({
                "error": e.error_code(),
                "error_description": e.description()
            }))
            .with_status_code(StatusCode(400))
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
        }
    }
}

fn google_fulfillment(app_state: &AppState, request: &mut Request) -> Response<Cursor<Vec<u8>>> {
    let Some(oauth) = &app_state.oauth else {
        return Response::from_string("OAuth is not configured").with_status_code(StatusCode(503));
    };

    let auth_header = header_value(request, "Authorization").unwrap_or("");
    let agent_user_id = match oauth.verify_bearer(auth_header) {
        Ok(sub) => sub,
        Err(e) => {
            warn!("Rejected /google/fulfillment: {}", e.description());
            return Response::from_string("Unauthorized")
                .with_status_code(StatusCode(401))
                .with_header(
                    Header::from_bytes("WWW-Authenticate", "Bearer error=\"invalid_token\"")
                        .unwrap(),
                );
        }
    };

    fulfillment(app_state, request, &agent_user_id)
}

fn varz() -> Response<Cursor<Vec<u8>>> {
    let metrics = prometheus::gather();
    let encoder = prometheus::TextEncoder::new();
    match encoder.encode_to_string(&metrics) {
        Ok(encoded) => Response::from_string(encoded),
        Err(err) => Response::from_string(err.to_string()).with_status_code(StatusCode(500)),
    }
}

fn route(app_state: &AppState, req: &mut Request) -> Response<Cursor<Vec<u8>>> {
    let url = req.url().to_string();
    let path = url.split('?').next().unwrap_or(&url);

    match (req.method(), path) {
        (Method::Get, "/") => index(),
        (Method::Get, "/manifest.json") => manifest(),
        (Method::Get, "/varz") => varz(),
        (Method::Post, "/fulfillment") => {
            let user = header_value(req, "Remote-Email")
                .or_else(|| header_value(req, "Remote-User"))
                .unwrap_or("cecvol-stvn-user")
                .to_string();
            fulfillment(app_state, req, &user)
        }
        (Method::Get, "/oauth/authorize") => oauth_authorize(app_state, req),
        (Method::Post, "/oauth/token") => oauth_token(app_state, req),
        (Method::Post, "/google/fulfillment") => google_fulfillment(app_state, req),
        _ => Response::from_data(Vec::new()).with_status_code(StatusCode(404)),
    }
}

pub fn run(http_addr: &str, app_state: AppState) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting server...");
    let server = Server::http(http_addr).map_err(|e| format!("failed to bind {http_addr}: {e}"))?;

    for mut request in server.incoming_requests() {
        let log_path = request.url().split('?').next().unwrap_or(request.url());
        info!("{method} {path}", method = request.method(), path = log_path);

        let app_state = app_state.clone();
        std::thread::spawn(move || {
            let resp = route(&app_state, &mut request);
            info!("... {status}", status = resp.status_code().0);
            let _ = request.respond(resp);
        });
    }

    Ok(())
}
