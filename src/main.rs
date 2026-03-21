use std::{env, net::SocketAddr, sync::Arc};

use anyhow::{Context, Result};
use axum::{
    Router,
    extract::{Query, State},
    response::Html,
    routing::get,
};
use rmcp::{
    ServiceExt,
    model::ClientInfo,
    transport::{
        AuthorizationManager, StreamableHttpClientTransport,
        auth::{AuthClient, OAuthState},
        streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde::Deserialize;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
    sync::{Mutex, oneshot},
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::cred_store::{CredStore, OauthStore};

mod cred_store;

const MCP_SERVER_URL: &str = "https://mcp.montrose.io";
// The MCP server does not like an IP as the host in the callback server (HTTP/2 403)
const MCP_REDIRECT_URI: &str = "http://localhost:8080/callback";
const CALLBACK_PORT: u16 = 8080;
const CALLBACK_HTML: &str = include_str!("callback.html");

#[derive(Clone)]
struct AppState {
    code_receiver: Arc<Mutex<Option<oneshot::Sender<CallbackParams>>>>,
}

#[derive(Debug, Deserialize)]
struct CallbackParams {
    code: String,
    state: String,
}

async fn callback_handler(
    Query(params): Query<CallbackParams>,
    State(state): State<AppState>,
) -> Html<String> {
    tracing::info!("Received callback: {params:?}");

    // Send the code to the main thread
    if let Some(sender) = state.code_receiver.lock().await.take() {
        let _ = sender.send(params);
    }
    // Return success page
    Html(CALLBACK_HTML.to_string())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".to_string().into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    // it is a http server for handling callback
    // Create channel for receiving authorization code
    let (code_sender, code_receiver) = oneshot::channel::<CallbackParams>();

    // Create app state
    let app_state = AppState {
        code_receiver: Arc::new(Mutex::new(Some(code_sender))),
    };

    // Start HTTP server for handling callbacks
    let app = Router::new()
        .route("/callback", get(callback_handler))
        .with_state(app_state);

    let addr = SocketAddr::from(([127, 0, 0, 1], CALLBACK_PORT));
    tracing::info!("Starting callback server at: http://{}", addr);
    tracing::warn!(
        "Note: Callback server may not receive callbacks if redirect URI doesn't match localhost if using CIMD (SEP-991)"
    );

    // Start server in a separate task
    tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        let result = axum::serve(listener, app).await;

        if let Err(e) = result {
            tracing::error!("Callback server error: {}", e);
        }
    });

    // Get server URL and client metadata URL from CLI (with defaults)
    //
    // Usage:
    //   cargo run -p mcp-client-examples --example clients_oauth_client -- <server_url> <client_metadata_url>
    let args: Vec<String> = env::args().collect();
    let server_url = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| MCP_SERVER_URL.to_string());

    tracing::info!("Using MCP server URL: {}", server_url);

    let dirs = etcetera::choose_app_strategy(etcetera::AppStrategyArgs {
        top_level_domain: "".to_string(),
        author: "Thomas Axelsson".to_string(),
        app_name: "tmr-client".to_string(),
    })
    .unwrap();
    // let cred_store = CredStore::new(&dirs);
    // // am.set_credential_store(store);
    // let mut am = AuthorizationManager::new(&server_url).await?;
    // am.set_credential_store(cred_store);
    // am.initialize_from_store().await?;

    // initialize oauth state machine
    let mut oauth_state = OAuthState::new(&server_url, None)
        .await
        .context("Failed to initialize oauth state machine")?;

    let mut output = BufWriter::new(tokio::io::stdout());

    let oauth_store = OauthStore::new(&dirs);
    if let Some((client_id, token_response)) = oauth_store.load() {
        tracing::info!("Loaded credentials from store for client_id: {}", client_id);
        oauth_state.set_credentials(&client_id, token_response).await?;
    } else {
        tracing::info!("No credentials found in store, starting new authorization flow");
        // passing empty scopes lets the SDK auto-select from the server's
        // WWW-Authenticate header, Protected Resource Metadata, or AS metadata.
        oauth_state
            .start_authorization(&["mcp"], MCP_REDIRECT_URI, Some("TMR Client"))
            .await
            .context("Failed to start authorization")?;

        // Output authorization URL to user
        output.write_all(b"\n=== MCP OAuth Client ===\n\n").await?;
        output
            .write_all(b"Please open the following URL in your browser to authorize:\n\n")
            .await?;
        output
            .write_all(oauth_state.get_authorization_url().await?.as_bytes())
            .await?;
        output
            .write_all(b"\n\nWaiting for browser callback, please do not close this window...\n")
            .await?;
        output.flush().await?;

        // Wait for authorization code
        tracing::info!("Waiting for authorization code...");
        let CallbackParams {
            code: auth_code,
            state: csrf_token,
        } = code_receiver
            .await
            .context("Failed to get authorization code")?;
        tracing::info!("Received authorization code: {}", auth_code);
        // Exchange code for access token
        tracing::info!("Exchanging authorization code for access token...");
        oauth_state
            .handle_callback(&auth_code, &csrf_token)
            .await
            .context("Failed to handle callback")?;
        tracing::info!("Successfully obtained access token");

        output
            .write_all(b"\nAuthorization successful! Access token obtained.\n\n")
            .await?;
        output.flush().await?;

        let creds = oauth_state
            .get_credentials()
            .await
            .context("Failed to get credentials from oauth state")?;
        if let (client_id, Some(token_response)) = creds {
            oauth_store.save(&client_id, &token_response);
        } else {
            tracing::warn!("No credentials obtained from oauth state");
        }
    }

    // Create authorized transport, this transport is authorized by the oauth state machine
    tracing::info!("Establishing authorized connection to MCP server...");
    let am = oauth_state
        .into_authorization_manager()
        .ok_or_else(|| anyhow::anyhow!("Failed to get authorization manager"))?;

    let client = AuthClient::new(reqwest::Client::default(), am);
    let transport = StreamableHttpClientTransport::with_client(
        client,
        StreamableHttpClientTransportConfig::with_uri(server_url.as_str()),
    );

    // Create client and connect to MCP server
    let client_service = ClientInfo::default();
    let client = client_service.serve(transport).await?;
    tracing::info!("Successfully connected to MCP server");

    // Test API requests
    output
        .write_all(b"Fetching available tools from server...\n")
        .await?;
    output.flush().await?;

    match client.peer().list_all_tools().await {
        Ok(tools) => {
            output
                .write_all(format!("Available tools: {}\n\n", tools.len()).as_bytes())
                .await?;
            for tool in tools {
                output
                    .write_all(
                        format!(
                            "- {} ({})\n",
                            tool.name,
                            tool.description.unwrap_or_default()
                        )
                        .as_bytes(),
                    )
                    .await?;
            }
        }
        Err(e) => {
            output
                .write_all(format!("Error fetching tools: {}\n", e).as_bytes())
                .await?;
        }
    }

    output
        .write_all(b"\nFetching available prompts from server...\n")
        .await?;
    output.flush().await?;

    match client.peer().list_all_prompts().await {
        Ok(prompts) => {
            output
                .write_all(format!("Available prompts: {}\n\n", prompts.len()).as_bytes())
                .await?;
            for prompt in prompts {
                output
                    .write_all(format!("- {}\n", prompt.name).as_bytes())
                    .await?;
            }
        }
        Err(e) => {
            output
                .write_all(format!("Error fetching prompts: {}\n", e).as_bytes())
                .await?;
        }
    }

    output
        .write_all(b"\nConnection established successfully. You are now authenticated with the MCP server.\n")
        .await?;
    output.flush().await?;

    Ok(())
}
