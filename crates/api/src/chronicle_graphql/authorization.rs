use base64::Engine;
use jwtk::jwk::RemoteJwksVerifier;
use serde_json::Value;
use std::time::Duration;
use thiserror::Error;
use tracing::instrument;
use url::Url;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Base64 decoding failure: {0}", source)]
    Base64 {
        #[from]
        source: base64::DecodeError,
    },
    #[error("JSON decoding failure: {0}", source)]
    Json {
        #[from]
        source: serde_json::Error,
    },
    #[error("JWT validation failure: {0}", source)]
    Jwks {
        #[from]
        source: jwtk::Error,
    },
    #[error("formatting error: {0}", message)]
    Format { message: String },
}

pub struct JwtChecker {
    verifier: RemoteJwksVerifier,
}

impl JwtChecker {
    pub fn new(jwks_uri: &Url) -> Self {
        Self {
            verifier: RemoteJwksVerifier::new(jwks_uri.to_string(), None, Duration::from_secs(100)),
        }
    }

    #[instrument(skip(self), ret(Debug))]
    pub async fn verify_jwt(
        &self,
        token: &str,
    ) -> Result<serde_json::map::Map<String, Value>, Error> {
        let base64_engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;

        // JWT is composed of three base64-encoded components
        let components = token
            .split('.')
            .map(|component| base64_engine.decode(component))
            .collect::<Result<Vec<Vec<u8>>, base64::DecodeError>>()?;
        if components.len() != 3 {
            return Err(Error::Format {
                message: format!("JWT has unexpected format: {token}"),
            });
        };

        self.verifier
            .verify::<serde_json::map::Map<String, Value>>(token)
            .await?;

        if let Value::Object(claims) = serde_json::from_slice(components[1].as_slice())? {
            Ok(claims)
        } else {
            Err(Error::Format {
                message: format!("JWT claims have unexpected format: {:?}", components[1]),
            })
        }
    }
}
