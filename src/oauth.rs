use percent_encoding::{percent_decode_str, utf8_percent_encode, NON_ALPHANUMERIC};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const AUTH_CODE_TTL_SECS: u64 = 300;
const ACCESS_TOKEN_TTL_SECS: u64 = 3600;
pub const MIN_SECRET_LEN: usize = 32;

#[derive(Debug, PartialEq, Eq)]
pub enum OAuthError {
    InvalidRequest(&'static str),
    InvalidClient(&'static str),
    InvalidGrant(&'static str),
    UnsupportedGrantType(&'static str),
    InvalidToken(&'static str),
}

impl OAuthError {
    pub fn error_code(&self) -> &'static str {
        match self {
            OAuthError::InvalidRequest(_) => "invalid_request",
            OAuthError::InvalidClient(_) => "invalid_client",
            OAuthError::InvalidGrant(_) => "invalid_grant",
            OAuthError::UnsupportedGrantType(_) => "unsupported_grant_type",
            OAuthError::InvalidToken(_) => "invalid_token",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            OAuthError::InvalidRequest(s)
            | OAuthError::InvalidClient(s)
            | OAuthError::InvalidGrant(s)
            | OAuthError::UnsupportedGrantType(s)
            | OAuthError::InvalidToken(s) => s,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum TokenType {
    Code,
    Access,
    Refresh,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct TokenClaims {
    typ: TokenType,
    sub: String,
    cid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    exp: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    jti: Option<String>,
}

#[derive(Deserialize, Debug)]
struct AuthorizeQuery {
    client_id: Option<String>,
    redirect_uri: Option<String>,
    response_type: Option<String>,
    state: Option<String>,
}

#[derive(Deserialize, Debug)]
struct TokenRequest {
    grant_type: Option<String>,
    code: Option<String>,
    redirect_uri: Option<String>,
    refresh_token: Option<String>,
    client_id: Option<String>,
    client_secret: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct TokenResponse {
    pub token_type: String,
    pub access_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    pub expires_in: u64,
}

pub struct OAuthServer {
    client_id: String,
    client_secret: String,
    allowed_redirect_uris: Vec<String>,
    signing_key: [u8; 32],
    used_codes: Mutex<HashMap<String, u64>>,
}

impl OAuthServer {
    pub fn new(
        client_id: String,
        client_secret: String,
        signing_key: String,
        allowed_redirect_uris: Vec<String>,
    ) -> Result<Self, &'static str> {
        if client_id.is_empty() {
            return Err("OAUTH_CLIENT_ID must not be empty");
        }
        if client_secret.len() < MIN_SECRET_LEN {
            return Err("OAUTH_CLIENT_SECRET must be at least 32 bytes");
        }
        if signing_key.len() < MIN_SECRET_LEN {
            return Err("OAUTH_SIGNING_KEY must be at least 32 bytes");
        }

        let mut hasher = Sha256::new();
        hasher.update(b"cecvol-oauth-v1:");
        hasher.update(signing_key.as_bytes());
        let signing_key: [u8; 32] = hasher.finalize().into();

        Ok(Self {
            client_id,
            client_secret,
            allowed_redirect_uris,
            signing_key,
            used_codes: Mutex::new(HashMap::new()),
        })
    }

    pub fn authorize(&self, query: &str, user: &str) -> Result<String, OAuthError> {
        self.authorize_at(query, user, now_unix())
    }

    fn authorize_at(&self, query: &str, user: &str, now: u64) -> Result<String, OAuthError> {
        let params: AuthorizeQuery = serde_urlencoded::from_str(query)
            .map_err(|_| OAuthError::InvalidRequest("malformed query parameters"))?;

        let client_id = params
            .client_id
            .as_deref()
            .ok_or(OAuthError::InvalidRequest("missing client_id"))?;
        if !constant_time_eq(client_id.as_bytes(), self.client_id.as_bytes()) {
            return Err(OAuthError::InvalidClient("unknown client_id"));
        }

        let response_type = params
            .response_type
            .as_deref()
            .ok_or(OAuthError::InvalidRequest("missing response_type"))?;
        if response_type != "code" {
            return Err(OAuthError::InvalidRequest("response_type must be code"));
        }

        let redirect_uri = params
            .redirect_uri
            .as_deref()
            .ok_or(OAuthError::InvalidRequest("missing redirect_uri"))?;
        if !self.allowed_redirect_uris.iter().any(|u| u == redirect_uri) {
            return Err(OAuthError::InvalidRequest("redirect_uri is not allowed"));
        }

        let claims = TokenClaims {
            typ: TokenType::Code,
            sub: user.to_string(),
            cid: self.client_id.clone(),
            exp: Some(now + AUTH_CODE_TTL_SECS),
            uri: Some(redirect_uri.to_string()),
            jti: Some(random_hex_16()),
        };
        let code = self.sign_token(&claims);

        let sep = if redirect_uri.contains('?') { '&' } else { '?' };
        let mut location = format!(
            "{}{}code={}",
            redirect_uri,
            sep,
            utf8_percent_encode(&code, NON_ALPHANUMERIC)
        );
        if let Some(state) = params.state {
            location.push_str("&state=");
            location.push_str(&utf8_percent_encode(&state, NON_ALPHANUMERIC).to_string());
        }
        Ok(location)
    }

    pub fn token(
        &self,
        body: &str,
        authorization_header: Option<&str>,
    ) -> Result<TokenResponse, OAuthError> {
        self.token_at(body, authorization_header, now_unix())
    }

    fn token_at(
        &self,
        body: &str,
        authorization_header: Option<&str>,
        now: u64,
    ) -> Result<TokenResponse, OAuthError> {
        let req: TokenRequest = serde_urlencoded::from_str(body)
            .map_err(|_| OAuthError::InvalidRequest("malformed form body"))?;

        self.verify_client_credentials(&req, authorization_header)?;

        let grant_type = req
            .grant_type
            .as_deref()
            .ok_or(OAuthError::InvalidRequest("missing grant_type"))?;

        match grant_type {
            "authorization_code" => {
                let code = req
                    .code
                    .as_deref()
                    .ok_or(OAuthError::InvalidGrant("missing code"))?;
                let claims = self.verify_token(code, TokenType::Code, now)?;

                if let Some(expected_uri) = claims.uri.as_deref() {
                    match req.redirect_uri.as_deref() {
                        Some(provided_uri) if provided_uri == expected_uri => {}
                        _ => return Err(OAuthError::InvalidGrant("redirect_uri mismatch")),
                    }
                }

                if let Some(jti) = claims.jti {
                    let mut used = self.used_codes.lock().unwrap();
                    used.retain(|_, &mut exp| exp > now);
                    if used.contains_key(&jti) {
                        return Err(OAuthError::InvalidGrant("authorization code already used"));
                    }
                    used.insert(jti, claims.exp.unwrap_or(now + AUTH_CODE_TTL_SECS));
                }

                let access_token = self.sign_token(&TokenClaims {
                    typ: TokenType::Access,
                    sub: claims.sub.clone(),
                    cid: self.client_id.clone(),
                    exp: Some(now + ACCESS_TOKEN_TTL_SECS),
                    uri: None,
                    jti: None,
                });
                let refresh_token = self.sign_token(&TokenClaims {
                    typ: TokenType::Refresh,
                    sub: claims.sub,
                    cid: self.client_id.clone(),
                    exp: None,
                    uri: None,
                    jti: Some(random_hex_16()),
                });

                Ok(TokenResponse {
                    token_type: "Bearer".to_string(),
                    access_token,
                    refresh_token: Some(refresh_token),
                    expires_in: ACCESS_TOKEN_TTL_SECS,
                })
            }
            "refresh_token" => {
                let refresh_token = req
                    .refresh_token
                    .as_deref()
                    .ok_or(OAuthError::InvalidGrant("missing refresh_token"))?;
                let claims = self.verify_token(refresh_token, TokenType::Refresh, now)?;

                let access_token = self.sign_token(&TokenClaims {
                    typ: TokenType::Access,
                    sub: claims.sub,
                    cid: self.client_id.clone(),
                    exp: Some(now + ACCESS_TOKEN_TTL_SECS),
                    uri: None,
                    jti: None,
                });

                Ok(TokenResponse {
                    token_type: "Bearer".to_string(),
                    access_token,
                    refresh_token: None,
                    expires_in: ACCESS_TOKEN_TTL_SECS,
                })
            }
            _ => Err(OAuthError::UnsupportedGrantType("unsupported grant_type")),
        }
    }

    pub fn verify_bearer(&self, authorization_header: &str) -> Result<String, OAuthError> {
        self.verify_bearer_at(authorization_header, now_unix())
    }

    fn verify_bearer_at(
        &self,
        authorization_header: &str,
        now: u64,
    ) -> Result<String, OAuthError> {
        let mut parts = authorization_header.splitn(2, ' ');
        let scheme = parts.next().unwrap_or("");
        let token = parts.next().unwrap_or("").trim();
        if !scheme.eq_ignore_ascii_case("bearer") || token.is_empty() {
            return Err(OAuthError::InvalidToken("missing bearer token"));
        }
        let claims = self
            .verify_token(token, TokenType::Access, now)
            .map_err(|_| OAuthError::InvalidToken("invalid or expired access token"))?;
        Ok(claims.sub)
    }

    fn verify_client_credentials(
        &self,
        req: &TokenRequest,
        authorization_header: Option<&str>,
    ) -> Result<(), OAuthError> {
        if let Some(auth) = authorization_header {
            if let Some((basic_id, basic_secret)) = parse_basic_auth(auth) {
                if self.matches_client(&basic_id, &basic_secret) {
                    return Ok(());
                }
                // RFC 6749 §2.3.1 allows percent-encoded credentials in Basic Auth
                let dec_id = percent_decode_str(&basic_id).decode_utf8_lossy();
                let dec_secret = percent_decode_str(&basic_secret).decode_utf8_lossy();
                if self.matches_client(&dec_id, &dec_secret) {
                    return Ok(());
                }
                return Err(OAuthError::InvalidClient("invalid basic credentials"));
            }
        }

        let cid = req
            .client_id
            .as_deref()
            .ok_or(OAuthError::InvalidClient("missing client_id"))?;
        let csecret = req
            .client_secret
            .as_deref()
            .ok_or(OAuthError::InvalidClient("missing client_secret"))?;
        if self.matches_client(cid, csecret) {
            Ok(())
        } else {
            Err(OAuthError::InvalidClient("invalid client credentials"))
        }
    }

    fn matches_client(&self, client_id: &str, client_secret: &str) -> bool {
        let id_ok = constant_time_eq(client_id.as_bytes(), self.client_id.as_bytes());
        let secret_ok = constant_time_eq(client_secret.as_bytes(), self.client_secret.as_bytes());
        id_ok && secret_ok
    }

    fn sign_token(&self, claims: &TokenClaims) -> String {
        let json = serde_json::to_vec(claims).expect("serializing TokenClaims cannot fail");
        let payload_b64 = base64url_encode(&json);
        let mac = hmac_sha256(&self.signing_key, payload_b64.as_bytes());
        let sig_b64 = base64url_encode(&mac);
        format!("{}.{}", payload_b64, sig_b64)
    }

    fn verify_token(
        &self,
        token: &str,
        expected_type: TokenType,
        now: u64,
    ) -> Result<TokenClaims, OAuthError> {
        let mut parts = token.split('.');
        let payload_b64 = parts
            .next()
            .ok_or(OAuthError::InvalidGrant("malformed token"))?;
        let sig_b64 = parts
            .next()
            .ok_or(OAuthError::InvalidGrant("malformed token"))?;
        if parts.next().is_some() {
            return Err(OAuthError::InvalidGrant("malformed token"));
        }

        let provided_sig =
            base64url_decode(sig_b64).ok_or(OAuthError::InvalidGrant("invalid signature encoding"))?;
        let expected_sig = hmac_sha256(&self.signing_key, payload_b64.as_bytes());
        if !constant_time_eq(&provided_sig, &expected_sig) {
            return Err(OAuthError::InvalidGrant("invalid token signature"));
        }

        let payload_bytes = base64url_decode(payload_b64)
            .ok_or(OAuthError::InvalidGrant("invalid payload encoding"))?;
        let claims: TokenClaims = serde_json::from_slice(&payload_bytes)
            .map_err(|_| OAuthError::InvalidGrant("invalid token payload"))?;

        if claims.typ != expected_type {
            return Err(OAuthError::InvalidGrant("wrong token type"));
        }
        if !constant_time_eq(claims.cid.as_bytes(), self.client_id.as_bytes()) {
            return Err(OAuthError::InvalidGrant("client_id mismatch"));
        }
        if let Some(exp) = claims.exp {
            if now >= exp {
                return Err(OAuthError::InvalidGrant("token expired"));
            }
        }
        Ok(claims)
    }
}

pub fn google_redirect_uris(project_id: &str) -> Vec<String> {
    vec![
        format!("https://oauth-redirect.googleusercontent.com/r/{}", project_id),
        format!(
            "https://oauth-redirect-sandbox.googleusercontent.com/r/{}",
            project_id
        ),
    ]
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn random_hex_16() -> String {
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    let mut out = String::with_capacity(32);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// RFC 2104 HMAC-SHA256
fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    const BLOCK_SIZE: usize = 64;
    let mut key_block = [0u8; BLOCK_SIZE];
    if key.len() > BLOCK_SIZE {
        let hashed = Sha256::digest(key);
        key_block[..32].copy_from_slice(&hashed);
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }

    let mut ipad = [0x36u8; BLOCK_SIZE];
    let mut opad = [0x5cu8; BLOCK_SIZE];
    for i in 0..BLOCK_SIZE {
        ipad[i] ^= key_block[i];
        opad[i] ^= key_block[i];
    }

    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(msg);
    let inner_hash = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner_hash);
    outer.finalize().into()
}

fn parse_basic_auth(header_value: &str) -> Option<(String, String)> {
    let mut parts = header_value.splitn(2, ' ');
    let scheme = parts.next()?;
    let encoded = parts.next()?.trim();
    if !scheme.eq_ignore_ascii_case("basic") || encoded.is_empty() {
        return None;
    }
    let decoded = base64_decode_standard(encoded)?;
    let s = String::from_utf8(decoded).ok()?;
    let idx = s.find(':')?;
    Some((s[..idx].to_string(), s[idx + 1..].to_string()))
}

const B64URL_TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn base64url_encode(input: &[u8]) -> String {
    let mut out = String::with_capacity((input.len() * 4).div_ceil(3));
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(B64URL_TABLE[((triple >> 18) & 0x3f) as usize] as char);
        out.push(B64URL_TABLE[((triple >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(B64URL_TABLE[((triple >> 6) & 0x3f) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(B64URL_TABLE[(triple & 0x3f) as usize] as char);
        }
    }
    out
}

fn base64url_decode(input: &str) -> Option<Vec<u8>> {
    base64_decode_inner(input.as_bytes(), true)
}

fn base64_decode_standard(input: &str) -> Option<Vec<u8>> {
    let trimmed = input.trim_end_matches('=');
    base64_decode_inner(trimmed.as_bytes(), false)
}

fn base64_decode_inner(bytes: &[u8], url_safe: bool) -> Option<Vec<u8>> {
    if bytes.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity((bytes.len() * 3) / 4);
    let decode_char = |c: u8| -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a' + 26) as u32),
            b'0'..=b'9' => Some((c - b'0' + 52) as u32),
            b'-' if url_safe => Some(62),
            b'_' if url_safe => Some(63),
            b'+' if !url_safe => Some(62),
            b'/' if !url_safe => Some(63),
            _ => None,
        }
    };

    for chunk in bytes.chunks(4) {
        let v0 = decode_char(chunk[0])?;
        let v1 = decode_char(chunk[1])?;
        out.push(((v0 << 2) | (v1 >> 4)) as u8);
        if chunk.len() > 2 {
            let v2 = decode_char(chunk[2])?;
            out.push((((v1 & 0x0f) << 4) | (v2 >> 2)) as u8);
            if chunk.len() > 3 {
                let v3 = decode_char(chunk[3])?;
                out.push((((v2 & 0x03) << 6) | v3) as u8);
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SECRET: &str = "super-secret-value-at-least-32-bytes-long";
    const TEST_SIGNING_KEY: &str = "separate-signing-key-at-least-32-bytes-long";

    fn test_server() -> OAuthServer {
        OAuthServer::new(
            "my-client".to_string(),
            TEST_SECRET.to_string(),
            TEST_SIGNING_KEY.to_string(),
            google_redirect_uris("my-project-123"),
        )
        .unwrap()
    }

    fn extract_query_param(url: &str, key: &str) -> Option<String> {
        let query = url.split_once('?')?.1;
        let pairs: Vec<(String, String)> = serde_urlencoded::from_str(query).ok()?;
        pairs.into_iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    #[test]
    fn test_hmac_sha256_rfc4231_case1() {
        let key = [0x0bu8; 20];
        let data = b"Hi There";
        let mac = hmac_sha256(&key, data);
        let expected: [u8; 32] = [
            0xb0, 0x34, 0x4c, 0x61, 0xd8, 0xdb, 0x38, 0x53, 0x5c, 0xa8, 0xaf, 0xce, 0xaf, 0x0b,
            0xf1, 0x2b, 0x88, 0x1d, 0xc2, 0x00, 0xc9, 0x83, 0x3d, 0xa7, 0x26, 0xe9, 0x37, 0x6c,
            0x2e, 0x32, 0xcf, 0xf7,
        ];
        assert_eq!(mac, expected);
    }

    #[test]
    fn test_hmac_sha256_rfc4231_case2() {
        let key = b"Jefe";
        let data = b"what do ya want for nothing?";
        let mac = hmac_sha256(key, data);
        let expected: [u8; 32] = [
            0x5b, 0xdc, 0xc1, 0x46, 0xbf, 0x60, 0x75, 0x4e, 0x6a, 0x04, 0x24, 0x26, 0x08, 0x95,
            0x75, 0xc7, 0x5a, 0x00, 0x3f, 0x08, 0x9d, 0x27, 0x39, 0x83, 0x9d, 0xec, 0x58, 0xb9,
            0x64, 0xec, 0x38, 0x43,
        ];
        assert_eq!(mac, expected);
    }

    #[test]
    fn test_base64url_roundtrip() {
        for input in [
            b"".as_slice(),
            b"a",
            b"ab",
            b"abc",
            b"hello world! \xff\xfe\xfd",
        ] {
            let encoded = base64url_encode(input);
            let decoded = base64url_decode(&encoded).unwrap();
            assert_eq!(decoded, input);
        }
    }

    #[test]
    fn test_full_oauth_flow() {
        let server = test_server();
        let redirect = "https://oauth-redirect.googleusercontent.com/r/my-project-123";
        let query = format!(
            "client_id=my-client&redirect_uri={}&response_type=code&state=abc+123",
            utf8_percent_encode(redirect, NON_ALPHANUMERIC)
        );

        let loc = server
            .authorize_at(&query, "user@example.com", 1000)
            .unwrap();
        assert!(loc.starts_with(redirect));
        assert_eq!(extract_query_param(&loc, "state").unwrap(), "abc 123");
        let code = extract_query_param(&loc, "code").unwrap();

        let body = format!(
            "grant_type=authorization_code&code={}&redirect_uri={}&client_id=my-client&client_secret={}",
            utf8_percent_encode(&code, NON_ALPHANUMERIC),
            utf8_percent_encode(redirect, NON_ALPHANUMERIC),
            TEST_SECRET
        );
        let tok = server.token_at(&body, None, 1010).unwrap();
        assert_eq!(tok.token_type, "Bearer");
        assert_eq!(tok.expires_in, 3600);
        let refresh_token = tok.refresh_token.clone().unwrap();

        // Access token verifies and returns authenticated user
        let sub = server
            .verify_bearer_at(&format!("Bearer {}", tok.access_token), 1020)
            .unwrap();
        assert_eq!(sub, "user@example.com");

        // Replay of authorization code must fail
        assert_eq!(
            server.token_at(&body, None, 1020).unwrap_err().error_code(),
            "invalid_grant"
        );

        // Access token expires after 3600s
        assert!(server
            .verify_bearer_at(&format!("Bearer {}", tok.access_token), 1010 + 3600)
            .is_err());

        // Refresh token still works far in the future (even on a fresh server instance!)
        let fresh_server = test_server();
        let refresh_body = format!(
            "grant_type=refresh_token&refresh_token={}&client_id=my-client&client_secret={}",
            utf8_percent_encode(&refresh_token, NON_ALPHANUMERIC),
            TEST_SECRET
        );
        let refreshed = fresh_server
            .token_at(&refresh_body, None, 10_000_000)
            .unwrap();
        assert!(refreshed.refresh_token.is_none());
        let sub2 = fresh_server
            .verify_bearer_at(&format!("Bearer {}", refreshed.access_token), 10_000_100)
            .unwrap();
        assert_eq!(sub2, "user@example.com");
    }

    #[test]
    fn test_basic_auth_token_exchange() {
        let server = test_server();
        let redirect = "https://oauth-redirect-sandbox.googleusercontent.com/r/my-project-123";
        let query = format!(
            "client_id=my-client&redirect_uri={}&response_type=code&state=s",
            utf8_percent_encode(redirect, NON_ALPHANUMERIC)
        );
        let loc = server.authorize_at(&query, "user@example.com", 100).unwrap();
        let code = extract_query_param(&loc, "code").unwrap();

        // Standard base64 of "my-client:super-secret-value-at-least-32-bytes-long"
        let basic_b64 = base64url_encode(format!("my-client:{TEST_SECRET}").as_bytes())
            .replace('-', "+")
            .replace('_', "/");
        let body = format!(
            "grant_type=authorization_code&code={}&redirect_uri={}",
            utf8_percent_encode(&code, NON_ALPHANUMERIC),
            utf8_percent_encode(redirect, NON_ALPHANUMERIC)
        );
        let tok = server
            .token_at(&body, Some(&format!("Basic {basic_b64}")), 110)
            .unwrap();
        assert_eq!(tok.token_type, "Bearer");
    }

    #[test]
    fn test_rejects_short_secrets_and_isolates_signing_key() {
        assert!(OAuthServer::new(
            "id".to_string(),
            "too-short".to_string(),
            TEST_SIGNING_KEY.to_string(),
            vec![],
        )
        .is_err());
        assert!(OAuthServer::new(
            "id".to_string(),
            TEST_SECRET.to_string(),
            "too-short".to_string(),
            vec![],
        )
        .is_err());

        // Rotating signing_key invalidates previously issued tokens even with same client_secret
        let server_a = test_server();
        let server_b = OAuthServer::new(
            "my-client".to_string(),
            TEST_SECRET.to_string(),
            "rotated-signing-key-at-least-32-bytes-long".to_string(),
            google_redirect_uris("my-project-123"),
        )
        .unwrap();
        let redirect = "https://oauth-redirect.googleusercontent.com/r/my-project-123";
        let loc = server_a
            .authorize_at(
                &format!(
                    "client_id=my-client&redirect_uri={}&response_type=code",
                    utf8_percent_encode(redirect, NON_ALPHANUMERIC)
                ),
                "user@example.com",
                100,
            )
            .unwrap();
        let code = extract_query_param(&loc, "code").unwrap();
        let tok = server_a
            .token_at(
                &format!(
                    "grant_type=authorization_code&code={}&redirect_uri={}&client_id=my-client&client_secret={}",
                    utf8_percent_encode(&code, NON_ALPHANUMERIC),
                    utf8_percent_encode(redirect, NON_ALPHANUMERIC),
                    TEST_SECRET
                ),
                None,
                110,
            )
            .unwrap();
        assert!(server_b
            .verify_bearer_at(&format!("Bearer {}", tok.access_token), 120)
            .is_err());
    }

    #[test]
    fn test_rejections() {
        let server = test_server();
        // Disallowed redirect URI
        let bad_uri = server.authorize_at(
            "client_id=my-client&redirect_uri=https://evil.example/cb&response_type=code&state=1",
            "user@example.com",
            100,
        );
        assert_eq!(bad_uri.unwrap_err().error_code(), "invalid_request");

        // Wrong client secret
        let bad_secret = server.token_at(
            "grant_type=refresh_token&refresh_token=abc&client_id=my-client&client_secret=wrong",
            None,
            100,
        );
        assert_eq!(bad_secret.unwrap_err().error_code(), "invalid_client");

        // Refresh token cannot be used as Bearer access token
        let redirect = "https://oauth-redirect.googleusercontent.com/r/my-project-123";
        let loc = server
            .authorize_at(
                &format!(
                    "client_id=my-client&redirect_uri={}&response_type=code",
                    utf8_percent_encode(redirect, NON_ALPHANUMERIC)
                ),
                "user@example.com",
                100,
            )
            .unwrap();
        let code = extract_query_param(&loc, "code").unwrap();
        let tok = server
            .token_at(
                &format!(
                    "grant_type=authorization_code&code={}&redirect_uri={}&client_id=my-client&client_secret={}",
                    utf8_percent_encode(&code, NON_ALPHANUMERIC),
                    utf8_percent_encode(redirect, NON_ALPHANUMERIC),
                    TEST_SECRET
                ),
                None,
                110,
            )
            .unwrap();
        assert!(server
            .verify_bearer_at(&format!("Bearer {}", tok.refresh_token.unwrap()), 120)
            .is_err());

        // Tampered token rejected
        let mut tampered = tok.access_token.clone();
        tampered.push('x');
        assert!(server
            .verify_bearer_at(&format!("Bearer {}", tampered), 120)
            .is_err());
    }
}
