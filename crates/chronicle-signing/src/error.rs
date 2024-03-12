use thiserror::Error;

#[derive(Error, Debug)]
pub enum SecretError {
	#[error("Invalid public key")]
	InvalidPublicKey,
	#[error("Invalid private key")]
	InvalidPrivateKey,
	#[error("No public key found")]
	NoPublicKeyFound,
	#[error("No private key found")]
	NoPrivateKeyFound,

	#[error("Vault {source}")]
	SecretVault {
		#[from]
		#[source]
		source: anyhow::Error,
	},
}
