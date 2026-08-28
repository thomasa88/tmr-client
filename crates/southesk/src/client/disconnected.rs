// Copyright 2026 Thomas Axelsson
// SPDX-License-Identifier: MIT

use rmcp::{
    RoleClient, ServiceExt,
    model::{ClientInfo, InitializeRequestParams},
    service::RunningService,
    transport::{
        AuthClient, AuthorizationManager, CredentialStore, StreamableHttpClientTransport,
        auth::{OAuthClientConfig, OAuthState},
        streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use tracing::{debug, info};

use crate::{
    auth_handler,
    cred_store::FullCredStore,
    error::{ClientConnectError, MapAuthToConnectError},
};

use super::{Client, Connected, Disconnected};

type McpClient = RunningService<RoleClient, InitializeRequestParams>;

const MCP_SERVER_URL: &str = "https://mcp.montrose.io";

impl Client<Disconnected> {
    /// Sets up a new MCP connection.
    ///
    /// This will automatically handle authentication, including refreshing
    /// tokens if needed. If there are no valid credentials, the user will be
    /// prompted to authenticate.
    ///
    /// Use [`ClientBuilder::no_auth`](crate::client::ClientBuilder::no_auth) to
    /// disable interactive authentication.
    ///
    /// Call [`disconnect`](Client::disconnect) to disconnect cleanly at
    /// shutdown.
    pub async fn connect(self) -> Result<Client<Connected>, ClientConnectError> {
        info!("Connecting to the MCP server...");
        debug!("Using MCP server URL: {}", MCP_SERVER_URL);

        let mut mcp_client_res = None;

        // Reactive discovery
        let transport = StreamableHttpClientTransport::from_uri(MCP_SERVER_URL);
        let challenge = match ClientInfo::default().serve(transport).await {
            Ok(_) => {
                return Err(ClientConnectError::AuthError {
                    msg: "server did not reply with a challenge".to_string(),
                    source: None,
                });
            }
            Err(e) => match e.auth_challenge() {
                Some(s) => s.to_string(),
                None => {
                    return Err(ClientConnectError::ConnectionError {
                        msg: "failed to connect to MCP server during reactive discovery"
                            .to_string(),
                        // The error type is boxed since it is very large (> 700 bytes), to
                        // avoid creating a gigantic Result type.
                        source: Some(Box::new(e)),
                    });
                }
            },
        };
        debug!("Received auth challenge from server: {}", challenge);

        if let Some(auth_mgr) = self.auth_mgr_from_creds(&challenge).await? {
            info!("Authenticating using stored credentials");
            let res = self.init_mcp_client(auth_mgr).await;
            if let Err(e) = &res
                && e.is_authorization_required()
            {
                info!("Authentication required error encountered");
            } else {
                mcp_client_res = Some(res);
            }
        } else {
            info!("No usable credentials found in the credential store.");
        }

        if mcp_client_res.is_none() {
            info!("Starting new authorization flow.",);
            let auth_mgr = self.authenticate_new_auth(&challenge).await?;
            mcp_client_res = Some(self.init_mcp_client(auth_mgr).await);
        }

        debug_assert!(mcp_client_res.is_some());
        match mcp_client_res {
            Some(Ok(mcp_client)) => {
                info!("Successfully connected to the MCP server");

                Ok(Client {
                    name: self.name,
                    auth_handler: self.auth_handler,
                    cred_store: self.cred_store,
                    state: Connected { client: mcp_client },
                })
            }
            Some(Err(e)) => Err(ClientConnectError::ConnectionError {
                msg: "failed to connect to MCP server".to_string(),
                source: Some(e.into()),
            }),
            None => Err(ClientConnectError::ConnectionError {
                msg: "Invalid client state".to_string(),
                source: None,
            }),
        }
    }

    async fn init_mcp_client(
        &self,
        auth_mgr: AuthorizationManager,
    ) -> Result<McpClient, Box<rmcp::service::ClientInitializeError>> {
        let auth_client = AuthClient::new(reqwest::Client::default(), auth_mgr);
        let transport = StreamableHttpClientTransport::with_client(
            auth_client,
            StreamableHttpClientTransportConfig::with_uri(MCP_SERVER_URL),
        );
        let client_service = ClientInfo::default();
        // The error type is boxed since it is very large (> 700 bytes), to
        // avoid creating a gigantic Result type.
        client_service.serve(transport).await.map_err(Box::new)
    }

    async fn auth_mgr_from_creds(
        &self,
        challenge: &str,
    ) -> Result<Option<AuthorizationManager>, ClientConnectError> {
        // Cannot convert an OAuthState into an AuthorizationManager, as it
        // initially isn't in the Authorized state. So we start with an
        // AuthorizationManager in case we already have usable credentials.
        let mut auth_mgr = AuthorizationManager::new(MCP_SERVER_URL)
            .await
            .map_err(|e| ClientConnectError::AuthError {
                msg: "failed to initialize authorization manager".to_string(),
                source: Some(e.into()),
            })?;

        auth_mgr.set_credential_store(self.cred_store.clone());

        // The authorization manager automatically does a token refresh if
        // needed. See AuthorizationManager::REFRESH_BUFFER_SECS.
        let initialized = Self::auth_mgr_init_from_store_with_secret(
            &mut auth_mgr,
            MCP_SERVER_URL,
            challenge,
            self.cred_store.clone(),
        )
        .await?;

        if initialized {
            info!("Initialized authorization manager from credential store");
            Ok(Some(auth_mgr))
        } else {
            Ok(None)
        }
    }

    /// Replacement for [`AuthorizationManager::initialize_from_store`] that
    /// initializes the auth manager with a client secret.
    async fn auth_mgr_init_from_store_with_secret(
        auth_mgr: &mut AuthorizationManager,
        base_url: impl Into<String>,
        challenge: &str,
        cred_store: impl FullCredStore,
    ) -> Result<bool, ClientConnectError> {
        let creds = cred_store
            .load()
            .await
            .to_connect_auth_err("error while loading credentials from credential store")?;
        let client_secret = cred_store
            .load_client_secret()
            .await
            .to_connect_auth_err("error while loading client secret from credential store")?;

        // Store is missing data. Cannot initialize the auth manager.
        let (Some(creds), Some(client_secret)) = (creds, client_secret) else {
            return Ok(false);
        };

        // AuthorizationManager::initialize_from_store() ->
        // AuthorizationManager::configure_client_id() passes "base_url" for
        // "redirect_uri". Maybe it is just to have a valid URL as placeholder,
        // so that OAuthClientConfig::new() does not complain?
        let oauth_config = OAuthClientConfig::new(creds.client_id, base_url)
            .with_scopes(creds.granted_scopes)
            .with_client_secret(client_secret);

        let metadata = auth_mgr
            .resolve_metadata_from_challenge(Some(challenge))
            .await
            .to_connect_auth_err("failed to discover authorization server metadata")?
            .metadata;
        auth_mgr.set_metadata(metadata);

        // auth_mgr.configure_client_credentials(config) does basically the same as configure_client?
        auth_mgr
            .configure_client(oauth_config)
            .to_connect_auth_err(
                "failed to configure authorization manager with client credentials",
            )?;

        Ok(true)
    }

    async fn authenticate_new_auth(
        &self,
        challenge: &str,
    ) -> Result<AuthorizationManager, ClientConnectError> {
        let Some(auth_handler) = &self.auth_handler else {
            return Err(ClientConnectError::AuthError {
                msg: "need to do a new authentication, but interactive authentication is disabled."
                    .to_string(),
                source: None,
            });
        };

        // oauth: Empty scope will let the server select
        let wanted_scopes = &["mcp"];
        debug!("Requesting scopes: {:?}", wanted_scopes);

        let redirect_uri = auth_handler.redirect_uri();
        debug!("Using redirect URI: {}", redirect_uri);

        let (mut oauth_state, client_secret) =
            Self::oauth_state_start_authorization_and_get_secret(
                wanted_scopes,
                redirect_uri,
                challenge,
                &self.name,
                self.cred_store.clone(),
            )
            .await
            .map_err(|e| ClientConnectError::AuthError {
                msg: "failed to start authorization".to_string(),
                source: Some(e.into()),
            })?;

        // todo: check client_secret has value

        let auth_url = oauth_state.get_authorization_url().await.map_err(|e| {
            ClientConnectError::AuthError {
                msg: "failed to get authorization URL".to_string(),
                source: Some(e.into()),
            }
        })?;
        debug!("Auth URL: {}", auth_url);

        info!("Waiting for authorization code...");
        let auth_handler::AuthGrant {
            code: auth_code,
            state: csrf_token,
            iss: issuer,
        } = auth_handler.authenticate(&auth_url).await?;
        info!("Received authorization code: {}", auth_code);

        info!("Exchanging authorization code for access token...");
        oauth_state
            .handle_callback_with_issuer(&auth_code, &csrf_token, Some(&issuer))
            .await
            .to_connect_auth_err("failed to handle authorization callback")?;

        // OAuthState::handle_callback is the function that initially stores the credentials using the credential store.
        // So store the client secret now as well.
        self.cred_store
            .save_client_secret(&client_secret.unwrap())
            .await
            .unwrap();

        info!("Authorization successful! Access token obtained.");

        let auth_mgr = oauth_state.into_authorization_manager().ok_or_else(|| {
            ClientConnectError::AuthError {
                msg: "failed to convert OAuth state into authorization manager".to_string(),
                source: None,
            }
        })?;

        Ok(auth_mgr)
    }

    /// Replacement for [`OAuthState::start_authorization`] that returns the client secret.
    async fn oauth_state_start_authorization_and_get_secret<
        S: CredentialStore + FullCredStore + 'static,
    >(
        scopes: &[&str],
        redirect_uri: &str,
        challenge: &str,
        client_name: &str,
        cred_store: S,
    ) -> Result<(OAuthState, Option<String>), rmcp::transport::AuthError> {
        // Difference compared to OAuthState::start_authorization:
        // Cannot start with an existing OAuthState (created with `
        // OAuthState::new(MCP_SERVER_URL, None).into_authorization_manager()`)
        // taken as a parameter, because that function only works after the
        // OAuthState is authorized. So we start by creating an
        // AuthorizationManager instead.
        let mut auth_mgr = AuthorizationManager::new(MCP_SERVER_URL).await?;

        auth_mgr.set_credential_store(cred_store);

        debug!("start discovery");
        let metadata = auth_mgr
            .resolve_metadata_from_challenge(Some(challenge))
            .await?
            .metadata;
        auth_mgr.set_metadata(metadata);
        let _ = auth_mgr.select_scopes(None, scopes);

        debug!("start session");
        // Start of AuthorizationSession::new replacement
        //
        // AuthorizationSession::new will run register_client, without giving us
        // access to the client secret. Instead, we perform the steps of `new`
        // manually.
        //
        // register_client will store the client data (including secret) in the
        // auth managers OAuthClient/oauth2::Client object, so the current auth
        // manager will have it internally. (The client secret cannot be
        // retrieved from that object.)
        let oauth_config = auth_mgr
            .register_client(client_name, redirect_uri, scopes)
            .await?;
        let auth_url = auth_mgr.get_authorization_url(scopes).await?;
        // for_scope_upgrade lets us create the AuthorizationSession object with
        // the data from register_client. AuthorizationSession::new would have
        // run register_client.
        let session = rmcp::transport::AuthorizationSession::for_scope_upgrade(
            auth_mgr,
            auth_url,
            redirect_uri,
        );
        let oauth_state = OAuthState::Session(session);
        // End of AuthorizationSession::new replacement

        Ok((oauth_state, oauth_config.client_secret))
    }
}
