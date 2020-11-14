mod jwt;

use actix_web::{
    error, get, http, post, web, HttpMessage, HttpRequest, HttpResponse, Responder, Result,
};
use jwt::{Algorithm, Payload};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::Deserialize;
use std::time::{Duration, SystemTime};

const ENGLISH_LOCALE: &str = "en";
const AUTH_COOKIE_NAME: &str = "auth";

// TODO(stvn): These should all be config, not hardcoded
const HMAC_SECRET: &str = "WxAkpsafDoqXXZc7z4REpEfTaaQ1vIYt19";
const CLIENT_ID: &str = "google";
const CLIENT_SECRET: &str = "Tuq3Cw1iszftc50";
const ISSUER_NAME: &str = "cec.stevenandbonnie.com";
const GLOBAL_PASSWORD: &str = "cecpassword";
const _PROJECT_ID: &str = "cecvol-f4044";
const REDIRECTS: [&str; 2] = [
    "https://oauth-redirect.googleusercontent.com/r/cecvol-f4044",
    "https://oauth-redirect-sandbox.googleusercontent.com/r/cecvol-f4044",
];

#[derive(Deserialize)]
#[allow(dead_code)]
struct AuthInfo {
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
    // The Google Account language setting in RFC5646 format, used to localize your content in the user's preferred language.
    user_locale: String,
}

#[get("/auth")]
async fn auth(req: HttpRequest, info: web::Query<AuthInfo>) -> impl Responder {
    if info.user_locale != ENGLISH_LOCALE {
        return Err(error::ErrorBadRequest("Non-english locale"));
    }
    if info.client_id != CLIENT_ID {
        return Err(error::ErrorBadRequest("Bad client id"));
    }
    if !has_valid_auth(&req).unwrap_or(false) {
        return Ok(HttpResponse::Found()
            .header(
                "Location",
                format!(
                    "/login?redirect={}",
                    utf8_percent_encode(req.path(), NON_ALPHANUMERIC)
                ),
            )
            .finish());
    }
    if !REDIRECTS.contains(&info.redirect_uri.as_str()) {
        return Err(error::ErrorBadRequest("Bad redirect id"));
    }

    let now = SystemTime::now();
    let expiration = now + Duration::from_secs(10 * 60);
    let payload: String = Payload::new()
        .with_issuer(ISSUER_NAME.into())
        .with_not_before(now)?
        .with_expiration(expiration)?
        .with_issued_at(now)?
        .to_token(Algorithm::HS256, HMAC_SECRET)?;

    Ok(HttpResponse::Found()
        .header(
            "Location",
            format!(
                "{}?code={}&state={}",
                info.redirect_uri, payload, info.state
            ),
        )
        .finish())
}

#[get("/login")]
async fn login_page() -> impl Responder {
    let resp: &'static [u8] = include_bytes!("auth.html");
    HttpResponse::Ok().content_type("text/html").body(resp)
}

#[derive(Deserialize)]
struct AuthFormData {
    password: String,
}
#[derive(Deserialize)]
struct AuthQueryString {
    redirect: Option<String>,
}

#[post("/login")]
async fn login(
    data: web::Form<AuthFormData>,
    qs: web::Query<AuthQueryString>,
) -> Result<HttpResponse> {
    // TODO(stvn): Put into config
    if data.password != GLOBAL_PASSWORD {
        return Ok(HttpResponse::Unauthorized().body("Bad password"));
    }

    let now = SystemTime::now();
    let expiration = now + Duration::from_secs(24 * 7 * 60 * 60);
    let payload: String = Payload::new()
        // TODO(stvn): Put into config
        .with_issuer(ISSUER_NAME.into())
        .with_not_before(now)?
        .with_expiration(expiration)?
        .with_issued_at(now)?
        .to_token(Algorithm::HS256, HMAC_SECRET)?;

    Ok(HttpResponse::Found()
        .header(
            "Location",
            qs.redirect.as_ref().unwrap_or(&"/".to_string()).clone(),
        )
        .cookie(
            http::Cookie::build(AUTH_COOKIE_NAME, payload)
                .expires(expiration.into())
                .path("/")
                // TODO(stvn): Configure this
                //.secure(true)
                .http_only(true)
                .finish(),
        )
        .finish())
}

// TODO(stvn): Turn into middleware
pub fn has_valid_auth(req: &HttpRequest) -> Result<bool, jwt::Error> {
    if let Some(header) = req.headers().get("Authorization") {
        let payload = Payload::from_token(
            header
                .to_str()
                .unwrap_or("")
                .strip_prefix("Bearer ")
                .unwrap_or(""),
            HMAC_SECRET,
        )?;
        return payload.valid_at(SystemTime::now());
    }
    if let Some(cookie) = req.cookie("auth") {
        let payload = Payload::from_token(cookie.value(), HMAC_SECRET)?;
        return payload.valid_at(SystemTime::now());
    }
    Ok(false)
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum GrantType {
    AuthorizationCode,
    RefreshToken,
}
#[derive(Deserialize)]
#[allow(dead_code)]
struct TokenInfo {
    // A string that identifies the request origin as Google. This string must be registered within your system as Google's unique identifier.
    client_id: String,
    // A secret string that you registered with Google for your service.
    client_secret: String,
    // The type of token being exchanged. It's either authorization_code or refresh_token.
    grant_type: GrantType,
    // When grant_type=authorization_code, this parameter is the code Google received from either your sign-in or token exchange endpoint.
    code: String,
    // When grant_type=authorization_code, this parameter is the URL used in the initial authorization request.
    redirect_uri: String,
    // When grant_type=refresh_token, this parameter is the refresh token Google received from your token exchange endpoint.
    refresh_token: String,
}

#[allow(dead_code)]
struct Token {
    token_type: String,
    access_token: String,
    expires_in: u32,
}

#[get("/token")]
async fn token(info: web::Query<AuthInfo>) -> impl Responder {
    if info.user_locale != ENGLISH_LOCALE {
        return Err(error::ErrorBadRequest("Non-english locale"));
    }
    // https://developers.google.com/assistant/smarthome/develop/implement-oauth#implement_oauth_account_linking
    // Todo: Verify that the client_id identifies the request origin as an authorized origin, and that the client_secret matches the expected value.
    // Verify that the authorization code is valid and not expired, and that the client ID specified in the request matches the client ID associated with the authorization code.
    // If you can't verify all of the above criteria, return an HTTP 400 Bad Request error with {"error": "invalid_grant"} as the body.
    // Otherwise, use the user ID from the refresh token to generate an access token. These tokens can be any string value, but they must uniquely represent the user and the client the token is for, and they must not be guessable. For access tokens, also record the expiration time of the token, typically an hour after you issue the token.
    Ok("Unimplemented")
}
