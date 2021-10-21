use custom_error::custom_error;
use k256::{
    ecdsa::SigningKey,
    pkcs8::{FromPrivateKey, FromPublicKey},
    PublicKey,
};

use std::{path::Path, string::FromUtf8Error};

custom_error! {pub SignerError
    Io{source: std::io::Error}                              = "Invalid key store directory",
    Glob{source: glob::GlobError}                           = "Invalid glob ",
    Pattern{source: glob::PatternError}                     = "Invalid glob ",
    Encoding{source: FromUtf8Error}                         = "Invalid file encoding",
    InvalidPublicKey{source: k256::pkcs8::Error}            = "Invalid public key",
    NoPublicKeyFound{}                                      = "No public key found",
    NoPrivateKeyFound{}                                     = "No private key found",
}

pub struct DirectoryStoredKeys {
    public: PublicKey,
    signing: SigningKey,
}

impl DirectoryStoredKeys {
    pub fn new<P>(path: P) -> Result<Self, SignerError>
    where
        P: AsRef<Path>,
    {
        let public = glob::glob(&format!("{}/*.pub.pem", path.as_ref().to_string_lossy()))?.nth(0);

        let public =
            PublicKey::read_public_key_pem_file(public.ok_or(SignerError::NoPublicKeyFound {})??)?;

        let private =
            glob::glob(&format!("{}/*.priv.pem", path.as_ref().to_string_lossy()))?.nth(0);

        let signing =
            SigningKey::read_pkcs8_pem_file(private.ok_or(SignerError::NoPrivateKeyFound {})??)?;

        Ok(Self { public, signing })
    }
}
