mod mockchain;
mod stubstrate;

use clap::ArgMatches;

use k256::{
	pkcs8::{EncodePrivateKey, LineEnding},
	SecretKey,
};

use rand::rngs::StdRng;
use rand_core::SeedableRng;

use serde_json::{self, Value};

use std::io::Write;
use tempfile::{NamedTempFile, TempDir};

use crate::{cli, dispatch_args};

use self::stubstrate::Stubstrate;

fn get_opactl_cmd(command_line: &str) -> ArgMatches {
	let cli = cli::cli();
	cli.get_matches_from(command_line.split_whitespace())
}

fn key_from_seed(seed: u8) -> String {
	let secret: SecretKey = SecretKey::random(StdRng::from_seed([seed; 32]));
	secret.to_pkcs8_pem(LineEnding::CRLF).unwrap().to_string()
}

// Cli should automatically create ephemeral batcher keys, but we need to supply named keyfiles
// in a temp directory
async fn bootstrap_root_state() -> (String, Stubstrate, TempDir) {
	let root_key = key_from_seed(0);

	let keystore = tempfile::tempdir().unwrap();
	let keyfile_path = keystore.path().join("./opa-pk");
	std::fs::write(&keyfile_path, root_key.as_bytes()).unwrap();

	let matches = get_opactl_cmd(&format!(
		"opactl --batcher-key-generated --keystore-path {} bootstrap",
		keystore.path().display()
	));

	let stubstrate = Stubstrate::new();

	tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
	dispatch_args(matches, &stubstrate).await.unwrap();

	(root_key, stubstrate, keystore)
}

#[tokio::test]
async fn bootstrap_root_and_get_key() {
	let (_root_key, opa_tp, _keystore) = bootstrap_root_state().await;
	//Generate a key pem and set env vars
	insta::assert_yaml_snapshot!(opa_tp.stored_keys(), {
            ".**.date" => "[date]",
            ".**.key" => "[pem]",
        } ,@r###"
 ---
 - id: root
   current:
     key: "[pem]"
     version: 0
   expired: ~
 "###);

	tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

	let out_keyfile = NamedTempFile::new().unwrap();

	let matches = get_opactl_cmd(
		format!("opactl get-key --output {}", out_keyfile.path().display(),).as_str(),
	);

	insta::assert_yaml_snapshot!(
        dispatch_args(matches, &opa_tp)
            .await
            .unwrap(), @r###"
        ---
        NoWait
        "###);
}

#[tokio::test]
async fn rotate_root() {
	let (_root_key, opa_tp, keystore) = bootstrap_root_state().await;

	let new_root_key = key_from_seed(1);

	let keyfile_path = keystore.path().join("./new-root-1");
	std::fs::write(&keyfile_path, new_root_key.as_bytes()).unwrap();

	let matches = get_opactl_cmd(
            format!(
                "opactl --batcher-key-generated --opa-key-from-path --keystore-path {} rotate-root  --new-root-key new-root-1",
                keystore.path().display(),
            )
            .as_str(),
        );

	insta::assert_yaml_snapshot!(
        dispatch_args(matches, &opa_tp)
            .await
            .unwrap(), {
            ".**.date" => "[date]",
            ".**.key" => "[pem]",
            ".**.correlation_id" => "[correlation_id]"
        } ,@r###"
 ---
 WaitedAndFound:
   KeyUpdate:
     keys:
       id: root
       current:
         key: "[pem]"
         version: 1
       expired:
         key: "[pem]"
         version: 0
     correlation_id: "[correlation_id]"
 "###);

	insta::assert_yaml_snapshot!(opa_tp.stored_keys(),{
            ".**.date" => "[date]",
            ".**.key" => "[pem]",
            ".**.correlation_id" => "[correlation_id]"
        }, @r###"
        ---
        - - 7ed19313e8ece6c4f5551b9bd1090797ad25c6d85f7b523b2214d4fe448372279aa95c
          - current:
              key: "[pem]"
              version: 1
            expired:
              key: "[pem]"
              version: 0
            id: root
        "###);
}

#[tokio::test]
async fn register_and_rotate_key() {
	let (_root_key, opa_tp, keystore) = bootstrap_root_state().await;

	let new_key = key_from_seed(1);

	let keyfile_path = keystore.path().join("./new-key-1");
	std::fs::write(&keyfile_path, new_key.as_bytes()).unwrap();

	let matches = get_opactl_cmd(
            format!(
                "opactl --batcher-key-generated --keystore-path {} register-key  --new-key new-key-1 --id test",
                keystore.path().display(),
            )
            .as_str(),
        );

	insta::assert_yaml_snapshot!(
        dispatch_args(matches,&opa_tp)
            .await
            .unwrap(), {
            ".**.date" => "[date]",
            ".**.key" => "[pem]",
            ".**.correlation_id" => "[correlation_id]"
        },@r###"
 ---
 WaitedAndFound:
   KeyUpdate:
     keys:
       id: test
       current:
         key: "[pem]"
         version: 0
       expired: ~
     correlation_id: "[correlation_id]"
 "###);

	insta::assert_yaml_snapshot!(opa_tp.stored_keys(), {
            ".**.date" => "[date]",
            ".**.key" => "[pem]",
            ".**.correlation_id" => "[correlation_id]"
        }, @r###"
        ---
        - - 7ed19313e8ece6c4f5551b9bd1090797ad25c6d85f7b523b2214d4fe448372279aa95c
          - current:
              key: "[pem]"
              version: 0
            expired: ~
            id: root
        - - 7ed19336d8b5677c39a7b872910f948944dd84ba014846c81fcd53fe1fd5289b9dfd1c
          - current:
              key: "[pem]"
              version: 0
            expired: ~
            id: test
        "###);

	let new_key_2 = key_from_seed(1);

	let keyfile_path = keystore.path().join("./new-key-2");
	std::fs::write(&keyfile_path, new_key_2.as_bytes()).unwrap();

	let matches = get_opactl_cmd(
            format!(
                "opactl --batcher-key-generated --keystore-path {} rotate-key  --current-key new-key-1 --new-key new-key-2 --id test",
                keystore.path().display(),
            )
            .as_str(),
        );

	insta::assert_yaml_snapshot!(
        dispatch_args(matches, &opa_tp)
            .await
            .unwrap(), {
            ".**.date" => "[date]",
            ".**.key" => "[pem]",
            ".**.correlation_id" => "[correlation_id]"
        } ,@r###"
        ---
        WaitedAndFound:
          KeyUpdate:
            id: test
            current:
              key: "[pem]"
              version: 1
            expired:
              key: "[pem]"
              version: 0
        "###);

	insta::assert_yaml_snapshot!(opa_tp.stored_keys(), {
            ".**.date" => "[date]",
            ".**.key" => "[pem]",
            ".**.correlation_id" => "[correlation_id]"
        } ,@r###"
        ---
        - - 7ed19313e8ece6c4f5551b9bd1090797ad25c6d85f7b523b2214d4fe448372279aa95c
          - current:
              key: "[pem]"
              version: 0
            expired: ~
            id: root
        - - 7ed19336d8b5677c39a7b872910f948944dd84ba014846c81fcd53fe1fd5289b9dfd1c
          - current:
              key: "[pem]"
              version: 1
            expired:
              key: "[pem]"
              version: 0
            id: test
        "###);
}

#[tokio::test]
async fn set_and_update_policy() {
	let (root_key, opa_tp, keystore) = bootstrap_root_state().await;

	let mut root_keyfile = NamedTempFile::new().unwrap();
	root_keyfile.write_all(root_key.as_bytes()).unwrap();

	let mut policy = NamedTempFile::new().unwrap();
	policy.write_all(&[0]).unwrap();

	let matches = get_opactl_cmd(
		format!(
			"opactl --batcher-key-generated --keystore-path {} set-policy  --id test  --policy {}",
			keystore.path().display(),
			policy.path().display()
		)
		.as_str(),
	);

	insta::assert_yaml_snapshot!(dispatch_args(
            matches,
            &opa_tp,
        )
        .await
        .unwrap(), {
          ".**.date" => "[date]",
          ".**.correlation_id" => "[correlation_id]",
        }, @r###"
 ---
 WaitedAndFound:
   PolicyUpdate:
     - id: test
       hash:
         - 112
         - 37
         - 224
         - 117
         - 213
         - 226
         - 246
         - 205
         - 227
         - 204
         - 5
         - 26
         - 49
         - 240
         - 118
         - 96
       policy_address:
         - 57
         - 55
         - 86
         - 28
         - 76
         - 12
         - 167
         - 194
         - 100
         - 141
         - 152
         - 29
         - 77
         - 122
         - 58
         - 230
     - - 126
       - 73
       - 89
       - 163
       - 235
       - 197
       - 73
       - 130
       - 154
       - 213
       - 245
       - 47
       - 249
       - 40
       - 118
       - 225
 "###);

	insta::assert_yaml_snapshot!(opa_tp.stored_policy(), {
          ".**.date" => "[date]",
          ".**.key" => "[pem]",
          ".**.correlation_id" => "[correlation_id]",
        } ,@r###"
        ---
        - - 7ed19313e8ece6c4f5551b9bd1090797ad25c6d85f7b523b2214d4fe448372279aa95c
          - current:
              key: "[pem]"
              version: 0
            expired: ~
            id: root
        - - 7ed1931c262a4be700b69974438a35ae56a07ce96778b276c8a061dc254d9862c7ecff
          - - 0
        - - 7ed1932b35db049f40833c5c2eaa47e070ce2648c478469a4cdf44ff7a37dd5468208e
          - hash: 6e340b9cffb37a989ca544e6bb780a2c78901d3fb33738768511a30617afa01d
            id: test
            policy_address: 7ed1931c262a4be700b69974438a35ae56a07ce96778b276c8a061dc254d9862c7ecff
        "###);

	policy.write_all(&[1]).unwrap();

	let matches = get_opactl_cmd(
		format!(
			"opactl --batcher-key-generated --keystore-path {} set-policy  --id test  --policy {}",
			keystore.path().display(),
			policy.path().display()
		)
		.as_str(),
	);

	insta::assert_yaml_snapshot!(dispatch_args(matches, &opa_tp)
            .await
            .unwrap(), {
              ".**.date" => "[date]",
              ".**.correlation_id" => "[correlation_id]",
            }, @r###"
        ---
        WaitedAndFound:
          PolicyUpdate:
            id: test
            hash: b413f47d13ee2fe6c845b2ee141af81de858df4ec549a58b7970bb96645bc8d2
            policy_address: 7ed1931c262a4be700b69974438a35ae56a07ce96778b276c8a061dc254d9862c7ecff
        "### );

	insta::assert_yaml_snapshot!(opa_tp.stored_policy(), {
          ".**.date" => "[date]",
          ".**.key" => "[pem]",
        } ,@r###"
        ---
        - - 7ed19313e8ece6c4f5551b9bd1090797ad25c6d85f7b523b2214d4fe448372279aa95c
          - current:
              key: "[pem]"
              version: 0
            expired: ~
            id: root
        - - 7ed1931c262a4be700b69974438a35ae56a07ce96778b276c8a061dc254d9862c7ecff
          - - 0
            - 1
        - - 7ed1932b35db049f40833c5c2eaa47e070ce2648c478469a4cdf44ff7a37dd5468208e
          - hash: b413f47d13ee2fe6c845b2ee141af81de858df4ec549a58b7970bb96645bc8d2
            id: test
            policy_address: 7ed1931c262a4be700b69974438a35ae56a07ce96778b276c8a061dc254d9862c7ecff
     - 230
 "###);
}
