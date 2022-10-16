mod jwt;

use axum::extract;
use axum::http::header;
use axum::http::Request;
use axum::http::StatusCode;
use axum::middleware;
use axum::response;
use axum::response::Html;
use axum::response::IntoResponse;
use axum::response::Response;
use cookie::Cookie;
use jwt::{Algorithm, Payload};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use time::OffsetDateTime;

const AUTH_COOKIE_NAME: &str = "auth";

// TODO(stvn): These should all be config, not hardcoded
const HMAC_SECRET: &str = "WxAkpsafDoqXXZc7z4REpEfTaaQ1vIYt19";
const CLIENT_ID: &str = "google";
const CLIENT_SECRET: &str = "super-duper-secure-secret";
const ISSUER_NAME: &str = "cec.stevenandbonnie.com";
const GLOBAL_PASSWORD: &str = "cecpassword";
const REDIRECTS: [&str; 2] = [
    "https://oauth-redirect.googleusercontent.com/r/cecvol-f4044",
    "https://oauth-redirect-sandbox.googleusercontent.com/r/cecvol-f4044",
];

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct AuthInfo {
    // The Google client ID you registered with Google.
    client_id: String,
    // The URL to which you send the response to this request.
    redirect_uri: String,
    // A bookkeeping value that is passed back to Google unchanged in the redirect URI.
    state: String,
    // A space-delimited set of scope strings that specify the data Google is requesting authorization for.
    scope: Option<String>,
    // The type of value to return in the response. For the OAuth 2.0 authorization code flow, the response type is always code.
    response_type: String,
}

pub async fn auth(info: extract::Query<AuthInfo>) -> response::Result<impl IntoResponse> {
    if info.client_id != CLIENT_ID {
        return Err((StatusCode::BAD_REQUEST, "Bad client id").into());
    }
    if !REDIRECTS.contains(&info.redirect_uri.as_str()) {
        return Err((StatusCode::BAD_REQUEST, "Bad redirect id").into());
    }

    let now = time::OffsetDateTime::now_utc();
    let expiration = now + Duration::from_secs(10 * 60);
    let payload: String = Payload::new()
        .with_issuer(ISSUER_NAME.into())
        .with_not_before(now)?
        .with_expiration(expiration)?
        .with_issued_at(now)?
        .to_token(Algorithm::HS256, HMAC_SECRET)?;

    Ok((
        StatusCode::FOUND,
        [(
            header::LOCATION,
            format!(
                "{}?code={}&state={}",
                info.redirect_uri, payload, info.state
            ),
        )],
    ))
}

pub async fn login_page() -> impl IntoResponse {
    Html(include_str!("auth.html"))
}

#[derive(Deserialize)]
pub struct AuthFormData {
    password: String,
}
#[derive(Deserialize)]
pub struct AuthQueryString {
    redirect: Option<String>,
}

pub async fn login(
    data: extract::Form<AuthFormData>,
    qs: extract::Query<AuthQueryString>,
) -> response::Result<impl IntoResponse> {
    if data.password != GLOBAL_PASSWORD {
        return Err((StatusCode::UNAUTHORIZED, "Bad password").into());
    }

    let now = time::OffsetDateTime::now_utc();
    let expiration = now + Duration::from_secs(24 * 7 * 60 * 60);
    let payload: String = Payload::new()
        // TODO(stvn): Put into config
        .with_issuer(ISSUER_NAME.into())
        .with_not_before(now)?
        .with_expiration(expiration)?
        .with_issued_at(now)?
        .to_token(Algorithm::HS256, HMAC_SECRET)?;

    Ok((
        StatusCode::FOUND,
        [
            (
                header::LOCATION,
                qs.redirect.as_ref().unwrap_or(&"/".to_string()).clone(),
            ),
            (
                header::SET_COOKIE,
                Cookie::build(AUTH_COOKIE_NAME, payload)
                    .expires(expiration)
                    .path("/")
                    // TODO(stvn): Configure this
                    //.secure(true)
                    .http_only(true)
                    .finish()
                    .to_string(),
            ),
        ],
    ))
}

pub async fn has_valid_auth<B>(
    req: Request<B>,
    next: middleware::Next<B>,
) -> response::Result<Response> {
    if let Some(header) = req.headers().get("Authorization") {
        let payload = Payload::from_token(
            header
                .to_str()
                .unwrap_or("")
                .strip_prefix("Bearer ")
                .unwrap_or(""),
            HMAC_SECRET,
        )?;
        if payload.valid_at(OffsetDateTime::now_utc())? {
            return Ok(next.run(req).await);
        }
    }
    for c in req
        .headers()
        .get_all(header::COOKIE)
        .iter()
        .flat_map(|value| value.to_str().unwrap_or("").split(';'))
    {
        if let Ok(cookie) = Cookie::parse(c) {
            if cookie.name() == AUTH_COOKIE_NAME {
                let payload = Payload::from_token(cookie.value(), HMAC_SECRET)?;
                if payload.valid_at(OffsetDateTime::now_utc())? {
                    return Ok(next.run(req).await);
                }
            }
        }
    }
    Ok((
        StatusCode::FOUND,
        [(
            header::LOCATION,
            format!(
                "/login?redirect={}",
                utf8_percent_encode(req.uri().path(), NON_ALPHANUMERIC)
            ),
        )],
    )
        .into_response())
}

#[derive(Deserialize)]
#[serde(tag = "grant_type", rename_all = "snake_case")]
enum GrantType {
    AuthorizationCode {
        // When grant_type=authorization_code, this parameter is the code Google received from either your sign-in or token exchange endpoint.
        code: String,
        // When grant_type=authorization_code, this parameter is the URL used in the initial authorization request.
        redirect_uri: String,
    },
    RefreshToken {
        // When grant_type=refresh_token, this parameter is the refresh token Google received from your token exchange endpoint.
        refresh_token: String,
    },
}
#[derive(Deserialize)]
pub struct TokenInfo {
    // A string that identifies the request origin as Google. This string must be registered within your system as Google's unique identifier.
    client_id: String,
    // A secret string that you registered with Google for your service.
    client_secret: String,
    // The type of token being exchanged. It's either authorization_code or refresh_token.
    #[serde(flatten)]
    grant_type: GrantType,
}

pub async fn token(info: extract::Form<TokenInfo>) -> response::Result<impl IntoResponse> {
    // https://developers.google.com/assistant/smarthome/develop/implement-oauth#implement_oauth_account_linking
    if info.client_id != CLIENT_ID {
        return Err((StatusCode::BAD_REQUEST, "Bad client id").into());
    }
    if info.client_secret != CLIENT_SECRET {
        return Err((StatusCode::BAD_REQUEST, "Bad client id").into());
    }
    let now = OffsetDateTime::now_utc();

    match &info.grant_type {
        GrantType::AuthorizationCode { code, redirect_uri } => {
            if !REDIRECTS.contains(&redirect_uri.as_str()) {
                return Err((StatusCode::BAD_REQUEST, "Bad redirect id").into());
            }

            let payload = Payload::from_token(&code, HMAC_SECRET)?;
            if !payload.valid_at(now)? {
                return Err((
                    StatusCode::BAD_REQUEST,
                    response::Json(json!({"error":"invalid_grant"})),
                )
                    .into());
            }

            let access_token: String = Payload::new()
                .with_issuer(ISSUER_NAME.into())
                .with_not_before(now)?
                .with_expiration(now + Duration::from_secs(3600))?
                .with_issued_at(now)?
                .to_token(Algorithm::HS256, HMAC_SECRET)?;

            let refresh_token: String = Payload::new()
                .with_issuer(ISSUER_NAME.into())
                .with_not_before(now)?
                .with_issued_at(now)?
                .with_expiration(now + Duration::from_secs(3600 * 24 * 7 * 365 * 10))?
                .to_token(Algorithm::HS256, HMAC_SECRET)?;

            Ok(response::Json(json!({
                "token_type": "Bearer",
                "access_token": access_token,
                "refresh_token":refresh_token,
                "expires_in":3600,
            })))
        }
        GrantType::RefreshToken { refresh_token } => {
            let payload = Payload::from_token(&refresh_token, HMAC_SECRET)?;
            if !payload.valid_at(now)? {
                return Err((
                    StatusCode::BAD_REQUEST,
                    response::Json(json!({"error":"invalid_grant"})),
                )
                    .into());
            }

            let access_token: String = Payload::new()
                .with_issuer(ISSUER_NAME.into())
                .with_not_before(now)?
                .with_expiration(now + Duration::from_secs(3600))?
                .with_issued_at(now)?
                .to_token(Algorithm::HS256, HMAC_SECRET)?;

            Ok(response::Json(json!({
                "token_type": "Bearer",
                "access_token": access_token,
                "expires_in":3600,
            })))
        }
    }
}
