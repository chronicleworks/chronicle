use std::net::ToSocketAddrs;
use std::sync::Arc;

use common::models::ProvModel;
use parking_lot::RwLock;

use async_change_tracker::ChangeTracker;
use bui_backend::highlevel::{create_bui_app_inner, BuiAppInner};
use bui_backend::AccessControl;
use bui_backend_types::CallbackDataAndSession;

use crate::ApiCommand;

#[derive(Debug)]
pub struct BuiError {
    kind: ErrorKind,
}

#[derive(Debug)]
enum ErrorKind {
    BuiBackend(bui_backend::Error),
    Raw(String),
}

impl From<ErrorKind> for BuiError {
    fn from(kind: ErrorKind) -> Self {
        Self { kind }
    }
}

impl From<bui_backend::Error> for BuiError {
    fn from(orig: bui_backend::Error) -> Self {
        let kind = ErrorKind::BuiBackend(orig);
        Self { kind }
    }
}

// Include the files to be served and define `fn get_default_config()`.
include!(concat!(env!("OUT_DIR"), "/public.rs")); // Despite slash, this works on Windows.

/// The structure that holds our app data
struct MyApp {
    inner: BuiAppInner<ProvModel, ApiCommand>,
}

fn address(address: &str) -> std::net::SocketAddr {
    address.to_socket_addrs().unwrap().next().unwrap()
}

fn is_loopback(addr_any: &std::net::SocketAddr) -> bool {
    match addr_any {
        &std::net::SocketAddr::V4(addr) => addr.ip().is_loopback(),
        &std::net::SocketAddr::V6(addr) => addr.ip().is_loopback(),
    }
}

impl MyApp {
    /// Create our app
    async fn new(auth: AccessControl, config: Config) -> Result<Self, BuiError> {
        // Create our shared state.
        let shared_store = Arc::new(RwLock::new(ChangeTracker::new(ProvModel::default())));

        let chan_size = 10;
        let (rx_conn, bui_server) =
            bui_backend::lowlevel::launcher(config, &auth, chan_size, "/events", None);

        let handle = tokio::runtime::Handle::current();

        // Create `inner`, which takes care of the browser communication details for us.
        let (_, mut inner) = create_bui_app_inner(
            handle,
            None,
            &auth,
            shared_store,
            Some("bui_backend".to_string()),
            rx_conn,
            bui_server,
        )
        .await?;

        // Make a clone of our shared state Arc which will be moved into our callback handler.
        let _tracker_arc2 = inner.shared_arc().clone();

        // Create a Stream to handle callbacks from clients.
        inner.set_callback_listener(Box::new(move |_msg: CallbackDataAndSession<ApiCommand>| {
            futures::future::ok(())
        }));

        // Return our app.
        Ok(MyApp { inner })
    }
}

pub async fn serve_ui(addr: &str) -> Result<(), BuiError> {
    let http_server_addr = address(addr);

    // Get our JWT secret.
    let _required = !is_loopback(&http_server_addr);
    let secret = vec![];

    let auth = if http_server_addr.ip().is_loopback() {
        AccessControl::Insecure(http_server_addr)
    } else {
        bui_backend::highlevel::generate_random_auth(http_server_addr, secret)?
    };

    let my_app = MyApp::new(auth, get_default_config()).await?;

    // Clone our shared data to move it into a closure later.
    let _tracker_arc = my_app.inner.shared_arc().clone();

    // Create a stream to call our closure every second.
    let mut interval_stream = tokio::time::interval(std::time::Duration::from_millis(1000));

    let stream_future = async move {
        loop {
            interval_stream.tick().await;
        }
    };

    let maybe_url = my_app.inner.guess_url_with_token();
    println!(
        "Depending on IP address resolution, you may be able to login \
        with this url: {}",
        maybe_url
    );

    // Run our app.
    stream_future.await;

    Ok(())
}
