use crate::streaming::event_parser::{TxExecutionStatus, TxFrame, TxMetadata};
use anyhow::{Context, Result};
use yellowstone_grpc_proto::prelude::{InnerInstructions, TokenBalance};

/// Adapt an RPC response requested with Base64 transaction encoding. All loaded keys
/// and compiled CPI positions are validated before the synchronous parser sees them.
pub fn transaction_frame(
    value: super::types::EncodedConfirmedTransactionWithStatusMeta,
    recv_us: i64,
) -> Result<TxFrame> {
    use super::types::{option_serializer::OptionSerializer, UiInstruction};
    let transaction =
        value.transaction.transaction.decode().context("RPC transaction decode failed")?;
    let static_keys = transaction.message.static_account_keys();
    let execution = value.transaction.meta;
    let loaded_len = execution.as_ref().map_or(0, |m| match &m.loaded_addresses {
        OptionSerializer::Some(a) => a.writable.len() + a.readonly.len(),
        _ => 0,
    });
    let mut keys = Vec::with_capacity(static_keys.len() + loaded_len);
    keys.extend_from_slice(static_keys);
    let mut inner = Vec::new();
    let mut logs = Vec::new();
    let mut balances = None;
    let mut status = TxExecutionStatus::Unknown;
    if let Some(meta) = execution {
        status =
            if meta.err.is_some() { TxExecutionStatus::Failed } else { TxExecutionStatus::Success };
        if let OptionSerializer::Some(addresses) = meta.loaded_addresses {
            for key in addresses.writable.into_iter().chain(addresses.readonly) {
                keys.push(key.parse().context("invalid RPC loaded key")?);
            }
        }
        if let OptionSerializer::Some(groups) = meta.inner_instructions {
            inner.reserve(groups.len());
            for group in groups {
                let mut instructions = Vec::with_capacity(group.instructions.len());
                for ix in group.instructions {
                    let UiInstruction::Compiled(ix) = ix else {
                        anyhow::bail!("RPC inner instruction is parsed; request Base64 encoding");
                    };
                    instructions.push(yellowstone_grpc_proto::prelude::InnerInstruction {
                        program_id_index: u32::from(ix.program_id_index),
                        accounts: ix.accounts,
                        data: solana_sdk::bs58::decode(ix.data)
                            .into_vec()
                            .context("invalid RPC instruction data")?,
                        stack_height: ix.stack_height,
                    });
                }
                inner.push(InnerInstructions { index: u32::from(group.index), instructions });
            }
        }
        if let OptionSerializer::Some(messages) = meta.log_messages {
            logs = messages;
        }
        let convert = |input: OptionSerializer<Vec<super::types::UiTransactionTokenBalance>>| {
            if let OptionSerializer::Some(input) = input {
                input
                    .into_iter()
                    .map(|b| TokenBalance {
                        account_index: u32::from(b.account_index),
                        mint: b.mint,
                        owner: match b.owner {
                            OptionSerializer::Some(s) => s,
                            _ => String::new(),
                        },
                        program_id: match b.program_id {
                            OptionSerializer::Some(s) => s,
                            _ => String::new(),
                        },
                        ui_token_amount: Some(yellowstone_grpc_proto::prelude::UiTokenAmount {
                            ui_amount: b.ui_token_amount.ui_amount.unwrap_or_default(),
                            decimals: u32::from(b.ui_token_amount.decimals),
                            amount: b.ui_token_amount.amount,
                            ui_amount_string: b.ui_token_amount.ui_amount_string,
                        }),
                    })
                    .collect()
            } else {
                Vec::new()
            }
        };
        balances = Some((convert(meta.pre_token_balances), convert(meta.post_token_balances)));
    }
    let meta = TxMetadata {
        slot: value.slot,
        block_time: value.block_time.unwrap_or_default(),
        block_time_ms: value.block_time.unwrap_or_default().saturating_mul(1000),
        recv_us,
        execution_status: status,
        ..TxMetadata::default()
    };
    let mut frame = TxFrame::from_versioned(transaction, keys, inner, logs, meta)?;
    frame.balances = balances;
    Ok(frame)
}
