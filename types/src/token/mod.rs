use crate::{Credential, UserID};
use chrono::{DateTime, Utc};
use serde::{de, ser, Deserialize, Serialize};

pub type RefreshTokenID = Credential<16>;

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub enum Error {
    #[error("decode error: {0}")]
    Decode(#[from] DecodeError),

    #[error("encode error: {0}")]
    Encode(#[from] EncodeError),

    #[error("decode header error: {0}")]
    DecodeHeader(#[from] DecodeHeaderError),
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub enum DecodeError {
    #[error("missing header")]
    MissingHeader,

    #[error("missing payload")]
    MissingPayload,

    #[error("missing signature")]
    MissingSignature,

    #[error("invalid json: {0}")]
    InvalidJSON(String),

    #[error("invalid encoding: `{0}`")]
    InvalidEncoding(String),

    #[error("invalid signature")]
    InvalidSignature,

    #[error("validation error: {0}")]
    ValidationError(#[from] ValidationError),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub enum ValidationError {
    #[error("token is expired since {seconds} seconds")]
    Expired { seconds: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub enum EncodeError {
    #[error("json error: {0}")]
    JsonError(String),
}

impl From<serde_json::Error> for EncodeError {
    fn from(err: serde_json::Error) -> Self {
        Self::JsonError(err.to_string())
    }
}

#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecodeHeaderError {
    #[error("header is missing")]
    MissingHeader,

    #[error("invalid header encoding: {0}")]
    InvalidEncoding(String),

    #[error("invalid header value syntax")]
    InvalidSyntax,

    #[error("invalid header schema: {0}")]
    InvalidSchema(String),
}

#[cfg(feature = "actix")]
impl Error {
    pub fn status_code(&self) -> actix_web::http::StatusCode {
        use actix_web::http::StatusCode;
        match self {
            Error::Decode(_) => StatusCode::BAD_REQUEST,
            Error::Encode(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::DecodeHeader(_) => StatusCode::BAD_REQUEST,
        }
    }
}

pub type Key = Vec<u8>;
pub type Signature = Vec<u8>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Algorithm {
    /// HMAC using SHA-256
    HS256,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    alg: Algorithm,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token<P: ser::Serialize + de::DeserializeOwned> {
    header: Header,
    payload: P,
    signature: Signature,
}

pub type AccessToken = Token<AccessTokenPayload>;
pub type RefreshToken = Token<RefreshTokenPayload>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessTokenPayload {
    pub sub: UserID,

    #[serde(with = "chrono::serde::ts_seconds")]
    pub exp: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefreshTokenPayload {
    pub tid: RefreshTokenID,
    pub sub: UserID,

    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub exp: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct BasePayload {
    #[serde(with = "chrono::serde::ts_seconds_option")]
    exp: Option<DateTime<Utc>>,
}

fn base64_encode(val: &[u8]) -> String {
    base64::encode_config(&val, base64::URL_SAFE_NO_PAD)
}

fn base64_decode(val: &str) -> Result<Vec<u8>, base64::DecodeError> {
    base64::decode_config(val, base64::URL_SAFE_NO_PAD)
}

fn encode_part<T: ser::Serialize>(val: &T) -> Result<String, serde_json::Error> {
    let string = serde_json::to_string(val)?;
    Ok(base64_encode(string.as_bytes()))
}

fn decode_part<T: de::DeserializeOwned>(val: &str) -> Result<T, DecodeError> {
    let json_bytes =
        base64_decode(val).map_err(|err| DecodeError::InvalidEncoding(err.to_string()))?;
    let json = String::from_utf8(json_bytes)
        .map_err(|err| DecodeError::InvalidEncoding(err.to_string()))?;
    serde_json::from_str(&json).map_err(|err| DecodeError::InvalidJSON(err.to_string()))
}

use ring::hmac;

pub fn sign<P: ser::Serialize + de::DeserializeOwned>(
    key: &Key,
    payload: P,
) -> Result<Token<P>, EncodeError> {
    const ALGORITHM: Algorithm = Algorithm::HS256; // that can be changed in the future
    const HEADER: Header = Header { alg: ALGORITHM };

    let raw_header = encode_part(&HEADER)?;
    let raw_payload = encode_part(&payload)?;
    let message = [raw_header, raw_payload].join(".");
    let signature: Signature = match ALGORITHM {
        Algorithm::HS256 => {
            let key = hmac::Key::new(hmac::HMAC_SHA256, key);
            Vec::from(hmac::sign(&key, message.as_bytes()).as_ref())
        }
    };

    let token = Token {
        header: HEADER,
        payload,
        signature,
    };
    Ok(token)
}

impl<P: ser::Serialize + de::DeserializeOwned> Token<P> {
    pub fn encode(&self) -> Result<String, EncodeError> {
        let raw_header = encode_part(&self.header)?;
        let raw_payload = encode_part(&self.payload)?;
        let raw_signature = base64_encode(&self.signature);
        Ok([raw_header, raw_payload, raw_signature].join("."))
    }

    pub fn decode(key: &Key, token: &str) -> Result<Self, DecodeError> {
        let mut iter = token.split(".");
        let raw_header = iter.next().ok_or(DecodeError::MissingHeader)?;
        let raw_payload = iter.next().ok_or(DecodeError::MissingPayload)?;
        let raw_signature = iter.next().ok_or(DecodeError::MissingSignature)?;

        let header = decode_part::<Header>(raw_header)?;
        let signature = base64_decode(raw_signature)
            .map_err(|err| DecodeError::InvalidEncoding(err.to_string()))?;

        let is_signature_valid = match header.alg {
            Algorithm::HS256 => {
                let key = hmac::Key::new(hmac::HMAC_SHA256, key);
                hmac::verify(
                    &key,
                    [raw_header, raw_payload].join(".").as_bytes(),
                    &signature,
                )
                .is_ok()
            }
        };
        if !is_signature_valid {
            return Err(DecodeError::InvalidSignature);
        }

        let payload_base = decode_part::<BasePayload>(raw_payload)?;
        validate(&payload_base)?;
        let payload = decode_part::<P>(raw_payload)?;
        let token = Token {
            header,
            payload,
            signature,
        };

        Ok(token)
    }
}

fn validate(base_payload: &BasePayload) -> Result<(), ValidationError> {
    if let Some(exp) = base_payload.exp {
        let difference = Utc::now().timestamp() - exp.timestamp();
        if difference > 0 {
            return Err(ValidationError::Expired {
                seconds: difference as u64,
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::SubsecRound;
    use rand::random;

    fn get_key() -> Key {
        use rand::RngCore;
        let mut bytes = [0; 32];
        rand::thread_rng().fill_bytes(&mut bytes);

        Vec::from(bytes)
    }

    mod at {
        use super::*;

        #[test]
        fn valid() {
            let key = get_key();
            let payload = AccessTokenPayload {
                sub: random(),
                exp: Utc::now().round_subsecs(0) + chrono::Duration::hours(1),
            };
            let token = sign(&key, payload).unwrap();
            let encoded = token.encode().unwrap();
            let decoded = Token::decode(&key, &encoded).unwrap();
            assert_eq!(token, decoded);
        }

        #[test]
        fn expired() {
            let key = get_key();
            let expired_by = chrono::Duration::hours(1);
            let payload = AccessTokenPayload {
                sub: random(),
                exp: Utc::now() - expired_by,
            };
            let token = sign(&key, payload).unwrap();
            let encoded = token.encode().unwrap();
            let err = Token::<AccessTokenPayload>::decode(&key, &encoded).unwrap_err();
            assert_eq!(
                err,
                DecodeError::ValidationError(ValidationError::Expired {
                    seconds: expired_by.num_seconds() as u64
                })
            );
        }

        #[test]
        fn invalid_signature() {
            let valid_key = get_key();
            let invalid_key = get_key();
            let payload = AccessTokenPayload {
                sub: random(),
                exp: Utc::now() - chrono::Duration::hours(1),
            };
            let token = sign(&valid_key, payload).unwrap();
            let encoded = token.encode().unwrap();
            let err = Token::<AccessTokenPayload>::decode(&invalid_key, &encoded).unwrap_err();
            assert_eq!(err, DecodeError::InvalidSignature);
        }
    }

    mod rt {
        use super::*;

        #[test]
        fn valid_with_exp() {
            let key = get_key();
            let payload = RefreshTokenPayload {
                sub: random(),
                exp: Some(Utc::now().round_subsecs(0) + chrono::Duration::hours(1)),
                tid: random(),
            };
            let token = sign(&key, payload).unwrap();
            let encoded = token.encode().unwrap();
            let decoded = Token::decode(&key, &encoded).unwrap();
            assert_eq!(token, decoded);
        }

        #[test]
        fn valid_without_exp() {
            let key = get_key();
            let payload = RefreshTokenPayload {
                sub: random(),
                exp: None,
                tid: random(),
            };
            let token = sign(&key, payload).unwrap();
            let encoded = token.encode().unwrap();
            let decoded = Token::decode(&key, &encoded).unwrap();
            assert_eq!(token, decoded);
        }

        #[test]
        fn expired() {
            let key = get_key();
            let expired_by = chrono::Duration::hours(1);
            let payload = RefreshTokenPayload {
                sub: random(),
                exp: Some(Utc::now() - expired_by),
                tid: random(),
            };
            let token = sign(&key, payload).unwrap();
            let encoded = token.encode().unwrap();
            let err = Token::<RefreshTokenPayload>::decode(&key, &encoded).unwrap_err();
            assert_eq!(
                err,
                DecodeError::ValidationError(ValidationError::Expired {
                    seconds: expired_by.num_seconds() as u64
                })
            );
        }

        #[test]
        fn invalid_signature() {
            let valid_key = get_key();
            let invalid_key = get_key();
            let payload = RefreshTokenPayload {
                sub: random(),
                exp: Some(Utc::now().round_subsecs(0) + chrono::Duration::hours(1)),
                tid: random(),
            };
            let token = sign(&valid_key, payload).unwrap();
            let encoded = token.encode().unwrap();
            let err = Token::<RefreshTokenPayload>::decode(&invalid_key, &encoded).unwrap_err();
            assert_eq!(err, DecodeError::InvalidSignature);
        }
    }
}
