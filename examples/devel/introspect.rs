// Copyright 2026 Thomas Axelsson
// SPDX-License-Identifier: MIT

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn,tmr_client=info,introspect=info".to_string().into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let montrose = tmr_client::TmrClient::new("TMR Client Sample");
    let montrose = montrose.connect().await?;

    montrose.introspect().await;

    Ok(())
}
