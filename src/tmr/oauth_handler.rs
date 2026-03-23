use std::{net::SocketAddr, sync::Arc};

use async_trait::async_trait;
use axum::{
    Router,
    extract::{Query, State},
    response::Html,
    routing::get,
};
use serde::Deserialize;
use tokio::{
    net::TcpListener,
    sync::{Mutex, oneshot},
};
use tracing::{debug, error, info};

use crate::tmr::result::TmrConnectError;

const CALLBACK_HTML: &str = include_str!("res/default_callback.html");

#[async_trait]
pub trait AuthCallbackHandler {
    async fn new() -> Result<Box<Self>, TmrConnectError>;
    fn get_listen_addr(&self) -> &str;
    async fn wait_for_callback(self, auth_url: &str) -> Result<AuthCallback, TmrConnectError>;
}

pub struct DefaultAuthCallbackHandler {
    listener: TcpListener,
    listen_addr: String,
}

#[derive(Clone)]
struct AppState {
    code_sender: Arc<Mutex<Option<oneshot::Sender<AuthCallback>>>>,
}

#[derive(Debug, Deserialize)]
pub struct AuthCallback {
    pub code: String,
    pub state: String,
}

async fn callback_handler(
    Query(params): Query<AuthCallback>,
    State(state): State<AppState>,
) -> Html<String> {
    debug!("Received callback: {params:?}");
    if let Some(sender) = state.code_sender.lock().await.take() {
        let _ = sender.send(params);
    }
    Html(CALLBACK_HTML.to_string())
}

#[async_trait]
impl AuthCallbackHandler for DefaultAuthCallbackHandler {
    async fn new() -> Result<Box<Self>, TmrConnectError> {
        // 0 means to bind to a random available port
        let addr = SocketAddr::from(([127, 0, 0, 1], 0));
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| TmrConnectError::AuthError {
                msg: format!("Failed to bind callback server: {e}"),
                source: Some(e.into()),
            })?;
        let addr = listener
            .local_addr()
            .map_err(|e| TmrConnectError::AuthError {
                msg: format!("Failed to get address of callback server: {e}"),
                source: Some(e.into()),
            })?;
        // The MCP server does not like an IP as the host in the callback server (HTTP/2 403)
        let listen_addr = format!("http://localhost:{}/callback", addr.port());

        Ok(Box::new(DefaultAuthCallbackHandler {
            listener,
            listen_addr,
        }))
    }

    fn get_listen_addr(&self) -> &str {
        &self.listen_addr
    }

    async fn wait_for_callback(self, auth_url: &str) -> Result<AuthCallback, TmrConnectError> {
        let (code_sender, code_receiver) = oneshot::channel::<AuthCallback>();

        let app_state = AppState {
            code_sender: Arc::new(Mutex::new(Some(code_sender))),
        };

        let app = Router::new()
            .route("/callback", get(callback_handler))
            .with_state(app_state);

        info!("Listening for callback at: http://{}", self.listen_addr);

        let listener = self.listener;
        tokio::spawn(async move {
            let result = axum::serve(listener, app).await;

            if let Err(e) = result {
                error!("Callback server error: {e}");
            }
        });

        info!("Opening authorization page in browser: {auth_url}");
        webbrowser::open(auth_url).ok();
        info!("Waiting for browser callback");
        let params = code_receiver
            .await
            .map_err(|e| TmrConnectError::AuthError {
                msg: format!("Failed to receive callback parameters: {e}"),
                source: Some(e.into()),
            })?;
        Ok(params)
    }
}
