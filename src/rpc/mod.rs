//! Optional adapters for historical transaction responses (`rpc` feature).
//!
//! HTTP queries belong to the caller. Enable `rpc-client` for the client and
//! configuration re-exports used by the `parse_tx_events` example.
mod swap;
mod transaction;

pub use solana_transaction_status_client_types as types;
pub use swap::parse_swap_data_from_next_instructions;
pub use transaction::transaction_frame;

#[cfg(feature = "rpc-client")]
pub use solana_commitment_config::CommitmentConfig;
#[cfg(feature = "rpc-client")]
pub use solana_rpc_client::nonblocking::rpc_client::RpcClient;
#[cfg(feature = "rpc-client")]
pub use solana_rpc_client_api::config::RpcTransactionConfig;
