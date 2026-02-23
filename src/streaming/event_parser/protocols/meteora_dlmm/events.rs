use crate::streaming::event_parser::common::EventMetadata;
use crate::streaming::event_parser::protocols::meteora_dlmm::types::{
    BinArray, BinArrayBitmapExtension, LbPair,
};
use borsh::BorshDeserialize;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

/// Meteora DLMM Swap event
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeteoraDlmmSwapEvent {
    pub metadata: EventMetadata,

    // Instruction params
    pub amount_in: u64,
    pub min_amount_out: u64,

    // CPI log details
    pub lb_pair: Pubkey,
    pub from: Pubkey,
    pub start_bin_id: i32,
    pub end_bin_id: i32,
    pub cpi_amount_in: u64,
    pub cpi_amount_out: u64,
    pub swap_for_y: bool,
    pub fee: u64,
    pub protocol_fee: u64,
    pub fee_bps: u128,
    pub host_fee: u64,

    // Instruction accounts
    pub bin_array_bitmap_extension: Option<Pubkey>,
    pub reserve_x: Option<Pubkey>,
    pub reserve_y: Option<Pubkey>,
    pub user_token_in: Option<Pubkey>,
    pub user_token_out: Option<Pubkey>,
    pub token_x_mint: Option<Pubkey>,
    pub token_y_mint: Option<Pubkey>,
    pub oracle: Option<Pubkey>,
    pub host_fee_in: Option<Pubkey>,
    pub user: Pubkey,
    pub token_x_program: Pubkey,
    pub token_y_program: Pubkey,
    pub event_authority: Pubkey,
    pub program: Pubkey,
    pub remaining_accounts: Vec<Pubkey>,
}

/// Meteora DLMM swap result from CPI log
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, BorshDeserialize)]
pub struct MeteoraDlmmSwapResult {
    pub amount_in: u64,
    pub amount_left: u64,
    pub amount_out: u64,
    pub total_fee: u64,
    pub lp_mm_fee: u64,
    pub protocol_fee: u64,
    pub host_fee: u64,
    pub lp_limit_order_fee: u64,
    pub limit_order_filled_amount: u64,
    pub limit_order_swapped_amount: u64,
}

/// Meteora DLMM Swap2 event
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeteoraDlmmSwap2Event {
    pub metadata: EventMetadata,

    // Instruction params
    pub amount_in: u64,
    pub min_amount_out: u64,

    // CPI log details
    pub lb_pair: Pubkey,
    pub from: Pubkey,
    pub start_bin_id: i32,
    pub end_bin_id: i32,
    pub swap_for_y: bool,
    pub fee_bps: u128,
    pub swap_result: MeteoraDlmmSwapResult,

    // Instruction accounts
    pub bin_array_bitmap_extension: Option<Pubkey>,
    pub reserve_x: Option<Pubkey>,
    pub reserve_y: Option<Pubkey>,
    pub user_token_in: Option<Pubkey>,
    pub user_token_out: Option<Pubkey>,
    pub token_x_mint: Option<Pubkey>,
    pub token_y_mint: Option<Pubkey>,
    pub oracle: Option<Pubkey>,
    pub host_fee_in: Option<Pubkey>,
    pub user: Pubkey,
    pub token_x_program: Pubkey,
    pub token_y_program: Pubkey,
    pub memo_program: Pubkey,
    pub event_authority: Pubkey,
    pub program: Pubkey,
    pub remaining_accounts: Vec<Pubkey>,
}

/// Raw swap CPI event payload
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, BorshDeserialize)]
pub struct MeteoraDlmmSwapCpiEventData {
    pub lb_pair: Pubkey,
    pub from: Pubkey,
    pub start_bin_id: i32,
    pub end_bin_id: i32,
    pub amount_in: u64,
    pub amount_out: u64,
    pub swap_for_y: bool,
    pub fee: u64,
    pub protocol_fee: u64,
    pub fee_bps: u128,
    pub host_fee: u64,
}

/// Raw swap2 CPI event payload
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, BorshDeserialize)]
pub struct MeteoraDlmmSwap2CpiEventData {
    pub lb_pair: Pubkey,
    pub from: Pubkey,
    pub start_bin_id: i32,
    pub end_bin_id: i32,
    pub swap_for_y: bool,
    pub fee_bps: u128,
    pub swap_result: MeteoraDlmmSwapResult,
}

/// LbPair 账户事件
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeteoraDlmmLbPairAccountEvent {
    pub metadata: EventMetadata,
    pub pubkey: Pubkey,
    pub executable: bool,
    pub lamports: u64,
    pub owner: Pubkey,
    pub rent_epoch: u64,
    #[serde(skip)]
    pub raw_account_data: Vec<u8>,
    pub lb_pair: LbPair,
}

/// BinArray 账户事件
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeteoraDlmmBinArrayAccountEvent {
    pub metadata: EventMetadata,
    pub pubkey: Pubkey,
    pub executable: bool,
    pub lamports: u64,
    pub owner: Pubkey,
    pub rent_epoch: u64,
    #[serde(skip)]
    pub raw_account_data: Vec<u8>,
    pub bin_array: BinArray,
}

/// BinArrayBitmapExtension 账户事件
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeteoraDlmmBinArrayBitmapExtensionAccountEvent {
    pub metadata: EventMetadata,
    pub pubkey: Pubkey,
    pub executable: bool,
    pub lamports: u64,
    pub owner: Pubkey,
    pub rent_epoch: u64,
    #[serde(skip)]
    pub raw_account_data: Vec<u8>,
    pub bin_array_bitmap_extension: BinArrayBitmapExtension,
}

/// 事件鉴别器常量
pub mod discriminators {
    // Instruction discriminators
    pub const SWAP_IX: &[u8] = &[0xf8, 0xc6, 0x9e, 0x91, 0xe1, 0x75, 0x87, 0xc8];
    pub const SWAP2_IX: &[u8] = &[0x41, 0x4b, 0x3f, 0x4c, 0xeb, 0x5b, 0x5b, 0x88];

    // CPI event discriminators
    // Prefix: e445a52e51cb9a1d
    pub const SWAP_EVENT: &[u8] = &[
        0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d, 0x51, 0x6c, 0xe3, 0xbe, 0xcd, 0xd0, 0x0a,
        0xc4,
    ];
    pub const SWAP2_EVENT: &[u8] = &[
        0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d, 0x2e, 0x74, 0x52, 0xd7, 0x94, 0x1b, 0x54,
        0x4d,
    ];

    // 账户鉴别器
    pub const LB_PAIR: &[u8] = &[33, 11, 49, 98, 181, 101, 177, 13];
    pub const BIN_ARRAY: &[u8] = &[92, 142, 92, 220, 5, 148, 70, 181];
    pub const BIN_ARRAY_BITMAP_EXTENSION: &[u8] = &[80, 111, 124, 113, 55, 237, 18, 5];
}

pub const METEORA_DLMM_SWAP_EVENT_LOG_SIZE: usize = 129;
pub fn meteora_dlmm_swap_event_decode(data: &[u8]) -> Option<MeteoraDlmmSwapCpiEventData> {
    if data.len() < METEORA_DLMM_SWAP_EVENT_LOG_SIZE {
        return None;
    }
    borsh::from_slice::<MeteoraDlmmSwapCpiEventData>(&data[..METEORA_DLMM_SWAP_EVENT_LOG_SIZE]).ok()
}

pub const METEORA_DLMM_SWAP2_EVENT_LOG_SIZE: usize = 169;
pub fn meteora_dlmm_swap2_event_decode(data: &[u8]) -> Option<MeteoraDlmmSwap2CpiEventData> {
    if data.len() < METEORA_DLMM_SWAP2_EVENT_LOG_SIZE {
        return None;
    }
    borsh::from_slice::<MeteoraDlmmSwap2CpiEventData>(&data[..METEORA_DLMM_SWAP2_EVENT_LOG_SIZE])
        .ok()
}
