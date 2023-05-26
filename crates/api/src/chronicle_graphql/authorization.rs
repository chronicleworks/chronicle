use base64::Engine;
use cached::{Cached, TimedCache};
use jwtk::jwk::RemoteJwksVerifier;
use reqwest::StatusCode;
use serde_json::{Map, Value};
use std::{sync::Arc, time::Duration};
use thiserror::Error;
use tokio::sync::Mutex;
use tracing::instrument;

use super::{JwksUri, UserInfoUri};

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
    #[error("web access failure: {0}", source)]
    Reqwest {
        #[from]
        source: reqwest::Error,
    },
    #[error("formatting error: {0}", message)]
    Format { message: String },
    #[error("unexpected response: {0} responded with status {1}", server, status)]
    UserInfoResponse { server: String, status: StatusCode },
}

pub struct JwtChecker {
    client: reqwest::Client,
    verifier: RemoteJwksVerifier,
    userinfo_uri: Option<String>,
    userinfo_cache: Arc<Mutex<TimedCache<String, Map<String, Value>>>>,
}

impl JwtChecker {
    #[instrument(level = "debug")]
    pub fn new(
        jwks_uri: &JwksUri,
        userinfo_uri: Option<&UserInfoUri>,
        cache_expiry_seconds: u32,
    ) -> Self {
        Self {
            client: reqwest::Client::new(),
            verifier: RemoteJwksVerifier::new(
                jwks_uri.uri.to_string(),
                None,
                Duration::from_secs(cache_expiry_seconds.into()),
            ),
            userinfo_uri: userinfo_uri.map(|s| s.uri.to_string()),
            userinfo_cache: Arc::new(Mutex::new(TimedCache::with_lifespan(
                cache_expiry_seconds.into(),
            ))),
        }
    }

    #[instrument(level = "trace", skip_all, err)]
    async fn attempt_jwt(&self, token: &str) -> Result<Map<String, Value>, Error> {
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

        self.verifier.verify::<Map<String, Value>>(token).await?;

        if let Value::Object(claims) = serde_json::from_slice(components[1].as_slice())? {
            Ok(claims)
        } else {
            Err(Error::Format {
                message: format!("JWT claims have unexpected format: {:?}", components[1]),
            })
        }
    }

    #[instrument(level = "debug", skip_all, err)]
    pub async fn verify_jwt(&self, token: &str) -> Result<Map<String, Value>, Error> {
        let mut claims = Map::new();
        let mut error = None;
        match self.attempt_jwt(token).await {
            Ok(claims_as_provided) => claims.extend(claims_as_provided),
            error @ Err(Error::Jwks { .. }) => return error, // abort on JWKS verifier failure
            Err(err) => error = Some(err), // could tolerate error from what may be opaque token
        };
        if let Some(userinfo_uri) = &self.userinfo_uri {
            let mut cache = self.userinfo_cache.lock().await;
            if let Some(claims_from_userinfo) = cache.cache_get(&token.to_string()) {
                tracing::trace!("userinfo cache hit");
                error = None;
                claims.extend(claims_from_userinfo.clone());
            } else {
                tracing::trace!("userinfo cache miss");
                drop(cache);
                let request = self
                    .client
                    .get(userinfo_uri)
                    .header("Authorization", format!("Bearer {token}"));
                let response = request.send().await?;
                cache = self.userinfo_cache.lock().await;
                if response.status() == 200 {
                    let response_text = &response.text().await?;
                    if let Ok(claims_from_userinfo) = self.attempt_jwt(response_text).await {
                        error = None;
                        claims.extend(claims_from_userinfo.clone());
                        cache.cache_set(token.to_string(), claims_from_userinfo);
                    } else if let Ok(Value::Object(claims_from_userinfo)) =
                        serde_json::from_str(response_text)
                    {
                        error = None;
                        claims.extend(claims_from_userinfo.clone());
                        cache.cache_set(token.to_string(), claims_from_userinfo);
                    } else {
                        error = Some(Error::Format {
                            message: format!(
                                "UserInfo response has unexpected format: {response_text}"
                            ),
                        })
                    }
                } else if error.is_some() {
                    tracing::trace!("first error before UserInfo was {error:?}");
                    error = Some(Error::UserInfoResponse {
                        server: userinfo_uri.clone(),
                        status: response.status(),
                    });
                } else {
                    cache.cache_set(token.to_string(), Map::new());
                }
            }
        }
        if let Some(error) = error {
            Err(error)
        } else {
            Ok(claims)
        }
    }
}
