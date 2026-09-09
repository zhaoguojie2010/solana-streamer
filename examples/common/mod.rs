// Each example uses only the helpers relevant to its transport.
#![allow(dead_code)]

use anyhow::{bail, Context, Result};
use solana_sdk::pubkey::Pubkey;
use solana_streamer_sdk::streaming::common::SubscriptionHandle;
use std::{env, time::Duration};
use tokio::sync::Mutex;

pub fn env_or_default(name: &str, default: &str) -> Result<String> {
    match env::var(name) {
        Ok(value) if value.trim().is_empty() => bail!("{name} must not be empty"),
        Ok(value) => Ok(value),
        Err(env::VarError::NotPresent) => Ok(default.to_string()),
        Err(error) => Err(error).with_context(|| format!("Cannot read {name}")),
    }
}

pub fn grpc_endpoint() -> Result<String> {
    env_or_default("GRPC_ENDPOINT", "https://solana-yellowstone-grpc.publicnode.com:443")
}

pub fn grpc_token() -> Result<Option<String>> {
    match env::var("GRPC_X_TOKEN") {
        Ok(token) => Ok((!token.trim().is_empty()).then_some(token)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(error).context("Cannot read GRPC_X_TOKEN"),
    }
}

pub fn required_pubkey(name: &str) -> Result<String> {
    let value =
        env::var(name).with_context(|| format!("Set {name} to the account address to monitor"))?;
    value.parse::<Pubkey>().with_context(|| format!("{name} must be a valid Solana public key"))?;
    Ok(value)
}

/// Stop a streaming example after the requested duration, or on Ctrl+C.
pub async fn wait_for_shutdown(handle: &Mutex<Option<SubscriptionHandle>>) -> Result<()> {
    let duration = env_or_default("RUN_DURATION_SECS", "1000")?
        .parse::<u64>()
        .context("RUN_DURATION_SECS must be a non-negative integer")?;
    tokio::select! {
        result = tokio::signal::ctrl_c() => result.context("Cannot listen for Ctrl+C")?,
        _ = tokio::time::sleep(Duration::from_secs(duration)) => {},
        _ = async {
            loop {
                if handle.lock().await.as_ref().map_or(true, SubscriptionHandle::is_finished) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        } => bail!("Subscription stream ended unexpectedly; check the stream error in the logs"),
    }
    Ok(())
}
