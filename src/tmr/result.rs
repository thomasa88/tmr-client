// Copyright 2026 Thomas Axelsson
// SPDX-License-Identifier: MIT

#[derive(Debug, thiserror::Error)]
pub enum TmrCallError {
    #[error("MCP service communication error")]
    CommunicationError(#[from] rmcp::ServiceError),
    #[error("Error response from MCP service: {0}")]
    McpError(String),
    #[error("Failed to parse response: {msg}")]
    ParseError {
        msg: String,
        source: Option<anyhow::Error>,
    },
    #[error("Invalid arguments")]
    InvalidArguments(String),
}

impl TmrCallError {
    pub fn parse_err(msg: impl Into<String>) -> TmrCallError {
        Self::ParseError {
            msg: msg.into(),
            source: None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TmrConnectError {
    #[error("Authentication failed: {msg}")]
    AuthError {
        msg: String,
        source: Option<anyhow::Error>,
    },
    #[error("Connection failed: {msg}")]
    ConnectionError {
        msg: String,
        source: Option<anyhow::Error>,
    },
}
