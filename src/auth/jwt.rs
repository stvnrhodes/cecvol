use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const OUTER_KEY_PAD: u8 = 0x5c;
const INNER_KEY_PAD: u8 = 0x36;

pub enum Algorithm {
    HS256,
}

#[derive(Deserialize, Serialize)]
struct Header {
    alg: String,
    typ: String,
}

#[derive(Default, Debug, Deserialize, Serialize, PartialEq)]
struct Payload {
    // The "iss" (issuer) claim identifies the principal that issued the
    // JWT.  The processing of this claim is generally application specific.
    // The "iss" value is a case-sensitive string containing a StringOrURI
    // value.  Use of this claim is OPTIONAL.
    #[serde(skip_serializing_if = "Option::is_none")]
    iss: Option<String>,

    // The "sub" (subject) claim identifies the principal that is the
    // subject of the JWT.  The claims in a JWT are normally statements
    // about the subject.  The subject value MUST either be scoped to be
    // locally unique in the context of the issuer or be globally unique.
    // The processing of this claim is generally application specific.  The
    // "sub" value is a case-sensitive string containing a StringOrURI
    // value.  Use of this claim is OPTIONAL.
    #[serde(skip_serializing_if = "Option::is_none")]
    sub: Option<String>,

    // The "aud" (audience) claim identifies the recipients that the JWT is
    // intended for.  Each principal intended to process the JWT MUST
    // identify itself with a value in the audience claim.  If the principal
    // processing the claim does not identify itself with a value in the
    // "aud" claim when this claim is present, then the JWT MUST be
    // rejected.  In the general case, the "aud" value is an array of case-
    // sensitive strings, each containing a StringOrURI value.  In the
    // special case when the JWT has one audience, the "aud" value MAY be a
    // single case-sensitive string containing a StringOrURI value.  The
    // interpretation of audience values is generally application specific.
    // Use of this claim is OPTIONAL.
    #[serde(skip_serializing_if = "Option::is_none")]
    aud: Option<String>,

    // The "exp" (expiration time) claim identifies the expiration time on
    // or after which the JWT MUST NOT be accepted for processing.  The
    // processing of the "exp" claim requires that the current date/time
    // MUST be before the expiration date/time listed in the "exp" claim.
    // Implementers MAY provide for some small leeway, usually no more than
    // a few minutes, to account for clock skew.  Its value MUST be a number
    // containing a NumericDate value.  Use of this claim is OPTIONAL.
    #[serde(skip_serializing_if = "Option::is_none")]
    exp: Option<i64>,

    // The "nbf" (not before) claim identifies the time before which the JWT
    // MUST NOT be accepted for processing.  The processing of the "nbf"
    // claim requires that the current date/time MUST be after or equal to
    // the not-before date/time listed in the "nbf" claim.  Implementers MAY
    // provide for some small leeway, usually no more than a few minutes, to
    // account for clock skew.  Its value MUST be a number containing a
    // NumericDate value.  Use of this claim is OPTIONAL.
    #[serde(skip_serializing_if = "Option::is_none")]
    nbf: Option<i64>,

    // The "iat" (issued at) claim identifies the time at which the JWT was
    // issued.  This claim can be used to determine the age of the JWT.  Its
    // value MUST be a number containing a NumericDate value.  Use of this
    // claim is OPTIONAL.
    #[serde(skip_serializing_if = "Option::is_none")]
    iat: Option<i64>,

    // The "jti" (JWT ID) claim provides a unique identifier for the JWT.
    // The identifier value MUST be assigned in a manner that ensures that
    // there is a negligible probability that the same value will be
    // accidentally assigned to a different data object; if the application
    // uses multiple issuers, collisions MUST be prevented among values
    // produced by different issuers as well.  The "jti" claim can be used
    // to prevent the JWT from being replayed.  The "jti" value is a case-
    // sensitive string.  Use of this claim is OPTIONAL.
    #[serde(skip_serializing_if = "Option::is_none")]
    jti: Option<String>,
}

//    HMACSHA256(
//     base64UrlEncode(header) + "." +
//     base64UrlEncode(payload),
//     secret)

#[derive(thiserror::Error, Debug)]
enum Error {
    #[error("Wrong number of . sections")]
    WrongNumSections(usize),
    #[error("Unknown algorithm")]
    UnknownAlgorithm(String),
    #[error("Unknown header type")]
    UnknownHeaderType(String),
    #[error("JWT signature does not match expected value")]
    BadSignature(String),
    #[error("Issue decoding as base64")]
    Base64Error(#[from] base64::DecodeError),
    #[error("Issue encoding as json")]
    JSONError(#[from] serde_json::Error),
}

fn hmac_sha256(header: impl AsRef<[u8]>, payload: impl AsRef<[u8]>, secret: &str) -> Vec<u8> {
    let mut padded_key: [u8; 64] = [0; 64];
    for (dst, src) in padded_key.iter_mut().zip(secret.bytes()) {
        *dst = src
    }
    let outer_key: Vec<u8> = padded_key.iter().map(|x| x ^ OUTER_KEY_PAD).collect();
    let inner_key: Vec<u8> = padded_key.iter().map(|x| x ^ INNER_KEY_PAD).collect();
    let inner_hash = Sha256::new()
        .chain(inner_key)
        .chain(header)
        .chain(".")
        .chain(payload)
        .finalize();
    let outer_hash = Sha256::new().chain(outer_key).chain(inner_hash).finalize();
    outer_hash.to_vec()
}

impl Payload {
    fn from_token(token: &str, secret: &str) -> Result<Payload, Error> {
        let vec: Vec<&str> = token.split('.').collect();
        if vec.len() != 3 {
            return Err(Error::WrongNumSections(vec.len()));
        }
        let header_json = base64::decode_config(vec[0], base64::URL_SAFE_NO_PAD)?;
        let header: Header = serde_json::from_slice(&header_json)?;
        if header.typ != "JWT" {
            return Err(Error::UnknownHeaderType(header.typ));
        }
        match header.alg.as_str() {
            "HS256" => {
                let hash = hmac_sha256(vec[0], vec[1], secret);
                let want_sig = base64::encode_config(hash, base64::URL_SAFE_NO_PAD);
                let sig = vec[2];
                if sig != want_sig {
                    return Err(Error::BadSignature(sig.to_string()));
                }
            }
            _ => {
                return Err(Error::UnknownAlgorithm(header.alg));
            }
        }

        let payload_json = base64::decode_config(vec[1], base64::URL_SAFE_NO_PAD)?;
        let payload: Payload = serde_json::from_slice(&payload_json)?;
        Ok(payload)
    }
    fn to_token(&self, alg: Algorithm, secret: &str) -> Result<String, Error> {
        let payload = base64::encode_config(serde_json::to_string(self)?, base64::URL_SAFE_NO_PAD);
        match alg {
            Algorithm::HS256 => {
                let header = base64::encode_config(
                    serde_json::to_string(&Header {
                        alg: "HS256".to_string(),
                        typ: "JWT".to_string(),
                    })?,
                    base64::URL_SAFE_NO_PAD,
                );
                let hash = hmac_sha256(&header, &payload, secret);
                let sig = base64::encode_config(hash, base64::URL_SAFE_NO_PAD);
                Ok(header + "." + &payload + "." + &sig)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::auth::jwt::*;

    #[test]
    fn test_parse_jwt() {
        let token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwiaWF0IjoxNTE2MjM5MDIyfQ.L8i6g3PfcHlioHCCPURC9pmXT7gdJpx3kOoyAfNUwCc";
        let payload = Payload::from_token(token, "your-256-bit-secret").unwrap();
        assert_eq!(
            payload,
            Payload {
                sub: Some("1234567890".to_string()),
                iat: Some(1516239022),
                ..Default::default()
            },
        );
    }

    #[test]
    fn test_create_jwt() {
        let payload = Payload {
            sub: Some("1234567890".to_string()),
            iat: Some(1516239022),
            ..Default::default()
        };
        let token = payload
            .to_token(Algorithm::HS256, "your-256-bit-secret")
            .unwrap();
        assert_eq!(token,
            "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwiaWF0IjoxNTE2MjM5MDIyfQ.L8i6g3PfcHlioHCCPURC9pmXT7gdJpx3kOoyAfNUwCc");
    }
}
