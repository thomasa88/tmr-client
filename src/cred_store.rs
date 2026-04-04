// Copyright 2026 Thomas Axelsson
// SPDX-License-Identifier: MIT

use std::path::PathBuf;
use tracing::debug;

use async_trait::async_trait;
use etcetera::AppStrategy;
use rmcp::transport::{AuthError, StoredCredentials};

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
