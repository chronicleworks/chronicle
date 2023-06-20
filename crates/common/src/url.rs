use std::{fs::File, io::Read, path::PathBuf};

use thiserror::Error;
use url::Url;

#[derive(Error, Debug)]
pub enum FromUrlError {
    #[error("HTTP error while attempting to read from URL: {0}")]
    HTTP(#[from] reqwest::Error),

    #[error("Invalid URL scheme: {0}")]
    InvalidUrlScheme(String),

    #[error("IO error while attempting to read from URL: {0}")]
    IO(#[from] std::io::Error),
}

pub enum PathOrUrl {
    File(PathBuf),
    Url(Url),
}

pub async fn load_bytes_from_url(url: &str) -> Result<Vec<u8>, FromUrlError> {
    let path_or_url = match url.parse::<Url>() {
        Ok(url) => PathOrUrl::Url(url),
        Err(_) => PathOrUrl::File(PathBuf::from(url)),
    };

    let content = match path_or_url {
        PathOrUrl::File(path) => {
            let mut file = File::open(path)?;
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)?;
            Ok(buf)
        }
        PathOrUrl::Url(url) => match url.scheme() {
            "file" => {
                let mut file = File::open(url.path())?;
                let mut buf = Vec::new();
                file.read_to_end(&mut buf)?;
                Ok(buf)
            }
            "http" | "https" => Ok(reqwest::get(url).await?.bytes().await?.into()),
            _ => Err(FromUrlError::InvalidUrlScheme(url.scheme().to_owned())),
        },
    }?;

    Ok(content)
}
