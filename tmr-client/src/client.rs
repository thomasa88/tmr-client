// Copyright 2026 Thomas Axelsson
// SPDX-License-Identifier: MIT

use std::marker::PhantomData;

use oauth2::TokenResponse;
use rmcp::{
    RoleClient, ServiceExt,
    model::{
        CallToolRequestParams, CallToolResult, ClientInfo, InitializeRequestParams, JsonObject,
    },
    service::RunningService,
    transport::{
        AuthClient, AuthorizationManager, CredentialStore, StoredCredentials,
        StreamableHttpClientTransport, auth::OAuthState,
        streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde::de::DeserializeOwned;
use tracing::{debug, info, trace};
use uuid::Uuid;

use crate::{
    TmrCallError,
    cred_store::CredStore,
    oauth_handler::{self, AuthCallbackHandler, DefaultAuthCallbackHandler},
    result::TmrConnectError,
    tools::{self, GetHoldingsResult},
};

pub struct TmrClient<CB: AuthCallbackHandler = DefaultAuthCallbackHandler, S: State = Disconnected>
{
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
    pub fn new() -> TmrClient<DefaultAuthCallbackHandler, Disconnected> {
        let lib_dirs = etcetera::choose_app_strategy(etcetera::AppStrategyArgs {
            top_level_domain: "".to_string(),
            author: "thomasa88".to_string(),
            app_name: "tmr-client".to_string(),
        })
        .unwrap();
        Self {
            lib_dirs,
            state: Disconnected {},
            auth_callback_handler: PhantomData,
        }
    }
}

impl<CB: AuthCallbackHandler> TmrClient<CB, Disconnected> {
    pub fn new_with_cb() -> TmrClient<CB, Disconnected> {
        let lib_dirs = etcetera::choose_app_strategy(etcetera::AppStrategyArgs {
            top_level_domain: "".to_string(),
            author: "thomasa88".to_string(),
            app_name: "tmr-client".to_string(),
        })
        .unwrap();
        Self {
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
        let am = self.authenticate().await?;

        // info!("refresh");
        // dbg!(am.get_credentials().await?);
        // am.refresh_token().await?;
        let client = AuthClient::new(reqwest::Client::default(), am);
        let transport = StreamableHttpClientTransport::with_client(
            client,
            StreamableHttpClientTransportConfig::with_uri(MCP_SERVER_URL),
        );

        let client_service = ClientInfo::default();
        let client = client_service.serve(transport).await.map_err(|e| {
            TmrConnectError::ConnectionError {
                msg: "Failed to connect to MCP server".to_string(),
                source: Some(e.into()),
            }
        })?;
        info!("Successfully connected to MCP server");

        Ok(TmrClient {
            lib_dirs: self.lib_dirs,
            state: Connected { client },
            auth_callback_handler: PhantomData,
        })
    }

    async fn authenticate(&self) -> Result<AuthorizationManager, TmrConnectError> {
        info!("Using MCP server URL: {}", MCP_SERVER_URL);

        // let cred_store = CredStore::new(&dirs);
        // // am.set_credential_store(store);
        // let mut am = AuthorizationManager::new(&server_url).await?;
        // am.set_credential_store(cred_store);
        // am.initialize_from_store().await?;

        let mut oauth_state = OAuthState::new(MCP_SERVER_URL, None).await.map_err(|e| {
            TmrConnectError::AuthError {
                msg: "Failed to initialize OAuth state".to_string(),
                source: Some(e.into()),
            }
        })?;

        // let oauth_store = OauthStore::new(&dirs);
        // if let Some((client_id, token_response)) = oauth_store.load() {
        //     info!("Loaded credentials from store for client_id: {}", client_id);
        //     oauth_state
        //         .set_credentials(&client_id, token_response)
        //         .await?;
        // } else {
        // }

        // let cred_store = CredStore::new(&dirs);

        info!("Establishing authorized connection to MCP server...");
        let am = {
            let mut am = AuthorizationManager::new(MCP_SERVER_URL)
                .await
                .map_err(|e| TmrConnectError::AuthError {
                    msg: "Failed to initialize authorization manager".to_string(),
                    source: Some(e.into()),
                })?;
            am.set_credential_store(CredStore::new(&self.lib_dirs));
            if am
                .initialize_from_store()
                .await
                .map_err(|e| TmrConnectError::AuthError {
                    msg: "Failed to initialize from credential store".to_string(),
                    source: Some(e.into()),
                })?
            {
                info!("Initialized authorization manager from credential store");
                am
            } else {
                info!("No credentials found in store, starting new authorization flow");

                // oauth: Empty scope will let the server select
                let wanted_scopes = &["mcp"];
                debug!("Requesting scopes: {:?}", wanted_scopes);

                let auth_serve = CB::new().await?;

                auth_serve.get_listen_addr();
                let redirect_uri = auth_serve.get_listen_addr();
                debug!("Using redirect URI: {}", redirect_uri);
                oauth_state
                    .start_authorization(wanted_scopes, redirect_uri, Some("TMR Client"))
                    .await
                    .map_err(|e| TmrConnectError::AuthError {
                        msg: "Failed to start authorization".to_string(),
                        source: Some(e.into()),
                    })?;

                let auth_url = oauth_state.get_authorization_url().await.map_err(|e| {
                    TmrConnectError::AuthError {
                        msg: "Failed to get authorization URL".to_string(),
                        source: Some(e.into()),
                    }
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

                // let creds = oauth_state
                //     .get_credentials()
                //     .await
                //     .context("Failed to get credentials from oauth state")?;
                // if let (client_id, Some(token_response)) = creds {
                //     // oauth_store.save(&client_id, &token_response);
                // } else {
                //     warn!("No credentials obtained from oauth state");
                // }

                // am.configure_client_credentials(config)
                // oauth_state.into_authorization_manager()
                // am
                // oauth_state.to_authorized_http_client().await?

                let (client_id, Some(token_response)) = oauth_state
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
                let granted_scopes: Vec<String> = token_response
                    .scopes()
                    .map(|scopes| scopes.iter().map(|scope| scope.to_string()).collect())
                    .unwrap_or_default();
                let received_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let cred_store = CredStore::new(&self.lib_dirs);
                cred_store
                    .save(StoredCredentials::new(
                        client_id,
                        Some(token_response),
                        granted_scopes,
                        Some(received_at),
                    ))
                    .await
                    .map_err(|e| TmrConnectError::AuthError {
                        msg: "Failed to save credentials".to_string(),
                        source: Some(e.into()),
                    })?;

                oauth_state.into_authorization_manager().ok_or_else(|| {
                    TmrConnectError::AuthError {
                        msg: "Failed to convert OAuth state into authorization manager".to_string(),
                        source: None,
                    }
                })?;

                // set_credential_store() clears the credentials in the manager, so we need to set them again
                am.set_credential_store(cred_store);
                am.initialize_from_store()
                    .await
                    .map_err(|e| TmrConnectError::AuthError {
                        msg: "Failed to initialize authorization manager from saved credentials"
                            .to_string(),
                        source: Some(e.into()),
                    })?;
                am
            }
        };
        Ok(am)
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
    ) -> Result<GetHoldingsResult, TmrCallError> {
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
    pub async fn get_user_accounts(&self) -> Result<tools::Accounts, TmrCallError> {
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
                TmrCallError::InvalidArguments("Could not convert args to JSON".to_string())
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

    async fn call<'a, T: DeserializeOwned>(
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
        .get(0)
        .ok_or(TmrCallError::parse_err("No content element in response"))?
        .raw
        .as_text()
        .ok_or(TmrCallError::parse_err("No raw text in response"))?
        .text;
    if res.is_error.unwrap_or(false) {
        return Err(TmrCallError::McpError(format!("Error from server: {text}")));
    }
    Ok(
        serde_json::from_str::<T>(text).map_err(|e| TmrCallError::ParseError {
            msg: format!("Failed to parse response text: {text}"),
            source: Some(e.into()),
        })?,
    )
}
