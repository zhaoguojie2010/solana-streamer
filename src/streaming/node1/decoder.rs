use crate::streaming::signal::{PreExecutionTxEnvelope, TxDataStage, TxSignalSourceId};
use bincode::Options;
use solana_sdk::{signature::Signature, transaction::VersionedTransaction};
use std::sync::Arc;
use thiserror::Error;

const SLOT_PREFIX_BYTES: usize = 8;

#[derive(Debug, Error)]
pub enum Node1DecodeError {
    #[error("datagram is too short: bytes={bytes}, minimum={minimum}")]
    DatagramTooShort { bytes: usize, minimum: usize },
    #[error("transaction decode failed: transaction_bytes={transaction_bytes}, error={error}")]
    TransactionDecode { transaction_bytes: usize, error: bincode::Error },
    #[error("transaction sanitize failed: {0}")]
    Sanitize(String),
    #[error("transaction has no signature")]
    MissingSignature,
    #[error("transaction first signature is the default signature")]
    DefaultSignature,
}

/// Decode one Node1 datagram without inferring fields absent from the wire contract.
pub fn decode_datagram(
    datagram: &[u8],
    recv_us: i64,
) -> Result<PreExecutionTxEnvelope, Node1DecodeError> {
    if datagram.len() <= SLOT_PREFIX_BYTES {
        return Err(Node1DecodeError::DatagramTooShort {
            bytes: datagram.len(),
            minimum: SLOT_PREFIX_BYTES + 1,
        });
    }

    let slot_bytes: [u8; SLOT_PREFIX_BYTES] =
        datagram[..SLOT_PREFIX_BYTES].try_into().map_err(|_| {
            Node1DecodeError::DatagramTooShort {
                bytes: datagram.len(),
                minimum: SLOT_PREFIX_BYTES + 1,
            }
        })?;
    let slot = u64::from_le_bytes(slot_bytes);
    let transaction_bytes = &datagram[SLOT_PREFIX_BYTES..];
    let transaction: VersionedTransaction = bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .reject_trailing_bytes()
        .deserialize(transaction_bytes)
        .map_err(|error| Node1DecodeError::TransactionDecode {
            transaction_bytes: transaction_bytes.len(),
            error,
        })?;
    transaction.sanitize().map_err(|error| Node1DecodeError::Sanitize(error.to_string()))?;

    let signature =
        transaction.signatures.first().copied().ok_or(Node1DecodeError::MissingSignature)?;
    if signature == Signature::default() {
        return Err(Node1DecodeError::DefaultSignature);
    }

    Ok(PreExecutionTxEnvelope {
        source_id: TxSignalSourceId::Node1,
        stage: TxDataStage::PreExecutionIntent,
        signature,
        slot: Some(slot),
        parent_slot: None,
        source_sequence: None,
        entry_index: None,
        tx_index_in_entry: None,
        recv_us,
        raw_transaction: Arc::new(transaction),
        provider_loaded_writable: None,
        provider_loaded_readonly: None,
        source_cursor: None,
    })
}
