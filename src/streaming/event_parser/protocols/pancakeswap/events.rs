use crate::streaming::event_parser::common::EventMetadata;
use crate::streaming::event_parser::protocols::pancakeswap::types::{
    PoolState, TickArrayBitmapExtension, TickArrayState,
};
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
    pub is_base_input: bool,

    // Program data 日志里的 SwapEvent 数据
    pub amount_0: u64,
    pub transfer_fee_0: u64,
    pub amount_1: u64,
    pub transfer_fee_1: u64,
    pub zero_for_one: bool,
    pub sqrt_price_x64: u128,
    pub liquidity: u128,
    pub tick: i32,
    pub log_pool_state: Pubkey,
    pub log_sender: Pubkey,
    pub log_input_token_account: Pubkey,
    pub log_output_token_account: Pubkey,

    // 指令账户（IDL）
    pub payer: Pubkey,
    pub amm_config: Pubkey,
    pub pool_state: Pubkey,
    pub input_token_account: Pubkey,
    pub output_token_account: Pubkey,
    pub input_vault: Pubkey,
    pub output_vault: Pubkey,
    pub observation_state: Pubkey,
    pub token_program: Pubkey,
    pub tick_array: Pubkey,
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
    pub is_base_input: bool,

    // Program data 日志里的 SwapEvent 数据
    pub amount_0: u64,
    pub transfer_fee_0: u64,
    pub amount_1: u64,
    pub transfer_fee_1: u64,
    pub zero_for_one: bool,
    pub sqrt_price_x64: u128,
    pub liquidity: u128,
    pub tick: i32,
    pub log_pool_state: Pubkey,
    pub log_sender: Pubkey,
    pub log_input_token_account: Pubkey,
    pub log_output_token_account: Pubkey,

    // 指令账户（IDL）
    pub payer: Pubkey,
    pub amm_config: Pubkey,
    pub pool_state: Pubkey,
    pub input_token_account: Pubkey,
    pub output_token_account: Pubkey,
    pub input_vault: Pubkey,
    pub output_vault: Pubkey,
    pub observation_state: Pubkey,
    pub token_program: Pubkey,
    pub token_program_2022: Pubkey,
    pub memo_program: Pubkey,
    pub input_mint: Pubkey,
    pub output_mint: Pubkey,
    pub remaining_accounts: Vec<Pubkey>,
}

/// PancakeSwap PoolState 账户事件
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PancakeSwapPoolStateAccountEvent {
    pub metadata: EventMetadata,
    pub pubkey: Pubkey,
    pub executable: bool,
    pub lamports: u64,
    pub owner: Pubkey,
    pub rent_epoch: u64,
    #[serde(skip)]
    pub raw_account_data: Vec<u8>,
    pub pool_state: PoolState,
}

/// PancakeSwap TickArrayState 账户事件
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PancakeSwapTickArrayStateAccountEvent {
    pub metadata: EventMetadata,
    pub pubkey: Pubkey,
    pub executable: bool,
    pub lamports: u64,
    pub owner: Pubkey,
    pub rent_epoch: u64,
    #[serde(skip)]
    pub raw_account_data: Vec<u8>,
    pub tick_array_state: TickArrayState,
}

/// PancakeSwap TickArrayBitmapExtension 账户事件
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PancakeSwapTickArrayBitmapExtensionAccountEvent {
    pub metadata: EventMetadata,
    pub pubkey: Pubkey,
    pub executable: bool,
    pub lamports: u64,
    pub owner: Pubkey,
    pub rent_epoch: u64,
    #[serde(skip)]
    pub raw_account_data: Vec<u8>,
    pub tick_array_bitmap_extension: TickArrayBitmapExtension,
}

/// 事件鉴别器常量
pub mod discriminators {
    // 指令鉴别器
    pub const SWAP: &[u8] = &[248, 198, 158, 145, 225, 117, 135, 200];
    pub const SWAP_V2: &[u8] = &[43, 4, 237, 11, 26, 201, 30, 98];
    // Anchor event: SwapEvent
    pub const SWAP_EVENT: &[u8] = &[64, 198, 205, 232, 38, 8, 113, 226];
}
