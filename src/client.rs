// Copyright 2026 Thomas Axelsson
// SPDX-License-Identifier: MIT

use std::marker::PhantomData;

use rmcp::{
    RoleClient, ServiceExt,
    model::{
        CallToolRequestParams, CallToolResult, ClientInfo, InitializeRequestParams, JsonObject,
    },
    service::RunningService,
    transport::{
        AuthClient, AuthorizationManager, StreamableHttpClientTransport, auth::OAuthState,
        streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde::de::DeserializeOwned;
use tracing::{debug, info};
use uuid::Uuid;

use crate::{
    TmrCallError,
    cred_store::CredStore,
    oauth_handler::{self, AuthCallbackHandler, DefaultAuthCallbackHandler},
    result::TmrConnectError,
    tools,
};

pub struct TmrClient<CB: AuthCallbackHandler = DefaultAuthCallbackHandler, S: State = Disconnected>
{
    client_name: String,
    lib_dirs: etcetera::app_strategy::Xdg,
    state: S,
    auth_callback_handler: std::marker::PhantomData<CB>,
}

pub trait State {}

pub struct Disconnected {}
pub struct Connected {
    client: RunningService<RoleClient, InitializeRequestParams>,
}

const MCP_SERVER_URL: &str = "https://mcp.montrose.io";

impl State for Disconnected {}
impl State for Connected {}

impl<CB: AuthCallbackHandler, S: State> TmrClient<CB, S> {}

impl TmrClient<DefaultAuthCallbackHandler, Disconnected> {
    pub fn new(
        client_name: impl Into<String>,
    ) -> TmrClient<DefaultAuthCallbackHandler, Disconnected> {
        Self::new_with_cb(client_name)
    }
}

impl<CB: AuthCallbackHandler> TmrClient<CB, Disconnected> {
    pub fn new_with_cb(client_name: impl Into<String>) -> TmrClient<CB, Disconnected> {
        let lib_dirs = etcetera::choose_app_strategy(etcetera::AppStrategyArgs {
            top_level_domain: "".to_string(),
            author: "thomasa88".to_string(),
            app_name: "tmr-client".to_string(),
        })
        .unwrap();
        Self {
            client_name: client_name.into(),
            lib_dirs,
            state: Disconnected {},
            auth_callback_handler: PhantomData,
        }
    }
}

impl<CB: AuthCallbackHandler> TmrClient<CB, Disconnected> {
    pub async fn connect(
        self,
    ) -> Result<TmrClient<DefaultAuthCallbackHandler, Connected>, TmrConnectError> {
        let auth_mgr = self.authenticate().await?;

        let mut mcp_client_res = self.init_mcp_client(auth_mgr).await;

        if let Err(rmcp::service::ClientInitializeError::TransportError {
            error: dyn_transport_err,
            context: _,
        }) = &mcp_client_res
        {
            debug!("Transport error: {dyn_transport_err:#?}");
            if Self::is_auth_required_error(dyn_transport_err) {
                info!("Authentication required error encountered");
                info!("Starting new authorization flow");
                let auth_mgr = self.authenticate_new_auth().await?;
                mcp_client_res = self.init_mcp_client(auth_mgr).await;
            }
        }

        let mcp_client = mcp_client_res.map_err(|e| TmrConnectError::ConnectionError {
            msg: "Failed to connect to MCP server".to_string(),
            source: Some(e.into()),
        })?;

        info!("Successfully connected to MCP server");

        Ok(TmrClient {
            client_name: self.client_name,
            lib_dirs: self.lib_dirs,
            state: Connected { client: mcp_client },
            auth_callback_handler: PhantomData,
        })
    }

    fn is_auth_required_error(dyn_transport_err: &rmcp::transport::DynamicTransportError) -> bool {
        let http_error = dyn_transport_err
            .error
            .downcast_ref::<rmcp::transport::streamable_http_client::StreamableHttpError<
            reqwest::Error,
        >>();
        matches!(
            http_error,
            Some(
                rmcp::transport::streamable_http_client::StreamableHttpError::Auth(
                    rmcp::transport::AuthError::AuthorizationRequired,
                )
            )
        )
    }

    async fn init_mcp_client(
        &self,
        auth_mgr: AuthorizationManager,
    ) -> Result<
        RunningService<RoleClient, InitializeRequestParams>,
        rmcp::service::ClientInitializeError,
    > {
        let auth_client = AuthClient::new(reqwest::Client::default(), auth_mgr);
        let transport = StreamableHttpClientTransport::with_client(
            auth_client,
            StreamableHttpClientTransportConfig::with_uri(MCP_SERVER_URL),
        );
        let client_service = ClientInfo::default();
        client_service.serve(transport).await
    }

    async fn authenticate(&self) -> Result<AuthorizationManager, TmrConnectError> {
        debug!("Using MCP server URL: {}", MCP_SERVER_URL);

        info!("Establishing authorized connection to MCP server...");
        // Cannot convert an OAuthState into an AuthorizationManager, as it
        // initially isn't in the Authorized state. So we start with an
        // AuthorizationManager in case we already have usable credentials.
        let mut auth_mgr = AuthorizationManager::new(MCP_SERVER_URL)
            .await
            .map_err(|e| TmrConnectError::AuthError {
                msg: "Failed to initialize authorization manager".to_string(),
                source: Some(e.into()),
            })?;
        auth_mgr.set_credential_store(CredStore::new(&self.lib_dirs));
        // The authorization manager automatically does a token refresh if
        // needed. See REFRESH_BUFFER_SECS in rmcp.
        let initialized =
            auth_mgr
                .initialize_from_store()
                .await
                .map_err(|e| TmrConnectError::AuthError {
                    msg: "Failed to initialize authorization manager from credential store"
                        .to_string(),
                    source: Some(e.into()),
                })?;
        if initialized {
            info!("Initialized authorization manager from credential store");
            return Ok(auth_mgr);
        }

        info!("No credentials found in store, starting new authorization flow");
        self.authenticate_new_auth().await
    }

    async fn authenticate_new_auth(&self) -> Result<AuthorizationManager, TmrConnectError> {
        let mut oauth_state = OAuthState::new(MCP_SERVER_URL, None).await.map_err(|e| {
            TmrConnectError::AuthError {
                msg: "Failed to initialize OAuth state".to_string(),
                source: Some(e.into()),
            }
        })?;
        oauth_state.set_credential_store(CredStore::new(&self.lib_dirs));

        // oauth: Empty scope will let the server select
        let wanted_scopes = &["mcp"];
        debug!("Requesting scopes: {:?}", wanted_scopes);

        let auth_serve = CB::new().await?;

        auth_serve.get_listen_addr();
        let redirect_uri = auth_serve.get_listen_addr();
        debug!("Using redirect URI: {}", redirect_uri);
        oauth_state
            .start_authorization(wanted_scopes, redirect_uri, Some(&self.client_name))
            .await
            .map_err(|e| TmrConnectError::AuthError {
                msg: "Failed to start authorization".to_string(),
                source: Some(e.into()),
            })?;

        let auth_url =
            oauth_state
                .get_authorization_url()
                .await
                .map_err(|e| TmrConnectError::AuthError {
                    msg: "Failed to get authorization URL".to_string(),
                    source: Some(e.into()),
                })?;
        debug!("Auth URL: {}", auth_url);

        info!("Waiting for authorization code...");
        let oauth_handler::AuthCallback {
            code: auth_code,
            state: csrf_token,
        } = auth_serve.wait_for_callback(&auth_url).await?;
        info!("Received authorization code: {}", auth_code);

        info!("Exchanging authorization code for access token...");
        oauth_state
            .handle_callback(&auth_code, &csrf_token)
            .await
            .map_err(|e| TmrConnectError::AuthError {
                msg: "Failed to handle authorization callback".to_string(),
                source: Some(e.into()),
            })?;
        info!("Successfully obtained access token");

        info!("Authorization successful! Access token obtained.");

        let (client_id, Some(_token_response)) =
            oauth_state
                .get_credentials()
                .await
                .map_err(|e| TmrConnectError::AuthError {
                    msg: "Failed to get credentials from OAuth state".to_string(),
                    source: Some(e.into()),
                })?
        else {
            return Err(TmrConnectError::AuthError {
                msg: "No credentials obtained from OAuth state".to_string(),
                source: None,
            });
        };
        debug!("Obtained client id: {}", client_id);

        let auth_mgr =
            oauth_state
                .into_authorization_manager()
                .ok_or_else(|| TmrConnectError::AuthError {
                    msg: "Failed to convert OAuth state into authorization manager".to_string(),
                    source: None,
                })?;

        Ok(auth_mgr)
    }
}

impl<CB: AuthCallbackHandler> TmrClient<CB, Connected> {
    /// Fetches and logs available tools and prompts from the server
    /// Used for TmrClient development.
    pub async fn introspect(&self) {
        info!("Fetching available tools from server...");

        match self.state.client.peer().list_all_tools().await {
            Ok(tools) => {
                info!("Available tools: {}", tools.len());
                for tool in tools {
                    info!(
                        "- {} ({})\n{:#?}\n{:#?}\n",
                        tool.name,
                        tool.description.unwrap_or_default(),
                        tool.input_schema,
                        tool.output_schema,
                    );
                }
            }
            Err(e) => {
                info!("Error fetching tools: {}", e);
            }
        }

        info!("Fetching available prompts from server...");

        match self.state.client.peer().list_all_prompts().await {
            Ok(prompts) => {
                info!("Available prompts: {}", prompts.len());
                for prompt in prompts {
                    info!("- {}", prompt.name);
                }
            }
            Err(e) => {
                info!("Error fetching prompts: {}", e);
            }
        }
    }

    /// Returns holdings for either one account (when accountId is provided) or
    /// all accessible accounts. Use get_user_accounts first to find valid account
    /// IDs.
    pub async fn get_holdings(
        &self,
        account_id: Option<Uuid>,
    ) -> Result<Vec<tools::Account>, TmrCallError> {
        let mut args = serde_json::Map::new();
        args.insert(
            "accountId".to_string(),
            account_id.map(|id| id.to_string()).into(),
        );
        self.call("get_holdings", Some(args)).await
    }

    /// Returns all user accounts with stable account IDs and display names. Use
    /// this tool to discover valid account IDs before calling GetHoldings for a
    /// specific account.
    pub async fn get_user_accounts(&self) -> Result<Vec<tools::AccountInfo>, TmrCallError> {
        self.call("get_user_accounts", None).await
    }

    /// Creates a pre-filled trade ticket URL for the Montrose app. Specify side
    /// (Buy/Sell), quantity or amount, and an instrument identifier.
    /// Instruments can be specified by orderbookId directly, or by ticker/name
    /// which will be resolved automatically. Returns a URL that opens the trade
    /// ticket in the Montrose app with the order details pre-filled.
    pub async fn create_trade_ticket(
        &self,
        args: tools::TradeTicketArgs,
    ) -> Result<reqwest::Url, TmrCallError> {
        let json_obj = serde_json::to_value(args)
            .map_err(|e| {
                TmrCallError::InvalidArguments(format!("Could not convert args to JSON: {e}"))
            })?
            .as_object()
            .ok_or(TmrCallError::InvalidArguments(
                "Could not convert args to JSON object".to_string(),
            ))?
            .to_owned();
        self.call::<tools::CreateTradeTicketResult>("create_trade_ticket", Some(json_obj))
            .await
            .map(|res| res.url)
    }

    //     [src/main.rs:115:9] &res = CallToolResult {
    //     content: [
    //         Annotated {
    //             raw: Text(
    //                 RawTextContent {
    //                     text: "{\"url\":\"https://app.montrose.io/trade?ticketId=<uuid>\"}",
    //                     meta: None,
    //                 },
    //             ),
    //             annotations: None,
    //         },
    //     ],
    //     structured_content: None,
    //     is_error: None,
    //     meta: None,

    //     [src/main.rs:115:9] &res = CallToolResult {
    //     content: [
    //         Annotated {
    //             raw: Text(
    //                 RawTextContent {
    //                     text: "No instruments found for \"LF GLOBur\".",
    //                     meta: None,
    //                 },
    //             ),
    //             annotations: None,
    //         },
    //     ],
    //     structured_content: None,
    //     is_error: Some(
    //         true,
    //     ),
    //     meta: None,

    // client.cancel()?

    async fn call<T: DeserializeOwned>(
        &self,
        tool: &str,
        args: Option<JsonObject>,
    ) -> Result<T, TmrCallError> {
        let req = CallToolRequestParams::new(tool.to_owned());
        let req = if let Some(args) = args {
            req.with_arguments(args)
        } else {
            req
        };
        debug!("Call request: {:#?}", req);
        let res = self.state.client.call_tool(req).await?;
        debug!("Call response: {:#?}", res);
        parse_result::<T>(&res)
    }
}

fn parse_result<T: DeserializeOwned>(res: &CallToolResult) -> Result<T, TmrCallError> {
    let text = &res
        .content
        .first()
        .ok_or(TmrCallError::parse_err("No content element in response"))?
        .raw
        .as_text()
        .ok_or(TmrCallError::parse_err("No raw text in response"))?
        .text;
    if res.is_error.unwrap_or(false) {
        return Err(TmrCallError::McpError(format!("Error from server: {text}")));
    }
    serde_json::from_str::<T>(text).map_err(|e| TmrCallError::ParseError {
        msg: format!("Failed to parse response text: {text}"),
        source: Some(e.into()),
    })
}
