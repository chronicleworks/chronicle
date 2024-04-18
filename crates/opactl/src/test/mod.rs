mod mockchain;
mod stubstrate;

use clap::ArgMatches;

use k256::{
	pkcs8::{EncodePrivateKey, LineEnding},
	SecretKey,
};

use rand::rngs::StdRng;
use rand_core::SeedableRng;

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


//TODO: downloads
#[tokio::test]
#[ignore]
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
 - id: root
   current:
     key: "[pem]"
     version: 1
   expired:
     key: "[pem]"
     version: 0
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
 - id: test
   current:
     key: "[pem]"
     version: 0
   expired: ~
 - id: root
   current:
     key: "[pem]"
     version: 0
   expired: ~
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
     keys:
       id: test
       current:
         key: "[pem]"
         version: 1
       expired:
         key: "[pem]"
         version: 0
     correlation_id: "[correlation_id]"
 "###);

	insta::assert_yaml_snapshot!(opa_tp.stored_keys(), {
            ".**.date" => "[date]",
            ".**.key" => "[pem]",
            ".**.correlation_id" => "[correlation_id]"
        } ,@r###"
 ---
 - id: test
   current:
     key: "[pem]"
     version: 1
   expired:
     key: "[pem]"
     version: 0
 - id: root
   current:
     key: "[pem]"
     version: 0
   expired: ~
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
     policy:
       id: test
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
     correlation_id: "[correlation_id]"
 "###);

	insta::assert_yaml_snapshot!(opa_tp.stored_policy(), {
          ".**.date" => "[date]",
          ".**.key" => "[pem]",
          ".**.correlation_id" => "[correlation_id]",
        } ,@r###"
 ---
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
     policy:
       id: test
       hash:
         - 15
         - 22
         - 254
         - 183
         - 126
         - 60
         - 18
         - 155
         - 216
         - 189
         - 76
         - 67
         - 159
         - 18
         - 235
         - 209
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
     correlation_id: "[correlation_id]"
 "### );

	insta::assert_yaml_snapshot!(opa_tp.stored_policy(), {
          ".**.date" => "[date]",
          ".**.key" => "[pem]",
        } ,@r###"
 ---
 - id: test
   hash:
     - 15
     - 22
     - 254
     - 183
     - 126
     - 60
     - 18
     - 155
     - 216
     - 189
     - 76
     - 67
     - 159
     - 18
     - 235
     - 209
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
 "###);
}
