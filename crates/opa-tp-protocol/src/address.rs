use hex;
use lazy_static::lazy_static;
use openssl::sha::Sha256;

lazy_static! {
	pub static ref PREFIX: String = {
		let mut sha = Sha256::new();
		sha.update("opa-tp".as_bytes());
		hex::encode(sha.finish())[..6].to_string()
	};
}

pub static VERSION: &str = "1.0";
pub static FAMILY: &str = "opa-tp";

pub fn hash_and_append(addr: impl AsRef<str>) -> String {
	let mut sha = Sha256::new();
	sha.update(addr.as_ref().as_bytes());
	format!("{}{}", &*PREFIX, hex::encode(sha.finish()))
}

pub trait HasSawtoothAddress {
	fn get_address(&self) -> String;
}
