use std::net::{SocketAddr, ToSocketAddrs};

use clap::{Arg, Command};
use http::{HeaderValue, StatusCode};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use serde_json::{json, Value};
use tungstenite::{client::IntoClientRequest, connect, Message};

fn main() -> Result<(), anyhow::Error> {
	let args = Command::new("gq-ws")
		.author("Blockchain Technology Partners")
		.about("Perform GraphQL subscription to a websocket")
		.arg(
			Arg::new("request")
				.long("subscription")
				.short('s')
				.takes_value(true)
				.required(true)
				.help("the GraphQL subscription request"),
		)
		.arg(
			Arg::new("count")
				.long("notification-count")
				.short('c')
				.takes_value(true)
				.required(true)
				.help("how many responses to report"),
		)
		.arg(
			Arg::new("address")
				.long("chronicle-address")
				.short('a')
				.takes_value(true)
				.default_value("localhost:9982")
				.help("the network address of the Chronicle API"),
		)
		.arg(
			Arg::new("token")
				.long("bearer-token")
				.short('t')
				.takes_value(true)
				.help("the bearer token to pass for authorization"),
		)
		.get_matches();

	let subscription_query = args.value_of("request").unwrap();
	let notification_count: u32 = args.value_of("count").unwrap().parse()?;
	let chronicle_address: SocketAddr = args
		.value_of("address")
		.unwrap()
		.to_socket_addrs()?
		.next()
		.expect("network address required for Chronicle API");
	let bearer_token = args.value_of("token");

	// generate random ID for subscription
	let subscription_id: String =
		thread_rng().sample_iter(&Alphanumeric).take(12).map(char::from).collect();

	// prepare websocket request
	let mut client_request = format!("ws://{chronicle_address}/ws").into_client_request()?;
	let headers = client_request.headers_mut();
	if let Some(token) = bearer_token {
		headers.insert("Authorization", HeaderValue::from_str(&format!("Bearer {token}"))?);
	}
	headers.insert("Sec-WebSocket-Protocol", HeaderValue::from_str("graphql-ws")?);

	// connect and upgrade websocket
	let (mut socket, response) = connect(client_request)?;
	if response.status() != StatusCode::SWITCHING_PROTOCOLS {
		panic!("failed connect and upgrade: {response:#?}");
	}

	// initialize gql connection
	let conn_init_json = json!({
		"type": "connection_init"
	});
	let conn_init_msg = Message::Text(serde_json::to_string(&conn_init_json)?);
	socket.send(conn_init_msg)?;
	let conn_response = socket.read()?;
	if let Value::Object(map) = serde_json::from_str::<Value>(&conn_response.clone().into_text()?)?
	{
		if map.get("type") == Some(&Value::String("connection_ack".to_string())) {
			// connection initialized, so subscribe
			let subscription_json = json!({
				"type": "start",
				"id": subscription_id,
				"payload": {
					"query": subscription_query
				}
			});
			let subscription_msg = Message::Text(serde_json::to_string(&subscription_json)?);
			socket.send(subscription_msg)?;

			// receive and print notifications
			let data_json = Value::String("data".to_string());
			let subscription_id_json = Value::String(subscription_id);
			let mut remaining = notification_count;
			while remaining > 0 {
				remaining -= 1;
				let notification_msg = socket.read()?;
				let notification_json =
					serde_json::from_str::<Value>(&notification_msg.into_text()?)?;

				if let Value::Object(map) = notification_json.clone() {
					if map.get("type") == Some(&data_json) &&
						map.get("id") == Some(&subscription_id_json)
					{
						let notification_pretty =
							serde_json::to_string_pretty(map.get("payload").unwrap())?;
						println!("{notification_pretty}");
					} else {
						panic!("expected a response to subscription, got: {notification_json}");
					}
				} else {
					panic!("expected a JSON object notification, got: {notification_json}");
				}
			}
			return Ok(());
		}
	}
	panic!("expected acknowledgement of connection initialization, got: {conn_response}");
}
