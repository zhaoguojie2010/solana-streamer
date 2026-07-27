use solana_sdk::{pubkey::Pubkey, signature::Signature, transaction::VersionedTransaction};
use std::sync::Arc;

/// The lifecycle stage represented by a normalized transaction envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TxDataStage {
    PreExecutionIntent,
    ProcessedExecution,
}

/// Stable identity for a pre-execution transaction source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TxSignalSourceId {
    Shred,
    Node1,
}

/// Ordering guarantee exposed by a signal provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderGuarantee {
    NotRequired,
    ConnectionLocal,
    Global,
}

/// Provider capabilities are explicit so consumers do not infer absent fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TxSignalCapabilities {
    pub pre_execution: bool,
    pub raw_versioned_transaction: bool,
    pub slot: bool,
    pub parent_slot: bool,
    pub global_order: OrderGuarantee,
    pub provider_resolved_alt: bool,
    pub reconnect_cursor: bool,
    pub execution_meta: bool,
}

/// Source-neutral transaction input for the predictive pipeline.
#[derive(Clone, Debug)]
pub struct PreExecutionTxEnvelope {
    pub source_id: TxSignalSourceId,
    pub stage: TxDataStage,
    pub signature: Signature,
    pub slot: Option<u64>,
    pub parent_slot: Option<u64>,
    pub source_sequence: Option<u64>,
    pub entry_index: Option<u64>,
    pub tx_index_in_entry: Option<u64>,
    pub recv_us: i64,
    pub raw_transaction: Arc<VersionedTransaction>,
    pub provider_loaded_writable: Option<Arc<[Pubkey]>>,
    pub provider_loaded_readonly: Option<Arc<[Pubkey]>>,
    pub source_cursor: Option<Arc<str>>,
}
