// Copyright 2026 Thomas Axelsson
// SPDX-License-Identifier: MIT

use std::io::{self, Write};

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn,tmr_client=info,introspect=info".to_string().into()),
        )
        // Log to stderr, so that logs and output can be separated
        .with(tracing_subscriber::fmt::layer().with_writer(io::stderr))
        .init();

    let montrose = tmr_client::TmrClient::new("TMR Client Sample");
    let montrose = montrose.connect().await?;

    io::stdout().write_all(montrose.introspect().await.as_bytes())?;

    Ok(())
}
