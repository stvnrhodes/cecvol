mod jwt;

use actix_web::{error, get, web, Responder};
use serde::Deserialize;

const ENGLISH_LOCALE: &str = "en";

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
async fn auth(info: web::Query<AuthInfo>) -> impl Responder {
    if info.user_locale != ENGLISH_LOCALE {
        return Err(error::ErrorBadRequest("Non-english locale"));
    }
    // https://developers.google.com/assistant/smarthome/develop/implement-oauth#implement_oauth_account_linking
    // Todo: verify client id and redirect uri is correct
    // check that user is signed in
    // generate auth code, expire after 10m
    //redirect browser to redirect_uri, include code and state
    Ok("Unimplemented")
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

// maybe just generate gibberish, store as file on disk?
