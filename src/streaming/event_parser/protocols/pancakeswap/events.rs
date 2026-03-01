use crate::streaming::event_parser::common::EventMetadata;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

/// PancakeSwap V3 Swap 事件
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PancakeSwapSwapEvent {
    pub metadata: EventMetadata,

    // 指令参数
    pub amount: u64,
    pub other_amount_threshold: u64,
    pub sqrt_price_limit: u128,
    pub is_base_input: Option<bool>,

    // Program data 日志里的 SwapEvent 数据
    pub amount_0: u64,
    pub transfer_fee_0: u64,
    pub amount_1: u64,
    pub transfer_fee_1: u64,
    pub zero_for_one: bool,
    pub sqrt_price_x64: u128,
    pub liquidity: u128,
    pub tick: i32,

    // 指令账户（按链上观测到的 account 索引顺序）
    pub token_authority: Pubkey,
    pub pool: Pubkey,
    pub input_token_account: Pubkey,
    pub output_token_account: Pubkey,
    pub input_vault: Pubkey,
    pub output_vault: Pubkey,
    pub account_6: Pubkey,
    pub account_7: Pubkey,
    pub token_program: Pubkey,
    pub account_9: Pubkey,
    pub account_10: Pubkey,
    pub account_11: Pubkey,
    pub remaining_accounts: Vec<Pubkey>,
}

/// PancakeSwap V3 SwapV2 事件
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PancakeSwapSwapV2Event {
    pub metadata: EventMetadata,

    // 指令参数
    pub amount: u64,
    pub other_amount_threshold: u64,
    pub sqrt_price_limit: u128,
    pub is_base_input: Option<bool>,

    // Program data 日志里的 SwapEvent 数据
    pub amount_0: u64,
    pub transfer_fee_0: u64,
    pub amount_1: u64,
    pub transfer_fee_1: u64,
    pub zero_for_one: bool,
    pub sqrt_price_x64: u128,
    pub liquidity: u128,
    pub tick: i32,

    // 指令账户（按链上观测到的 account 索引顺序）
    pub token_authority: Pubkey,
    pub pool: Pubkey,
    pub input_token_account: Pubkey,
    pub output_token_account: Pubkey,
    pub input_vault: Pubkey,
    pub output_vault: Pubkey,
    pub input_mint: Pubkey,
    pub output_mint: Pubkey,
    pub account_6: Pubkey,
    pub account_7: Pubkey,
    pub token_program_a: Pubkey,
    pub token_program_b: Pubkey,
    pub memo_program: Pubkey,
    pub account_13: Pubkey,
    pub account_14: Pubkey,
    pub account_15: Pubkey,
    pub account_16: Pubkey,
    pub account_17: Pubkey,
    pub account_18: Pubkey,
    pub account_19: Pubkey,
    pub remaining_accounts: Vec<Pubkey>,
}

/// 事件鉴别器常量
pub mod discriminators {
    // 指令鉴别器
    pub const SWAP: &[u8] = &[248, 198, 158, 145, 225, 117, 135, 200];
    pub const SWAP_V2: &[u8] = &[43, 4, 237, 11, 26, 201, 30, 98];
    // Anchor event: SwapEvent
    pub const SWAP_EVENT: &[u8] = &[64, 198, 205, 232, 38, 8, 113, 226];
}
