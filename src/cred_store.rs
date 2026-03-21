use std::path::{Path, PathBuf};

use etcetera::AppStrategy;
use rmcp::transport::{AuthError, StoredCredentials, auth::OAuthTokenResponse};
use serde::{Deserialize, Serialize};

pub struct CredStore {
    dirs: etcetera::app_strategy::Xdg,
}

impl CredStore {
    pub fn new(dirs: &etcetera::app_strategy::Xdg) -> Self {
        Self { dirs: dirs.clone() }
    }
}

// impl rmcp::transport::CredentialStore for CredStore {
//     fn load<'life0, 'async_trait>(
//         &'life0 self,
//     ) -> ::core::pin::Pin<
//         Box<
//             dyn ::core::future::Future<Output = Result<Option<StoredCredentials>, AuthError>>
//                 + ::core::marker::Send
//                 + 'async_trait,
//         >,
//     >
//     where
//         'life0: 'async_trait,
//         Self: 'async_trait,
//     {
//         todo!()
//     }

//     fn save<'life0, 'async_trait>(
//         &'life0 self,
//         credentials: StoredCredentials,
//     ) -> ::core::pin::Pin<
//         Box<
//             dyn ::core::future::Future<Output = Result<(), AuthError>>
//                 + ::core::marker::Send
//                 + 'async_trait,
//         >,
//     >
//     where
//         'life0: 'async_trait,
//         Self: 'async_trait,
//     {
//     }

//     fn clear<'life0, 'async_trait>(
//         &'life0 self,
//     ) -> ::core::pin::Pin<
//         Box<
//             dyn ::core::future::Future<Output = Result<(), AuthError>>
//                 + ::core::marker::Send
//                 + 'async_trait,
//         >,
//     >
//     where
//         'life0: 'async_trait,
//         Self: 'async_trait,
//     {
//         todo!()
//     }
// }

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
