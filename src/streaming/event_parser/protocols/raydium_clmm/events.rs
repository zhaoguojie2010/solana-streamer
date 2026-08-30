use crate::streaming::event_parser::common::EventMetadata;
use crate::streaming::event_parser::protocols::raydium_clmm::types::AmmConfig;
use crate::streaming::event_parser::protocols::raydium_clmm::types::{
    PoolState, TickArrayBitmapExtension, TickArrayState,
};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

/// Ordered Raydium CLMM instruction retained for transaction-level shadow replay.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RaydiumClmmInstructionKind {
    CreateAmmConfig,
    CreateSupportMintAssociated,
    UpdateAmmConfig,
    CreateDynamicFeeConfig,
    UpdateDynamicFeeConfig,
    CreatePermissionPda,
    ClosePermissionPda,
    CloseSupportMintAssociated,
    CreatePool,
    CreateCustomizablePool,
    CreatePermissionedPool,
    UpdatePoolStatus,
    CreateOperationAccount,
    UpdateOperationAccount,
    TransferRewardOwner,
    InitializeReward,
    CollectRemainingRewards,
    UpdateRewardInfos,
    SetRewardParams,
    CollectProtocolFee,
    CollectFundFee,
    OpenPosition,
    OpenPositionV2,
    OpenPositionWithToken22Nft,
    ClosePosition,
    IncreaseLiquidity,
    IncreaseLiquidityV2,
    DecreaseLiquidity,
    DecreaseLiquidityV2,
    SwapRouterBaseIn,
    CloseProtocolPosition,
    OpenLimitOrder,
    IncreaseLimitOrder,
    DecreaseLimitOrder,
    SettleLimitOrder,
    CloseLimitOrder,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RaydiumClmmInstructionEvent {
    pub metadata: EventMetadata,
    pub kind: RaydiumClmmInstructionKind,
    pub accounts: Vec<Pubkey>,
    /// Anchor instruction payload after the 8-byte discriminator.
    pub data: Vec<u8>,
    /// Ordered Anchor events emitted while this instruction executed. Router instructions can
    /// emit multiple swaps, so this intentionally remains a vector.
    #[serde(default)]
    pub execution_events: Vec<RaydiumClmmExecutionEvent>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RaydiumClmmExecutionEvent {
    Swap {
        pool_state: Pubkey,
        zero_for_one: bool,
        amount_0: u64,
        amount_1: u64,
        sqrt_price_x64: u128,
        liquidity: u128,
        tick: i32,
    },
    LiquidityChange {
        pool_state: Pubkey,
        tick: i32,
        tick_lower: i32,
        tick_upper: i32,
        liquidity_before: u128,
        liquidity_after: u128,
    },
    LimitOrder {
        pool_state: Pubkey,
        limit_order: Pubkey,
        zero_for_one: bool,
        tick: i32,
        total_amount: u64,
        filled_amount: u64,
    },
}

/// 交易
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RaydiumClmmSwapEvent {
    pub metadata: EventMetadata,
    pub amount: u64,
    pub other_amount_threshold: u64,
    pub sqrt_price_limit_x64: u128,
    pub is_base_input: bool,
    // 来自日志事件 SwapEvent 的字段
    pub sender: Pubkey,
    pub token_account_0: Pubkey,
    pub token_account_1: Pubkey,
    pub amount_0: u64,
    pub transfer_fee_0: u64,
    pub amount_1: u64,
    pub transfer_fee_1: u64,
    pub zero_for_one: bool,
    pub sqrt_price_x64: u128,
    pub liquidity: u128,
    pub tick: i32,
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

/// 交易v2
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RaydiumClmmSwapV2Event {
    pub metadata: EventMetadata,
    pub amount: u64,
    pub other_amount_threshold: u64,
    pub sqrt_price_limit_x64: u128,
    pub is_base_input: bool,
    // 来自日志事件 SwapEvent 的字段
    pub sender: Pubkey,
    pub token_account_0: Pubkey,
    pub token_account_1: Pubkey,
    pub amount_0: u64,
    pub transfer_fee_0: u64,
    pub amount_1: u64,
    pub transfer_fee_1: u64,
    pub zero_for_one: bool,
    pub sqrt_price_x64: u128,
    pub liquidity: u128,
    pub tick: i32,
    pub payer: Pubkey,
    pub amm_config: Pubkey,
    pub pool_state: Pubkey,
    pub input_token_account: Pubkey,
    pub output_token_account: Pubkey,
    pub input_vault: Pubkey,
    pub output_vault: Pubkey,
    pub observation_state: Pubkey,
    pub token_program: Pubkey,
    pub token_program2022: Pubkey,
    pub memo_program: Pubkey,
    pub input_vault_mint: Pubkey,
    pub output_vault_mint: Pubkey,
    pub remaining_accounts: Vec<Pubkey>,
}

/// 关闭仓位
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RaydiumClmmClosePositionEvent {
    pub metadata: EventMetadata,
    pub nft_owner: Pubkey,
    pub position_nft_mint: Pubkey,
    pub position_nft_account: Pubkey,
    pub personal_position: Pubkey,
    pub system_program: Pubkey,
    pub token_program: Pubkey,
}

/// 减少流动性v2
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RaydiumClmmDecreaseLiquidityV2Event {
    pub metadata: EventMetadata,
    pub liquidity: u128,
    pub amount0_min: u64,
    pub amount1_min: u64,
    pub nft_owner: Pubkey,
    pub nft_account: Pubkey,
    pub personal_position: Pubkey,
    pub pool_state: Pubkey,
    pub protocol_position: Pubkey,
    pub token_vault0: Pubkey,
    pub token_vault1: Pubkey,
    pub tick_array_lower: Pubkey,
    pub tick_array_upper: Pubkey,
    pub recipient_token_account0: Pubkey,
    pub recipient_token_account1: Pubkey,
    pub token_program: Pubkey,
    pub token_program2022: Pubkey,
    pub memo_program: Pubkey,
    pub vault0_mint: Pubkey,
    pub vault1_mint: Pubkey,
    pub remaining_accounts: Vec<Pubkey>,
}

/// 创建池
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RaydiumClmmCreatePoolEvent {
    pub metadata: EventMetadata,
    pub sqrt_price_x64: u128,
    pub open_time: u64,
    pub pool_creator: Pubkey,
    pub amm_config: Pubkey,
    pub pool_state: Pubkey,
    pub token_mint0: Pubkey,
    pub token_mint1: Pubkey,
    pub token_vault0: Pubkey,
    pub token_vault1: Pubkey,
    pub observation_state: Pubkey,
    pub tick_array_bitmap: Pubkey,
    pub token_program0: Pubkey,
    pub token_program1: Pubkey,
    pub system_program: Pubkey,
    pub rent: Pubkey,
}

/// 增加流动性v2
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RaydiumClmmIncreaseLiquidityV2Event {
    pub metadata: EventMetadata,
    pub liquidity: u128,
    pub amount0_max: u64,
    pub amount1_max: u64,
    pub base_flag: Option<bool>,
    pub nft_owner: Pubkey,
    pub nft_account: Pubkey,
    pub pool_state: Pubkey,
    pub protocol_position: Pubkey,
    pub personal_position: Pubkey,
    pub tick_array_lower: Pubkey,
    pub tick_array_upper: Pubkey,
    pub token_account0: Pubkey,
    pub token_account1: Pubkey,
    pub token_vault0: Pubkey,
    pub token_vault1: Pubkey,
    pub token_program: Pubkey,
    pub token_program2022: Pubkey,
    pub vault0_mint: Pubkey,
    pub vault1_mint: Pubkey,
}

/// 打开仓位v2
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RaydiumClmmOpenPositionWithToken22NftEvent {
    pub metadata: EventMetadata,
    pub tick_lower_index: i32,
    pub tick_upper_index: i32,
    pub tick_array_lower_start_index: i32,
    pub tick_array_upper_start_index: i32,
    pub liquidity: u128,
    pub amount0_max: u64,
    pub amount1_max: u64,
    pub with_metadata: bool,
    pub base_flag: Option<bool>,

    pub payer: Pubkey,
    pub position_nft_owner: Pubkey,
    pub position_nft_mint: Pubkey,
    pub position_nft_account: Pubkey,
    pub pool_state: Pubkey,
    pub protocol_position: Pubkey,
    pub tick_array_lower: Pubkey,
    pub tick_array_upper: Pubkey,
    pub personal_position: Pubkey,
    pub token_account0: Pubkey,
    pub token_account1: Pubkey,
    pub token_vault0: Pubkey,
    pub token_vault1: Pubkey,
    pub rent: Pubkey,
    pub system_program: Pubkey,
    pub token_program: Pubkey,
    pub associated_token_program: Pubkey,
    pub token_program2022: Pubkey,
    pub vault0_mint: Pubkey,
    pub vault1_mint: Pubkey,
}

/// 打开仓位V2
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RaydiumClmmOpenPositionV2Event {
    pub metadata: EventMetadata,
    pub tick_lower_index: i32,
    pub tick_upper_index: i32,
    pub tick_array_lower_start_index: i32,
    pub tick_array_upper_start_index: i32,
    pub liquidity: u128,
    pub amount0_max: u64,
    pub amount1_max: u64,
    pub with_metadata: bool,
    pub base_flag: Option<bool>,

    pub payer: Pubkey,
    pub position_nft_owner: Pubkey,
    pub position_nft_mint: Pubkey,
    pub position_nft_account: Pubkey,
    pub metadata_account: Pubkey,
    pub pool_state: Pubkey,
    pub protocol_position: Pubkey,
    pub tick_array_lower: Pubkey,
    pub tick_array_upper: Pubkey,
    pub personal_position: Pubkey,
    pub token_account0: Pubkey,
    pub token_account1: Pubkey,
    pub token_vault0: Pubkey,
    pub token_vault1: Pubkey,
    pub rent: Pubkey,
    pub system_program: Pubkey,
    pub token_program: Pubkey,
    pub associated_token_program: Pubkey,
    pub metadata_program: Pubkey,
    pub token_program2022: Pubkey,
    pub vault0_mint: Pubkey,
    pub vault1_mint: Pubkey,
    pub remaining_accounts: Vec<Pubkey>,
}

/// 池配置
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RaydiumClmmAmmConfigAccountEvent {
    pub metadata: EventMetadata,
    pub pubkey: Pubkey,
    pub executable: bool,
    pub lamports: u64,
    pub owner: Pubkey,
    pub rent_epoch: u64,
    #[serde(skip)]
    pub raw_account_data: Vec<u8>,
    pub amm_config: AmmConfig,
}

/// 池状态
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RaydiumClmmPoolStateAccountEvent {
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

/// 池状态
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RaydiumClmmTickArrayStateAccountEvent {
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

/// TickArrayBitmapExtension 账户事件
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RaydiumClmmTickArrayBitmapExtensionAccountEvent {
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
    pub const CREATE_AMM_CONFIG: &[u8] = &[137, 52, 237, 212, 215, 117, 108, 104];
    pub const CREATE_SUPPORT_MINT_ASSOCIATED: &[u8] = &[17, 251, 65, 92, 136, 242, 14, 169];
    pub const UPDATE_AMM_CONFIG: &[u8] = &[49, 60, 174, 136, 154, 28, 116, 200];
    pub const CREATE_DYNAMIC_FEE_CONFIG: &[u8] = &[189, 14, 181, 120, 85, 118, 227, 62];
    pub const UPDATE_DYNAMIC_FEE_CONFIG: &[u8] = &[7, 7, 80, 8, 2, 199, 132, 240];
    pub const CREATE_PERMISSION_PDA: &[u8] = &[135, 136, 2, 216, 137, 169, 181, 202];
    pub const CLOSE_PERMISSION_PDA: &[u8] = &[156, 84, 32, 118, 69, 135, 70, 123];
    pub const CLOSE_SUPPORT_MINT_ASSOCIATED: &[u8] = &[96, 136, 183, 99, 72, 152, 54, 131];
    pub const CREATE_CUSTOMIZABLE_POOL: &[u8] = &[43, 68, 212, 167, 89, 47, 164, 1];
    pub const CREATE_PERMISSIONED_POOL: &[u8] = &[36, 243, 179, 35, 42, 239, 53, 229];
    pub const UPDATE_POOL_STATUS: &[u8] = &[130, 87, 108, 6, 46, 224, 117, 123];
    pub const CREATE_OPERATION_ACCOUNT: &[u8] = &[63, 87, 148, 33, 109, 35, 8, 104];
    pub const UPDATE_OPERATION_ACCOUNT: &[u8] = &[127, 70, 119, 40, 188, 227, 61, 7];
    pub const TRANSFER_REWARD_OWNER: &[u8] = &[7, 22, 12, 83, 242, 43, 48, 121];
    pub const INITIALIZE_REWARD: &[u8] = &[95, 135, 192, 196, 242, 129, 230, 68];
    pub const COLLECT_REMAINING_REWARDS: &[u8] = &[18, 237, 166, 197, 34, 16, 213, 144];
    pub const UPDATE_REWARD_INFOS: &[u8] = &[163, 172, 224, 52, 11, 154, 106, 223];
    pub const SET_REWARD_PARAMS: &[u8] = &[112, 52, 167, 75, 32, 201, 211, 137];
    pub const COLLECT_PROTOCOL_FEE: &[u8] = &[136, 136, 252, 221, 194, 66, 126, 89];
    pub const COLLECT_FUND_FEE: &[u8] = &[167, 138, 78, 149, 223, 194, 6, 126];
    pub const OPEN_POSITION: &[u8] = &[135, 128, 47, 77, 15, 152, 240, 49];
    pub const INCREASE_LIQUIDITY: &[u8] = &[46, 156, 243, 118, 13, 205, 251, 178];
    pub const DECREASE_LIQUIDITY: &[u8] = &[160, 38, 208, 111, 104, 91, 44, 1];
    pub const SWAP_ROUTER_BASE_IN: &[u8] = &[69, 125, 115, 218, 245, 186, 242, 196];
    pub const CLOSE_PROTOCOL_POSITION: &[u8] = &[201, 117, 152, 144, 85, 85, 108, 178];
    pub const OPEN_LIMIT_ORDER: &[u8] = &[157, 32, 218, 183, 71, 29, 18, 147];
    pub const INCREASE_LIMIT_ORDER: &[u8] = &[177, 144, 89, 236, 250, 186, 125, 99];
    pub const DECREASE_LIMIT_ORDER: &[u8] = &[117, 157, 60, 103, 66, 49, 163, 0];
    pub const SETTLE_LIMIT_ORDER: &[u8] = &[205, 78, 116, 33, 92, 105, 26, 96];
    pub const CLOSE_LIMIT_ORDER: &[u8] = &[76, 124, 128, 15, 213, 87, 37, 250];

    pub const SWAP: &[u8] = &[248, 198, 158, 145, 225, 117, 135, 200];
    pub const SWAP_V2: &[u8] = &[43, 4, 237, 11, 26, 201, 30, 98];
    pub const CLOSE_POSITION: &[u8] = &[123, 134, 81, 0, 49, 68, 98, 98];
    pub const INCREASE_LIQUIDITY_V2: &[u8] = &[133, 29, 89, 223, 69, 238, 176, 10];
    pub const DECREASE_LIQUIDITY_V2: &[u8] = &[58, 127, 188, 62, 79, 82, 196, 96];
    pub const CREATE_POOL: &[u8] = &[233, 146, 209, 142, 207, 104, 64, 188];
    pub const OPEN_POSITION_WITH_TOKEN_22_NFT: &[u8] = &[77, 255, 174, 82, 125, 29, 201, 46];
    pub const OPEN_POSITION_V2: &[u8] = &[77, 184, 74, 214, 112, 86, 241, 199];

    // 事件鉴别器（Anchor event: SwapEvent）
    pub const SWAP_EVENT: &[u8] = &[64, 198, 205, 232, 38, 8, 113, 226];

    // 账号鉴别器
    pub const AMM_CONFIG: &[u8] = &[218, 244, 33, 104, 203, 203, 43, 111];
    pub const POOL_STATE: &[u8] = &[247, 237, 227, 245, 215, 195, 222, 70];
    pub const TICK_ARRAY_STATE: &[u8] = &[192, 155, 85, 205, 49, 249, 129, 42];
    pub const TICK_ARRAY_BITMAP_EXTENSION: &[u8] = &[60, 150, 36, 219, 97, 128, 139, 153];
}
