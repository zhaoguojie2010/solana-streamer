use crate::streaming::event_parser::common::types::{EventType, ProtocolType};
use crate::streaming::event_parser::common::EventMetadata;
use crate::streaming::event_parser::core::traits::TxEvent;
use borsh::BorshDeserialize;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

// Compute Budget Program ID
pub const COMPUTE_BUDGET_PROGRAM_ID: Pubkey =
    solana_sdk::pubkey!("ComputeBudget111111111111111111111111111111");

/// SetComputeUnitLimit 事件
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, BorshDeserialize)]
pub struct SetComputeUnitLimitEvent {
    #[borsh(skip)]
    pub metadata: EventMetadata,
    /// 请求的计算单元数量
    pub units: u32,
}

/// SetComputeUnitPrice 事件
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, BorshDeserialize)]
pub struct SetComputeUnitPriceEvent {
    #[borsh(skip)]
    pub metadata: EventMetadata,
    /// 每个计算单元的价格 (micro-lamports)
    pub micro_lamports: u64,
}

pub struct CommonEventParser {}

impl CommonEventParser {
    /// 解析 Compute Budget 指令
    pub fn parse_compute_budget_instruction(
        instruction_data: &[u8],
        mut metadata: EventMetadata,
    ) -> Option<TxEvent> {
        if instruction_data.is_empty() {
            return None;
        }

        // 设置 protocol 为 Common
        metadata.protocol = ProtocolType::Common;

        // Compute Budget 指令使用单字节判别器
        match instruction_data[0] {
            // SetComputeUnitLimit: discriminator = 2
            2 => {
                if instruction_data.len() < 5 {
                    return None;
                }
                let units = u32::from_le_bytes(instruction_data[1..5].try_into().ok()?);
                metadata.event_type = EventType::SetComputeUnitLimit;
                let event = SetComputeUnitLimitEvent { metadata, units };
                Some(TxEvent::SetComputeUnitLimitEvent(event))
            }
            // SetComputeUnitPrice: discriminator = 3
            3 => {
                if instruction_data.len() < 9 {
                    return None;
                }
                let micro_lamports = u64::from_le_bytes(instruction_data[1..9].try_into().ok()?);
                metadata.event_type = EventType::SetComputeUnitPrice;
                let event = SetComputeUnitPriceEvent { metadata, micro_lamports };
                Some(TxEvent::SetComputeUnitPriceEvent(event))
            }
            _ => None,
        }
    }
}
