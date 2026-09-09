//! Validated transaction storage for synchronous parsing and gRPC streaming.
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use solana_sdk::{pubkey::Pubkey, signature::Signature, transaction::VersionedTransaction};
use std::ops::Index;
use yellowstone_grpc_proto::prelude::{CompiledInstruction, InnerInstructions, TokenBalance};

use super::traits::TxExecutionStatus;
use crate::streaming::grpc::TransactionPretty;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxMetadata {
    pub signature: Signature,
    pub slot: u64,
    pub block_time: i64,
    pub block_time_ms: i64,
    pub transaction_index: Option<u64>,
    pub recv_us: i64,
    pub execution_status: TxExecutionStatus,
}

/// Projection over one transaction's key table. Construction checks every index.
/// No per-instruction `Vec<Pubkey>` is needed.
#[derive(Clone, Copy, Debug)]
pub struct InstructionAccounts<'a> {
    keys: &'a [Pubkey],
    indices: &'a [u8],
    missing: Option<usize>,
}

impl<'a> InstructionAccounts<'a> {
    pub fn new(keys: &'a [Pubkey], indices: &'a [u8]) -> Option<Self> {
        indices.iter().all(|&i| usize::from(i) < keys.len()).then_some(Self {
            keys,
            indices,
            missing: None,
        })
    }
    pub(crate) fn validated(keys: &'a [Pubkey], indices: &'a [u8]) -> Self {
        Self { keys, indices, missing: None }
    }
    pub fn len(self) -> usize {
        self.indices.len() + usize::from(self.missing.is_some())
    }
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
    pub fn get(self, index: usize) -> Option<&'a Pubkey> {
        static MISSING: Pubkey = Pubkey::new_from_array([0; 32]);
        if self.missing == Some(index) {
            return Some(&MISSING);
        }
        let physical = index.checked_sub(usize::from(self.missing.is_some_and(|m| index > m)))?;
        self.keys.get(usize::from(*self.indices.get(physical)?))
    }
    pub fn first(self) -> Option<&'a Pubkey> {
        self.get(0)
    }
    pub fn iter(self) -> impl ExactSizeIterator<Item = &'a Pubkey> + DoubleEndedIterator {
        (0..self.len()).map(move |i| self.get(i).expect("validated account index"))
    }
    pub fn indices_from(self, start: usize) -> impl Iterator<Item = u8> + 'a {
        assert!(self.missing.is_none(), "synthetic accounts have no transaction index");
        self.indices.get(start..).unwrap_or_default().iter().copied()
    }
    pub fn prefix(self, len: usize) -> Self {
        assert!(self.missing.is_none());
        Self { indices: &self.indices[..len], ..self }
    }
    /// AMM v4's 17-account layout omits the legacy target-orders account.
    pub(crate) fn with_missing(mut self, index: usize) -> Self {
        assert!(index <= self.indices.len() && self.missing.is_none());
        self.missing = Some(index);
        self
    }
}
impl Index<usize> for InstructionAccounts<'_> {
    type Output = Pubkey;
    fn index(&self, index: usize) -> &Pubkey {
        self.get(index).expect("protocol checked account count")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstructionFrame {
    pub program_id_index: u32,
    pub accounts: Vec<u8>,
    pub data: Vec<u8>,
    pub outer_index: u32,
    pub inner_index: Option<u32>,
    pub stack_height: Option<u32>,
    #[serde(skip)]
    pub(crate) group_end: usize,
}

// group_end is an internal traversal cache, not part of the serialized instruction.
impl PartialEq for InstructionFrame {
    fn eq(&self, other: &Self) -> bool {
        self.program_id_index == other.program_id_index
            && self.accounts == other.accounts
            && self.data == other.data
            && self.outer_index == other.outer_index
            && self.inner_index == other.inner_index
            && self.stack_height == other.stack_height
    }
}
impl Eq for InstructionFrame {}

#[derive(Clone, Copy, Debug)]
pub struct InstructionView<'a> {
    pub program_id: &'a Pubkey,
    pub account_indices: &'a [u8],
    pub accounts: InstructionAccounts<'a>,
    pub data: &'a [u8],
    pub outer_index: u32,
    pub inner_index: Option<u32>,
    pub stack_height: Option<u32>,
}

impl InstructionFrame {
    pub(crate) fn view<'a>(&'a self, keys: &'a [Pubkey]) -> InstructionView<'a> {
        InstructionView {
            program_id: &keys[self.program_id_index as usize],
            accounts: InstructionAccounts::validated(keys, &self.accounts),
            account_indices: &self.accounts,
            data: &self.data,
            outer_index: self.outer_index,
            inner_index: self.inner_index,
            stack_height: self.stack_height,
        }
    }
}

#[derive(Debug)]
pub struct TxFrame {
    pub(crate) meta: TxMetadata,
    pub(crate) keys: Vec<Pubkey>,
    pub(crate) instructions: Vec<InstructionFrame>,
    pub(crate) logs: Vec<String>,
    pub(crate) balances: Option<(Vec<TokenBalance>, Vec<TokenBalance>)>,
}

impl TxFrame {
    /// Move instruction buffers into a canonical outer-then-inner sequence.
    /// Invalid keys/indices are errors; the key table is never padded or compacted.
    pub fn new(
        meta: TxMetadata,
        keys: Vec<Pubkey>,
        outer: Vec<CompiledInstruction>,
        mut inner: Vec<InnerInstructions>,
        logs: Vec<String>,
    ) -> Result<Self> {
        ensure!(keys.len() <= 256, "transaction has more than 256 account keys");
        inner.sort_unstable_by_key(|group| group.index);
        ensure!(
            inner.windows(2).all(|w| w[0].index != w[1].index),
            "duplicate inner instruction group"
        );
        ensure!(
            inner.iter().all(|g| (g.index as usize) < outer.len()),
            "inner group index out of range"
        );
        let total = outer.len() + inner.iter().map(|g| g.instructions.len()).sum::<usize>();
        ensure!(total <= u32::MAX as usize, "too many instructions");
        let mut instructions = Vec::with_capacity(total);
        let mut groups = inner.into_iter().peekable();
        for (index, ix) in outer.into_iter().enumerate() {
            let start = instructions.len();
            instructions.push(InstructionFrame {
                program_id_index: ix.program_id_index,
                accounts: ix.accounts,
                data: ix.data,
                outer_index: index as u32,
                inner_index: None,
                stack_height: Some(1),
                group_end: 0,
            });
            if groups.peek().is_some_and(|g| g.index as usize == index) {
                for (inner_index, ix) in groups.next().unwrap().instructions.into_iter().enumerate()
                {
                    instructions.push(InstructionFrame {
                        program_id_index: ix.program_id_index,
                        accounts: ix.accounts,
                        data: ix.data,
                        outer_index: index as u32,
                        inner_index: Some(inner_index as u32),
                        stack_height: ix.stack_height,
                        group_end: 0,
                    });
                }
            }
            let end = instructions.len();
            for ix in &mut instructions[start..end] {
                ensure!(
                    (ix.program_id_index as usize) < keys.len(),
                    "program index out of range at outer {index}"
                );
                ensure!(
                    ix.accounts.iter().all(|&i| usize::from(i) < keys.len()),
                    "account index out of range at outer {index}"
                );
                ix.group_end = end;
            }
        }
        Ok(Self { meta, keys, instructions, logs, balances: None })
    }
    pub fn metadata(&self) -> &TxMetadata {
        &self.meta
    }
    pub fn keys(&self) -> &[Pubkey] {
        &self.keys
    }
    pub fn logs(&self) -> &[String] {
        &self.logs
    }
    pub fn instructions(&self) -> impl ExactSizeIterator<Item = InstructionView<'_>> {
        self.instructions.iter().map(|ix| ix.view(&self.keys))
    }
    pub fn instruction(&self, index: u32) -> Option<InstructionView<'_>> {
        self.instructions.get(index as usize).map(|ix| ix.view(&self.keys))
    }
    /// Adapt an owned SDK transaction with its resolved static + writable ALT + readonly ALT keys.
    /// Execution status remains Unknown unless the caller supplies execution evidence.
    pub fn from_versioned(
        transaction: VersionedTransaction,
        keys: Vec<Pubkey>,
        inner: Vec<InnerInstructions>,
        logs: Vec<String>,
        mut meta: TxMetadata,
    ) -> Result<Self> {
        meta.signature =
            *transaction.signatures.first().context("transaction signature missing")?;
        ensure!(
            keys.starts_with(transaction.message.static_account_keys()),
            "resolved keys must start with the static account keys"
        );
        let instructions = match transaction.message {
            solana_sdk::message::VersionedMessage::Legacy(m) => m.instructions,
            solana_sdk::message::VersionedMessage::V0(m) => m.instructions,
        };
        let outer = instructions
            .into_iter()
            .map(|ix| CompiledInstruction {
                program_id_index: u32::from(ix.program_id_index),
                accounts: ix.accounts,
                data: ix.data,
            })
            .collect();
        Self::new(meta, keys, outer, inner, logs)
    }
}

impl TryFrom<TransactionPretty> for TxFrame {
    type Error = anyhow::Error;
    fn try_from(value: TransactionPretty) -> Result<Self> {
        let info = value.grpc_tx;
        let transaction = info.transaction.context("transaction body missing")?;
        let message = transaction.message.context("transaction message missing")?;
        let mut execution = info.meta;
        let loaded_len = execution
            .as_ref()
            .map_or(0, |m| m.loaded_writable_addresses.len() + m.loaded_readonly_addresses.len());
        let mut keys = Vec::with_capacity(message.account_keys.len() + loaded_len);
        let mut push_key = |bytes: Vec<u8>| -> Result<()> {
            let key: [u8; 32] =
                bytes.try_into().map_err(|_| anyhow::anyhow!("invalid account key length"))?;
            keys.push(Pubkey::new_from_array(key));
            Ok(())
        };
        for key in message.account_keys {
            push_key(key)?;
        }
        if let Some(m) = execution.as_mut() {
            for key in std::mem::take(&mut m.loaded_writable_addresses) {
                push_key(key)?;
            }
            for key in std::mem::take(&mut m.loaded_readonly_addresses) {
                push_key(key)?;
            }
        }
        let signature =
            Signature::try_from(info.signature.as_slice()).context("invalid signature length")?;
        let time = value.block_time.unwrap_or_default();
        let meta = TxMetadata {
            signature,
            slot: value.slot,
            block_time: time.seconds,
            block_time_ms: time
                .seconds
                .saturating_mul(1000)
                .saturating_add(i64::from(time.nanos) / 1_000_000),
            transaction_index: Some(info.index),
            recv_us: value.recv_us,
            execution_status: execution.as_ref().map_or(TxExecutionStatus::Unknown, |m| {
                if m.err.is_some() {
                    TxExecutionStatus::Failed
                } else {
                    TxExecutionStatus::Success
                }
            }),
        };
        let (inner, logs, balances) = match execution {
            Some(m) => (
                m.inner_instructions,
                m.log_messages,
                Some((m.pre_token_balances, m.post_token_balances)),
            ),
            None => (Vec::new(), Vec::new(), None),
        };
        let mut frame = Self::new(meta, keys, message.instructions, inner, logs)?;
        frame.balances = balances;
        Ok(frame)
    }
}
