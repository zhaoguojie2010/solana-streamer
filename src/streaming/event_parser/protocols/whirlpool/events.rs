use crate::streaming::event_parser::common::EventMetadata;
use crate::streaming::event_parser::protocols::whirlpool::types::{Whirlpool, WhirlpoolTickArray};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

/// Whirlpool Swap 事件
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WhirlpoolSwapEvent {
    pub metadata: EventMetadata,

    // 指令参数
    pub amount: u64,
    pub other_amount_threshold: u64,
    pub sqrt_price_limit: u128,
    pub amount_specified_is_input: bool,
    pub a_to_b: bool,

    // Program data 日志里的 Traded 事件数据
    pub pre_sqrt_price: u128,
    pub post_sqrt_price: u128,
    pub input_amount: u64,
    pub output_amount: u64,
    pub input_transfer_fee: u64,
    pub output_transfer_fee: u64,
    pub lp_fee: u64,
    pub protocol_fee: u64,

    // 指令账户
    pub token_program: Pubkey,
    pub token_authority: Pubkey,
    pub whirlpool: Pubkey,
    pub token_owner_account_a: Pubkey,
    pub token_vault_a: Pubkey,
    pub token_owner_account_b: Pubkey,
    pub token_vault_b: Pubkey,
    pub tick_array_0: Pubkey,
    pub tick_array_1: Pubkey,
    pub tick_array_2: Pubkey,
    pub oracle: Pubkey,
    pub remaining_accounts: Vec<Pubkey>,
}

/// Whirlpool SwapV2 事件
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WhirlpoolSwapV2Event {
    pub metadata: EventMetadata,

    // 指令参数
    pub amount: u64,
    pub other_amount_threshold: u64,
    pub sqrt_price_limit: u128,
    pub amount_specified_is_input: bool,
    pub a_to_b: bool,

    // Program data 日志里的 Traded 事件数据
    pub pre_sqrt_price: u128,
    pub post_sqrt_price: u128,
    pub input_amount: u64,
    pub output_amount: u64,
    pub input_transfer_fee: u64,
    pub output_transfer_fee: u64,
    pub lp_fee: u64,
    pub protocol_fee: u64,

    // 指令账户
    pub token_program_a: Pubkey,
    pub token_program_b: Pubkey,
    pub memo_program: Pubkey,
    pub token_authority: Pubkey,
    pub whirlpool: Pubkey,
    pub token_mint_a: Pubkey,
    pub token_mint_b: Pubkey,
    pub token_owner_account_a: Pubkey,
    pub token_vault_a: Pubkey,
    pub token_owner_account_b: Pubkey,
    pub token_vault_b: Pubkey,
    pub tick_array_0: Pubkey,
    pub tick_array_1: Pubkey,
    pub tick_array_2: Pubkey,
    pub oracle: Pubkey,
    pub remaining_accounts: Vec<Pubkey>,
}

/// Whirlpool 账户事件
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WhirlpoolAccountEvent {
    pub metadata: EventMetadata,
    pub pubkey: Pubkey,
    pub executable: bool,
    pub lamports: u64,
    pub owner: Pubkey,
    pub rent_epoch: u64,
    #[serde(skip)]
    pub raw_account_data: Vec<u8>,
    pub whirlpool: Whirlpool,
}

/// Whirlpool TickArray 账户事件
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WhirlpoolTickArrayAccountEvent {
    pub metadata: EventMetadata,
    pub pubkey: Pubkey,
    pub executable: bool,
    pub lamports: u64,
    pub owner: Pubkey,
    pub rent_epoch: u64,
    #[serde(skip)]
    pub raw_account_data: Vec<u8>,
    pub tick_array: WhirlpoolTickArray,
}

/// 事件鉴别器常量
pub mod discriminators {
    // 指令鉴别器
    pub const SWAP: &[u8] = &[248, 198, 158, 145, 225, 117, 135, 200];
    pub const SWAP_V2: &[u8] = &[43, 4, 237, 11, 26, 201, 30, 98];
    // Anchor event: Traded
    pub const TRADED_EVENT: &[u8] = &[225, 202, 73, 175, 147, 43, 160, 150];

    // 账户鉴别器 - Anchor discriminator for "Whirlpool" account
    // 这是通过 Anchor 的账户名称 "account:Whirlpool" 计算得出的 8 字节哈希
    pub const WHIRLPOOL: &[u8] = &[63, 149, 209, 12, 225, 128, 99, 9];
    // 账户鉴别器 - Anchor discriminator for "TickArray" account
    // 这是通过 Anchor 的账户名称 "account:TickArray" 计算得出的 8 字节哈希
    pub const TICK_ARRAY: &[u8] = &[69, 97, 189, 190, 110, 7, 66, 187];
}
