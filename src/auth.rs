use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;

use base64::engine::general_purpose::URL_SAFE;
use base64::Engine;
use log::info;
use rand::distributions::Alphanumeric;
use rand::distributions::DistString;
use rouille::input;
use rouille::Request;
use rouille::Response;
use serde::Deserialize;
use serde::Serialize;

const SESSION_COOKIE_NAME: &str = "session-id";

#[derive(Serialize)]
struct GoogleAuthParams<'a> {
    response_type: &'a str,
    client_id: &'a str,
    scope: &'a str,
    redirect_uri: &'a str,
    state: &'a str,
    nonce: &'a str,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct CodeResponse {
    // A token that can be sent to a Google API.
    access_token: String,
    // The remaining lifetime of the access token in seconds.
    expires_in: i64,
    // A JWT that contains identity information about the user that is digitally
    // signed by Google.
    id_token: String,
    // The scopes of access granted by the access_token expressed as a list of
    // space-delimited, case-sensitive strings.
    scope: String,
    // Identifies the type of token returned. At this time, this field always
    // has the value Bearer.
    token_type: String,
    // This field is only present if the access_type parameter was set to offline
    // in the authentication request. For details, see Refresh tokens.
    refresh_token: Option<String>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct Claims {
    // The audience that this ID token is intended for. It must be one of the OAuth
    // 2.0 client IDs of your application.
    aud: String,
    // Expiration time on or after which the ID token must not be accepted. Represented
    // in Unix time (integer seconds).
    exp: i64,
    // The time the ID token was issued. Represented in Unix time (integer seconds).
    iat: i64,
    // The Issuer Identifier for the Issuer of the response. Always
    // https://accounts.google.com or accounts.google.com for Google ID tokens.
    iss: String,
    // An identifier for the user, unique among all Google accounts and never reused. A
    // Google account can have multiple email addresses at different points in time, but
    // the sub value is never changed. Use sub within your application as the
    // unique-identifier key for the user. Maximum length of 255 case-sensitive ASCII characters.
    sub: String,
    // Access token hash. Provides validation that the access token is tied to the identity
    // token. If the ID token is issued with an access_token value in the server flow, this
    // claim is always included. This claim can be used as an alternate mechanism to protect
    // against cross-site request forgery attacks, but if you follow Step 1 and Step 3 it is
    // not necessary to verify the access token.
    at_hash: Option<String>,
    // The user's email address. Provided only if you included the email scope in your request.
    // The value of this claim may not be unique to this account and could change over time,
    // therefore you should not use this value as the primary identifier to link to your user
    // record. You also can't rely on the domain of the email claim to identify users of
    // Google Workspace or Cloud organizations; use the hd claim instead.
    email: Option<String>,
    // True if the user's e-mail address has been verified; otherwise false.
    email_verified: Option<bool>,
    // The value of the nonce supplied by your app in the authentication request. You should
    // enforce protection against replay attacks by ensuring it is presented only once.
    nonce: Option<String>,
}

fn self_uri(req: &Request) -> String {
    if let Some(host) = req.header("Host") {
        let prefix =
            if !req.is_secure() && !host.starts_with("localhost") && !host.starts_with("127.0.0.1")
            {
                "http://"
            } else {
                "https://"
            };
        format!("{prefix}{host}")
    } else {
        "".into()
    }
}

pub struct Authorizer {
    nonces: Arc<Mutex<HashSet<String>>>,
    session_ids: Arc<RwLock<HashSet<String>>>,
    auth_tokens: HashSet<String>,
    allowed_emails: HashSet<String>,
    oidc_client_id: String,
    oidc_client_secret: String,
}

impl Authorizer {
    pub fn new(
        auth_tokens: HashSet<String>,
        allowed_emails: HashSet<String>,
        oidc_client_id: String,
        oidc_client_secret: String,
    ) -> Self {
        Self {
            nonces: Arc::new(Mutex::new(HashSet::new())),
            session_ids: Arc::new(RwLock::new(HashSet::new())),
            allowed_emails,
            auth_tokens,
            oidc_client_id,
            oidc_client_secret,
        }
    }
}

impl Authorizer {
    fn is_authorized(&self, req: &Request) -> bool {
        if let Some(auth_header) = req.header("Authorization") {
            if self.auth_tokens.contains(auth_header) {
                return true;
            }
        }

        if let Some((_, val)) = input::cookies(req).find(|&(n, _)| n == SESSION_COOKIE_NAME) {
            // session_ids last for the lifetime of the program for simplicity.
            if self.session_ids.read().unwrap().contains(val) {
                return true;
            }
        }

        return false;
    }

    fn process_code(&self, req: &Request) -> Response {
        let state = match req.get_param("state") {
            Some(s) => s,
            None => return Response::text("missing state").with_status_code(400),
        };
        let nonces = self.nonces.lock().unwrap();
        if !nonces.contains(&state) {
            return Response::text("unknown state").with_status_code(400);
        }
        let code = match req.get_param("code") {
            Some(c) => c,
            None => return Response::text("missing code").with_status_code(400),
        };
        let redirect_uri = self_uri(&req) + "/code";
        let resp = ureq::post("https://oauth2.googleapis.com/token")
            .send_form(&[
                // The authorization code that is returned from the initial request.
                ("code", &code),
                // The client ID that you obtain from the API Console Credentials page, as
                // described in Obtain OAuth 2.0 credentials.
                ("client_id", &self.oidc_client_id),
                // The client secret that you obtain from the API Console Credentials page,
                // as described in Obtain OAuth 2.0 credentials.
                ("client_secret", &self.oidc_client_secret),
                // An authorized redirect URI for the given client_id specified in the API
                // Console Credentials page, as described in Set a redirect URI.
                ("redirect_uri", &redirect_uri),
                // This field must contain a value of authorization_code, as defined in
                // the OAuth 2.0 specification.
                ("grant_type", "authorization_code"),
            ])
            .unwrap();
        let parsed_resp: CodeResponse = resp.into_json().unwrap();
        let jsonclaims = URL_SAFE
            .decode(&parsed_resp.id_token.split(".").skip(1).next().unwrap())
            .unwrap();
        let claims: Claims = serde_json::from_slice(&jsonclaims).unwrap();

        // Check nonces
        let nonce = claims.nonce.unwrap_or_default();
        let mut nonces = self.nonces.lock().unwrap();
        if !nonces.contains(&nonce) {
            return Response::text("reused nonce").with_status_code(400);
        }
        nonces.remove(&nonce);

        let email = claims.email.unwrap_or_default();

        // Make sure user is allowed
        if !self.allowed_emails.contains(&email) {
            info!("denied {email}");
            return Response::text("not authorized").with_status_code(401);
        }
        info!("authenticated {email}");

        // Create session and add to headers
        let session_id = Alphanumeric.sample_string(&mut rand::thread_rng(), 16);
        let session_cookie = format!("{SESSION_COOKIE_NAME}={session_id}");
        self.session_ids.write().unwrap().insert(session_id);

        // Now back to where we wanted to go.
        Response::redirect_302(self_uri(req)).with_unique_header("Set-Cookie", session_cookie)
    }

    pub fn ensure_authorized<N>(&self, req: &Request, next: N) -> Response
    where
        N: FnOnce(&Request) -> Response,
    {
        // See if we're handling an earlier auth message.
        if req.url() == "/code" {
            return self.process_code(req);
        }

        if self.is_authorized(&req) {
            // No need to do any more auth, call our normal function.
            return next(req);
        }

        let redirect_uri = self_uri(&req) + "/code";
        // Construct a message for OIDC.
        // We omit state because CSRF attacks don't seem like a meaningful problem
        // for this specific application.
        let nonce = Alphanumeric.sample_string(&mut rand::thread_rng(), 16);
        let params = GoogleAuthParams {
            response_type: "code",
            client_id: &self.oidc_client_id,
            scope: "openid email",
            redirect_uri: &redirect_uri,
            state: &nonce,
            nonce: &nonce,
        };
        let encoded = serde_urlencoded::to_string(params).unwrap();
        self.nonces.lock().unwrap().insert(nonce);

        let redirect = format!("https://accounts.google.com/o/oauth2/v2/auth?{encoded}");
        Response::redirect_302(redirect)
    }
}
