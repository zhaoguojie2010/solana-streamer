use crate::streaming::event_parser::common::EventMetadata;
use crate::streaming::event_parser::protocols::meteora_dlmm::types::{
    BinArray, BinArrayBitmapExtension, LbPair,
};
use borsh::BorshDeserialize;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MeteoraDlmmInstructionKind {
    InitializeLbPair,
    InitializeLbPair2,
    InitializeCustomizablePermissionlessLbPair,
    InitializeCustomizablePermissionlessLbPair2,
    InitializePermissionLbPair,
    InitializeBinArrayBitmapExtension,
    InitializeBinArray,
    InitializePosition,
    InitializePosition2,
    InitializePositionPda,
    InitializePositionByOperator,
    IncreasePositionLength,
    IncreasePositionLength2,
    AddLiquidity,
    AddLiquidity2,
    AddLiquidityByWeight,
    AddLiquidityByWeight2,
    AddLiquidityByStrategy,
    AddLiquidityByStrategy2,
    AddLiquidityByStrategyOneSide,
    AddLiquidityOneSide,
    AddLiquidityOneSidePrecise,
    AddLiquidityOneSidePrecise2,
    RemoveLiquidity,
    RemoveLiquidity2,
    RemoveLiquidityByRange,
    RemoveLiquidityByRange2,
    RemoveAllLiquidity,
    SetActivationPoint,
    SetPairStatus,
    SetPairStatusPermissionless,
    UpdateBaseFeeParameters,
    UpdateDynamicFeeParameters,
    SwapStateChange,
    RebalanceLiquidity,
    CloseBinArray,
    SetPreActivationDuration,
    SetPreActivationSwapAddress,
    UpdateFeesAndRewards,
    DecreasePositionLength,
    ClosePosition,
    ClaimFeeOrReward,
    RewardStateChange,
    WithdrawProtocolFee,
    ZapProtocolFee,
    PlaceLimitOrder,
    CancelLimitOrder,
}

/// Ordered raw DLMM instruction retained for transaction-level shadow replay.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeteoraDlmmInstructionEvent {
    pub metadata: EventMetadata,
    pub kind: MeteoraDlmmInstructionKind,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MeteoraDlmmSwapMode {
    #[default]
    ExactIn,
    ExactOut,
}

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
    pub remaining_account_indices: Vec<u8>,
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
    #[serde(default)]
    pub max_amount_in: u64,
    #[serde(default)]
    pub amount_out: u64,
    #[serde(default)]
    pub swap_mode: MeteoraDlmmSwapMode,

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
    pub remaining_account_indices: Vec<u8>,
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
    pub raw_account_data: bytes::Bytes,
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
    pub raw_account_data: bytes::Bytes,
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
    pub raw_account_data: bytes::Bytes,
    pub bin_array_bitmap_extension: BinArrayBitmapExtension,
}

/// 事件鉴别器常量
pub mod discriminators {
    // Instruction discriminators
    pub const SWAP_IX: &[u8] = &[0xf8, 0xc6, 0x9e, 0x91, 0xe1, 0x75, 0x87, 0xc8];
    pub const SWAP2_IX: &[u8] = &[0x41, 0x4b, 0x3f, 0x4c, 0xeb, 0x5b, 0x5b, 0x88];
    pub const SWAP_EXACT_OUT2_IX: &[u8] = &[0x2b, 0xd7, 0xf7, 0x84, 0x89, 0x3c, 0xf3, 0x51];
    pub const SWAP_EXACT_OUT_IX: &[u8] = &[250, 73, 101, 33, 38, 207, 75, 184];
    pub const SWAP_WITH_PRICE_IMPACT_IX: &[u8] = &[56, 173, 230, 208, 173, 228, 156, 205];
    pub const SWAP_WITH_PRICE_IMPACT2_IX: &[u8] = &[74, 98, 192, 214, 177, 51, 75, 51];
    pub const INITIALIZE_LB_PAIR_IX: &[u8] = &[45, 154, 237, 210, 221, 15, 166, 92];
    pub const INITIALIZE_LB_PAIR2_IX: &[u8] = &[73, 59, 36, 120, 237, 83, 108, 198];
    pub const INITIALIZE_CUSTOMIZABLE_PERMISSIONLESS_LB_PAIR_IX: &[u8] =
        &[46, 39, 41, 135, 111, 183, 200, 64];
    pub const INITIALIZE_CUSTOMIZABLE_PERMISSIONLESS_LB_PAIR2_IX: &[u8] =
        &[243, 73, 129, 126, 51, 19, 241, 107];
    pub const INITIALIZE_PERMISSION_LB_PAIR_IX: &[u8] = &[108, 102, 213, 85, 251, 3, 53, 21];
    pub const INITIALIZE_BIN_ARRAY_BITMAP_EXTENSION_IX: &[u8] =
        &[47, 157, 226, 180, 12, 240, 33, 71];
    pub const INITIALIZE_BIN_ARRAY_IX: &[u8] = &[35, 86, 19, 185, 78, 212, 75, 211];
    pub const INITIALIZE_POSITION_IX: &[u8] = &[219, 192, 234, 71, 190, 191, 102, 80];
    pub const INITIALIZE_POSITION2_IX: &[u8] = &[143, 19, 242, 145, 213, 15, 104, 115];
    pub const INITIALIZE_POSITION_PDA_IX: &[u8] = &[46, 82, 125, 146, 85, 141, 228, 153];
    pub const INITIALIZE_POSITION_BY_OPERATOR_IX: &[u8] = &[251, 189, 190, 244, 117, 254, 35, 148];
    pub const INCREASE_POSITION_LENGTH_IX: &[u8] = &[80, 83, 117, 211, 66, 13, 33, 149];
    pub const INCREASE_POSITION_LENGTH2_IX: &[u8] = &[255, 210, 204, 71, 115, 137, 225, 113];
    pub const ADD_LIQUIDITY_IX: &[u8] = &[181, 157, 89, 67, 143, 182, 52, 72];
    pub const ADD_LIQUIDITY2_IX: &[u8] = &[228, 162, 78, 28, 70, 219, 116, 115];
    pub const ADD_LIQUIDITY_BY_WEIGHT_IX: &[u8] = &[28, 140, 238, 99, 231, 162, 21, 149];
    pub const ADD_LIQUIDITY_BY_WEIGHT2_IX: &[u8] = &[209, 59, 63, 91, 111, 200, 153, 228];
    pub const ADD_LIQUIDITY_BY_STRATEGY_IX: &[u8] = &[7, 3, 150, 127, 148, 40, 61, 200];
    pub const ADD_LIQUIDITY_BY_STRATEGY2_IX: &[u8] = &[3, 221, 149, 218, 111, 141, 118, 213];
    pub const ADD_LIQUIDITY_BY_STRATEGY_ONE_SIDE_IX: &[u8] = &[41, 5, 238, 175, 100, 225, 6, 205];
    pub const ADD_LIQUIDITY_ONE_SIDE_IX: &[u8] = &[94, 155, 103, 151, 70, 95, 220, 165];
    pub const ADD_LIQUIDITY_ONE_SIDE_PRECISE_IX: &[u8] = &[161, 194, 103, 84, 171, 71, 250, 154];
    pub const ADD_LIQUIDITY_ONE_SIDE_PRECISE2_IX: &[u8] = &[33, 51, 163, 201, 117, 98, 125, 231];
    pub const REMOVE_LIQUIDITY_IX: &[u8] = &[80, 85, 209, 72, 24, 206, 177, 108];
    pub const REMOVE_LIQUIDITY2_IX: &[u8] = &[230, 215, 82, 127, 241, 101, 227, 146];
    pub const REMOVE_LIQUIDITY_BY_RANGE_IX: &[u8] = &[26, 82, 102, 152, 240, 74, 105, 26];
    pub const REMOVE_LIQUIDITY_BY_RANGE2_IX: &[u8] = &[204, 2, 195, 145, 53, 145, 145, 205];
    pub const REMOVE_ALL_LIQUIDITY_IX: &[u8] = &[10, 51, 61, 35, 112, 105, 24, 85];
    pub const SET_ACTIVATION_POINT_IX: &[u8] = &[91, 249, 15, 165, 26, 129, 254, 125];
    pub const SET_PAIR_STATUS_IX: &[u8] = &[67, 248, 231, 137, 154, 149, 217, 174];
    pub const SET_PAIR_STATUS_PERMISSIONLESS_IX: &[u8] = &[78, 59, 152, 211, 70, 183, 46, 208];
    pub const UPDATE_BASE_FEE_PARAMETERS_IX: &[u8] = &[75, 168, 223, 161, 16, 195, 3, 47];
    pub const UPDATE_DYNAMIC_FEE_PARAMETERS_IX: &[u8] = &[92, 161, 46, 246, 255, 189, 22, 22];
    pub const REBALANCE_LIQUIDITY_IX: &[u8] = &[92, 4, 176, 193, 119, 185, 83, 9];
    pub const CLOSE_BIN_ARRAY_IX: &[u8] = &[68, 174, 88, 80, 181, 204, 19, 224];
    pub const SET_PRE_ACTIVATION_DURATION_IX: &[u8] = &[165, 61, 201, 244, 130, 159, 22, 100];
    pub const SET_PRE_ACTIVATION_SWAP_ADDRESS_IX: &[u8] = &[57, 139, 47, 123, 216, 80, 223, 10];
    pub const UPDATE_FEES_AND_REWARDS_IX: &[u8] = &[154, 230, 250, 13, 236, 209, 75, 223];
    pub const UPDATE_FEES_AND_REWARD2_IX: &[u8] = &[32, 142, 184, 154, 103, 65, 184, 88];
    pub const DECREASE_POSITION_LENGTH_IX: &[u8] = &[194, 219, 136, 32, 25, 96, 105, 37];
    pub const CLOSE_POSITION_IX: &[u8] = &[123, 134, 81, 0, 49, 68, 98, 98];
    pub const CLOSE_POSITION2_IX: &[u8] = &[174, 90, 35, 115, 186, 40, 147, 226];
    pub const CLOSE_POSITION_IF_EMPTY_IX: &[u8] = &[59, 124, 212, 118, 91, 152, 110, 157];
    pub const CLAIM_FEE_IX: &[u8] = &[169, 32, 79, 137, 136, 232, 70, 137];
    pub const CLAIM_FEE2_IX: &[u8] = &[112, 191, 101, 171, 28, 144, 127, 187];
    pub const CLAIM_REWARD_IX: &[u8] = &[149, 95, 181, 242, 94, 90, 158, 162];
    pub const CLAIM_REWARD2_IX: &[u8] = &[190, 3, 127, 119, 178, 87, 157, 183];
    pub const INITIALIZE_REWARD_IX: &[u8] = &[95, 135, 192, 196, 242, 129, 230, 68];
    pub const FUND_REWARD_IX: &[u8] = &[188, 50, 249, 165, 93, 151, 38, 63];
    pub const UPDATE_REWARD_DURATION_IX: &[u8] = &[138, 174, 196, 169, 213, 235, 254, 107];
    pub const UPDATE_REWARD_FUNDER_IX: &[u8] = &[211, 28, 48, 32, 215, 160, 35, 23];
    pub const WITHDRAW_INELIGIBLE_REWARD_IX: &[u8] = &[148, 206, 42, 195, 247, 49, 103, 8];
    pub const WITHDRAW_PROTOCOL_FEE_IX: &[u8] = &[158, 201, 158, 189, 33, 93, 162, 103];
    pub const ZAP_PROTOCOL_FEE_IX: &[u8] = &[213, 155, 187, 34, 56, 182, 91, 240];
    pub const PLACE_LIMIT_ORDER_IX: &[u8] = &[108, 176, 33, 186, 146, 229, 1, 197];
    pub const CANCEL_LIMIT_ORDER_IX: &[u8] = &[132, 156, 132, 31, 67, 40, 232, 97];

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
