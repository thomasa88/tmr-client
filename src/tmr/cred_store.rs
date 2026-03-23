use std::path::PathBuf;
use tracing::debug;

use async_trait::async_trait;
use etcetera::AppStrategy;
use rmcp::transport::{AuthError, StoredCredentials, auth::OAuthTokenResponse};
use serde::{Deserialize, Serialize};

pub struct CredStore {
    filename: PathBuf,
}

impl CredStore {
    pub fn new(dirs: &etcetera::app_strategy::Xdg) -> Self {
        let filename = dirs.config_dir().join("credentials.json");
        std::fs::create_dir_all(filename.parent().unwrap()).unwrap();
        Self { filename }
    }
}

#[async_trait]
impl rmcp::transport::CredentialStore for CredStore {
    async fn load(&self) -> Result<Option<StoredCredentials>, AuthError> {
        if !self.filename.exists() {
            return Ok(None);
        }
        let file = std::fs::File::open(&self.filename).unwrap();
        let creds: StoredCredentials = serde_json::from_reader(file).unwrap();
        debug!("Loaded credentials from {:?}", self.filename);
        Ok(Some(creds))
    }

    async fn save(&self, credentials: StoredCredentials) -> Result<(), AuthError> {
        let file = std::fs::File::create(&self.filename).unwrap();
        serde_json::to_writer_pretty(file, &credentials).unwrap();
        debug!("Saved credentials to {:?}", self.filename);
        Ok(())
    }

    async fn clear(&self) -> Result<(), AuthError> {
        std::fs::remove_file(&self.filename).ok();
        debug!("Cleared credentials at {:?}", self.filename);
        Ok(())
    }
}

pub struct OauthStore {
    filename: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
struct OauthCredentials {
    client_id: String,
    token_response: OAuthTokenResponse,
}

impl OauthStore {
    pub fn new(dirs: &etcetera::app_strategy::Xdg) -> Self {
        let filename = dirs.config_dir().join("oauth_credentials.json");
        std::fs::create_dir_all(filename.parent().unwrap()).unwrap();
        Self { filename }
    }

    pub fn save(&self, client_id: &str, credentials: &OAuthTokenResponse) {
        let creds = OauthCredentials {
            client_id: client_id.to_string(),
            token_response: credentials.clone(),
        };
        let file = std::fs::File::create(&self.filename).unwrap();
        serde_json::to_writer_pretty(file, &creds).unwrap();
    }

    pub fn load(&self) -> Option<(String, OAuthTokenResponse)> {
        let file = std::fs::File::open(&self.filename).ok()?;
        let creds: OauthCredentials = serde_json::from_reader(file).ok()?;
        Some((creds.client_id, creds.token_response))
    }
}
