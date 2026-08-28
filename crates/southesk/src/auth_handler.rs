// Copyright 2026 Thomas Axelsson
// SPDX-License-Identifier: MIT

//! (Interactive) authentication handlers for the OAuth flow, use to
//! authenticate the client with the MCP server.

use std::{net::SocketAddr, sync::Arc, time::Duration};

use async_trait::async_trait;
use axum::{
    Router,
    extract::{Query, State},
    response::Html,
    routing::get,
};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
    sync::{Mutex, oneshot},
    time::timeout,
};
use tracing::{debug, error, info};

const BROWSER_CALLBACK_HTML: &str = include_str!("res/default_callback.html");

/// OAuth callback handler that prompts the user to authenticate, typically by
/// opening a URL and waiting for a callback.
#[async_trait]
pub trait AuthHandler: Debug {
    /// The URL that the OAuth server will redirect to after authentication. It
    /// is passed as part of the OAuth authorization request.
    fn redirect_uri(&self) -> &str;
    /// Requests the user to authenticate by visiting the given `auth_url` and
    /// waits for the OAuth callback to be received.
    async fn authenticate(&self, auth_url: &str) -> Result<AuthGrant, AuthFlowError>;
}

#[derive(Debug, thiserror::Error)]
#[error("initializing authentication handler failed: {msg}")]
pub struct AuthInitError {
    msg: String,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

#[derive(Debug, thiserror::Error)]
pub enum AuthFlowError {
    /// The data provided in the server callback, or the callback URL itself,
    /// was malformed or incomplete
    #[error("callback response malformed or incomplete: {msg}")]
    BadResponse {
        msg: String,
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    /// Authentication was aborted. For example by a timeout or the user
    /// canceling the authentication
    #[error("authentication aborted: {msg}")]
    Aborted {
        msg: String,
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    /// An internal error. For example, failure to launch the browser or to read
    /// user input from the console.
    #[error("authentication process failed with internal error: {msg}")]
    Internal {
        msg: String,
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

/// The parameters received in the OAuth callback URL, containing the
/// authorization code and state (CSRF token).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthGrant {
    /// The authorization code to exchange for an access token
    pub code: String,
    /// CSRF token sent by the client and now returned by the server
    pub state: String,
    /// The issuer of the authorization server, returned by the server in the
    /// callback URL.
    pub iss: String,
}

/// An OAuth callback handler that opens the user's web browser and listens for
/// the callback on a local HTTP server.
#[derive(Debug)]
pub struct BrowserAuth {
    listen_addr: String,
    cb_tx: Arc<Mutex<Option<oneshot::Sender<AuthGrant>>>>,
    auth_timeout: Duration,
}

#[derive(Clone)]
struct BrowserAuthAppState {
    cb_tx: Arc<Mutex<Option<oneshot::Sender<AuthGrant>>>>,
}

async fn browser_callback_handler(
    Query(params): Query<AuthGrant>,
    State(state): State<BrowserAuthAppState>,
) -> Html<String> {
    debug!("Received callback: {params:?}");
    // Anyone waiting for the value?
    if let Some(cb_tx) = state.cb_tx.lock().await.take() {
        cb_tx.send(params).ok();
    }
    Html(BROWSER_CALLBACK_HTML.to_string())
}

impl BrowserAuth {
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_mins(2);

    /// Creates a new browser authentication handler and starts a callback server.
    ///
    /// The callback server listens on a random available port on localhost.
    pub async fn new() -> Result<Self, AuthInitError> {
        // 0 means to bind to a random available port
        let addr = SocketAddr::from(([127, 0, 0, 1], 0));
        let listener = TcpListener::bind(addr).await.map_err(|e| AuthInitError {
            msg: "failed to bind callback server to a random port".to_string(),
            source: Some(e.into()),
        })?;
        let addr = listener.local_addr().map_err(|e| AuthInitError {
            msg: "failed to get address of callback server".to_string(),
            source: Some(e.into()),
        })?;
        // The MCP server does not like an IP as the host in the callback server (HTTP/2 403)
        let listen_addr = format!("http://localhost:{}/callback", addr.port());

        // let (cb_tx, cb_rx) = watch::channel::<Option<oneshot::Sender<AuthGrant>>>(None);
        let cb_rx = Arc::new(Mutex::new(None));

        let app_state = BrowserAuthAppState {
            cb_tx: cb_rx.clone(),
        };

        let app = Router::new()
            .route("/callback", get(browser_callback_handler))
            .with_state(app_state);

        info!("Listening for callback at: {}", listen_addr);

        tokio::spawn(async move {
            let result = axum::serve(listener, app).await;

            if let Err(e) = result {
                error!("callback server error: {e}");
            }
        });

        Ok(BrowserAuth {
            listen_addr,
            cb_tx: cb_rx,
            auth_timeout: Self::DEFAULT_TIMEOUT,
        })
    }

    /// Sets the timeout for the user to complete authentication.
    ///
    /// Defaults to [`DEFAULT_TIMEOUT`](Self::DEFAULT_TIMEOUT). Note that the
    /// Montrose web page login times out after 1 minute, but the user can wait
    /// longer before approving the MCP client.
    #[must_use]
    pub fn with_timeout(self, timeout: Duration) -> Self {
        Self {
            auth_timeout: timeout,
            ..self
        }
    }
}

#[async_trait]
impl AuthHandler for BrowserAuth {
    fn redirect_uri(&self) -> &str {
        &self.listen_addr
    }

    async fn authenticate(&self, auth_url: &str) -> Result<AuthGrant, AuthFlowError> {
        let (cb_tx, cb_rx) = oneshot::channel();
        *self.cb_tx.lock().await = Some(cb_tx);
        info!("Opening authorization page in browser: {auth_url}");
        webbrowser::open(auth_url).map_err(|e| AuthFlowError::Internal {
            msg: "failed to open the authentication URL in a web browser".to_string(),
            source: Some(e.into()),
        })?;
        info!(
            "Waiting {} seconds for browser callback",
            self.auth_timeout.as_secs()
        );
        let auth_result =
            timeout(self.auth_timeout, cb_rx)
                .await
                .map_err(|_| AuthFlowError::Aborted {
                    msg: "timeout while waiting for the user to authenticate".to_string(),
                    source: None,
                })?;
        let params = auth_result.map_err(|e| AuthFlowError::Internal {
            msg: "failed to receive parameters over callback channel".to_string(),
            source: Some(e.into()),
        })?;
        info!("Authentication completed.");
        Ok(params)
    }
}

/// An OAuth callback handler that works in a console.
///
/// It prints the authorization URL and waits for the user to paste the redirect
/// URL.
#[derive(Debug)]
pub struct ConsoleAuth {}

impl Default for ConsoleAuth {
    fn default() -> Self {
        Self::new()
    }
}

impl ConsoleAuth {
    const REDIRECT_BASE_URL: &str = "http://localhost:7878/callback";

    /// Creates a new console authentication callback handler.
    #[must_use]
    pub fn new() -> Self {
        ConsoleAuth {}
    }
}

#[async_trait]
impl AuthHandler for ConsoleAuth {
    fn redirect_uri(&self) -> &str {
        Self::REDIRECT_BASE_URL
    }

    async fn authenticate(&self, auth_url: &str) -> Result<AuthGrant, AuthFlowError> {
        eprintln!("\nOpen the following URL in your browser to authenticate:\n");
        eprintln!("  {auth_url}\n");
        eprintln!("After completing authentication, your browser will redirect to a URL");
        eprintln!(
            "starting with '{}?...' (the page won't load).",
            Self::REDIRECT_BASE_URL
        );
        eprint!("\nPaste that full redirect URL here: ");
        tokio::io::stdout().flush().await.ok();

        let mut line = String::new();
        BufReader::new(tokio::io::stdin())
            .read_line(&mut line)
            .await
            .map_err(|e| AuthFlowError::Internal {
                msg: "failed to read redirect URL from stdin".to_string(),
                source: Some(e.into()),
            })?;
        let redirect_url = line.trim().to_string();

        let parsed = Url::parse(&redirect_url).map_err(|e| AuthFlowError::BadResponse {
            msg: format!("invalid redirect URL '{redirect_url}'"),
            source: Some(e.into()),
        })?;

        let grant = serde_urlencoded::from_str(parsed.query().unwrap_or("")).map_err(|e| {
            AuthFlowError::BadResponse {
                msg: format!("failed to parse query parameters from redirect URL '{redirect_url}'"),
                source: Some(e.into()),
            }
        })?;

        Ok(grant)
    }
}
