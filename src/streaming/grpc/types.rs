use crate::streaming::event_parser::common::high_performance_clock::get_high_perf_clock;
use anyhow::{Context, Result};
use solana_sdk::{pubkey::Pubkey, signature::Signature};
use std::{collections::HashMap, fmt};
use yellowstone_grpc_proto::{
    geyser::{
        SubscribeRequestFilterAccounts, SubscribeRequestFilterTransactions, SubscribeUpdateAccount,
        SubscribeUpdateBlockMeta, SubscribeUpdateTransaction, SubscribeUpdateTransactionInfo,
    },
    prost_types::Timestamp,
};

pub type TransactionsFilterMap = HashMap<String, SubscribeRequestFilterTransactions>;
pub type AccountsFilterMap = HashMap<String, SubscribeRequestFilterAccounts>;

#[derive(Clone, Default)]
pub struct AccountFrame {
    pub slot: u64,
    pub write_version: u64,
    pub is_startup: bool,
    pub signature: Signature,
    pub pubkey: Pubkey,
    pub executable: bool,
    pub lamports: u64,
    pub owner: Pubkey,
    pub rent_epoch: u64,
    pub data: bytes::Bytes,
    pub recv_us: i64,
}

impl fmt::Debug for AccountFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AccountFrame")
            .field("slot", &self.slot)
            .field("write_version", &self.write_version)
            .field("is_startup", &self.is_startup)
            .field("signature", &self.signature)
            .field("pubkey", &self.pubkey)
            .field("executable", &self.executable)
            .field("lamports", &self.lamports)
            .field("owner", &self.owner)
            .field("rent_epoch", &self.rent_epoch)
            .field("data", &self.data)
            .finish()
    }
}

#[derive(Clone, Default)]
pub struct BlockMetaPretty {
    pub slot: u64,
    pub block_hash: String,
    pub block_time: Option<Timestamp>,
    pub recv_us: i64,
}

impl fmt::Debug for BlockMetaPretty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BlockMetaPretty")
            .field("slot", &self.slot)
            .field("block_hash", &self.block_hash)
            .field("block_time", &self.block_time)
            .field("recv_us", &self.recv_us)
            .finish()
    }
}

#[derive(Clone)]
pub struct TransactionPretty {
    pub slot: u64,
    pub transaction_index: Option<u64>, // 新增：交易在slot中的索引
    pub block_hash: String,
    pub block_time: Option<Timestamp>,
    pub signature: Signature,
    pub is_vote: bool,
    pub recv_us: i64,
    pub grpc_tx: SubscribeUpdateTransactionInfo,
}

impl fmt::Debug for TransactionPretty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TransactionPretty")
            .field("slot", &self.slot)
            .field("transaction_index", &self.transaction_index)
            .field("signature", &self.signature)
            .field("is_vote", &self.is_vote)
            .field("recv_us", &self.recv_us)
            .finish()
    }
}

impl Default for TransactionPretty {
    fn default() -> Self {
        Self {
            slot: 0,
            transaction_index: None,
            block_hash: String::new(),
            block_time: None,
            signature: Signature::default(),
            is_vote: false,
            grpc_tx: SubscribeUpdateTransactionInfo::default(),
            recv_us: 0,
        }
    }
}

/// Move protobuf payloads into the event wrapper without allocating a pooled shell.
impl TryFrom<SubscribeUpdateAccount> for AccountFrame {
    type Error = anyhow::Error;
    fn try_from(update: SubscribeUpdateAccount) -> Result<Self> {
        let info = update.account.context("account update missing account info")?;
        Ok(Self {
            slot: update.slot,
            write_version: info.write_version,
            is_startup: update.is_startup,
            signature: info
                .txn_signature
                .map(|signature| {
                    Signature::try_from(signature.as_slice()).context("invalid signature")
                })
                .transpose()?
                .unwrap_or_default(),
            pubkey: Pubkey::try_from(info.pubkey.as_slice()).context("invalid pubkey")?,
            executable: info.executable,
            lamports: info.lamports,
            owner: Pubkey::try_from(info.owner.as_slice()).context("invalid pubkey")?,
            rent_epoch: info.rent_epoch,
            data: info.data,
            recv_us: get_high_perf_clock(),
        })
    }
}

impl From<(SubscribeUpdateBlockMeta, Option<Timestamp>)> for BlockMetaPretty {
    fn from((update, block_time): (SubscribeUpdateBlockMeta, Option<Timestamp>)) -> Self {
        Self {
            slot: update.slot,
            block_hash: update.blockhash,
            block_time,
            recv_us: get_high_perf_clock(),
        }
    }
}

impl TryFrom<(SubscribeUpdateTransaction, Option<Timestamp>)> for TransactionPretty {
    type Error = anyhow::Error;
    fn try_from(
        (update, block_time): (SubscribeUpdateTransaction, Option<Timestamp>),
    ) -> Result<Self> {
        let info = update.transaction.context("transaction update missing transaction info")?;
        Ok(Self {
            slot: update.slot,
            transaction_index: Some(info.index),
            block_hash: String::new(),
            block_time,
            signature: Signature::try_from(info.signature.as_slice())
                .context("invalid signature")?,
            is_vote: info.is_vote,
            recv_us: get_high_perf_clock(),
            grpc_tx: info,
        })
    }
}
