use crate::streaming::event_parser::InstructionAccounts;
use crate::streaming::event_parser::{
    common::{
        read_i32_le, read_option_bool, read_u128_le, read_u64_le, read_u8_le, EventMetadata,
        EventType, ProgramDataItem,
    },
    protocols::raydium_clmm::{
        discriminators, RaydiumClmmClosePositionEvent, RaydiumClmmCreatePoolEvent,
        RaydiumClmmDecreaseLiquidityV2Event, RaydiumClmmExecutionEvent,
        RaydiumClmmIncreaseLiquidityV2Event, RaydiumClmmInstructionEvent,
        RaydiumClmmInstructionKind, RaydiumClmmOpenPositionV2Event,
        RaydiumClmmOpenPositionWithToken22NftEvent, RaydiumClmmSwapEvent, RaydiumClmmSwapV2Event,
    },
    TxEvent,
};
use solana_sdk::pubkey::Pubkey;

/// Raydium CLMM程序ID
pub const RAYDIUM_CLMM_PROGRAM_ID: Pubkey =
    solana_sdk::pubkey!("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK");

/// SwapEvent 从 Anchor 事件日志解析出来的数据
#[derive(Debug, Clone, Default)]
pub struct SwapEventLogData {
    pub pool_state: Pubkey,
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
}

/// 解析 Raydium CLMM instruction data
///
/// 根据判别器路由到具体的 instruction 解析函数
pub fn parse_raydium_clmm_instruction_data(
    discriminator: &[u8],
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    metadata: EventMetadata,
) -> Option<TxEvent> {
    match discriminator {
        discriminators::SWAP => parse_swap_instruction(data, accounts, metadata),
        discriminators::SWAP_V2 => parse_swap_v2_instruction(data, accounts, metadata),
        _ => parse_modeled_instruction(discriminator, data, accounts, metadata),
    }
}

fn parse_modeled_instruction(
    discriminator: &[u8],
    _data: &[u8],
    _accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    use RaydiumClmmInstructionKind as Kind;
    let kind = match discriminator {
        discriminators::CREATE_AMM_CONFIG => Kind::CreateAmmConfig,
        discriminators::CREATE_SUPPORT_MINT_ASSOCIATED => Kind::CreateSupportMintAssociated,
        discriminators::UPDATE_AMM_CONFIG => Kind::UpdateAmmConfig,
        discriminators::CREATE_DYNAMIC_FEE_CONFIG => Kind::CreateDynamicFeeConfig,
        discriminators::UPDATE_DYNAMIC_FEE_CONFIG => Kind::UpdateDynamicFeeConfig,
        discriminators::CREATE_PERMISSION_PDA => Kind::CreatePermissionPda,
        discriminators::CLOSE_PERMISSION_PDA => Kind::ClosePermissionPda,
        discriminators::CLOSE_SUPPORT_MINT_ASSOCIATED => Kind::CloseSupportMintAssociated,
        discriminators::CREATE_POOL => Kind::CreatePool,
        discriminators::CREATE_CUSTOMIZABLE_POOL => Kind::CreateCustomizablePool,
        discriminators::CREATE_PERMISSIONED_POOL => Kind::CreatePermissionedPool,
        discriminators::UPDATE_POOL_STATUS => Kind::UpdatePoolStatus,
        discriminators::CREATE_OPERATION_ACCOUNT => Kind::CreateOperationAccount,
        discriminators::UPDATE_OPERATION_ACCOUNT => Kind::UpdateOperationAccount,
        discriminators::TRANSFER_REWARD_OWNER => Kind::TransferRewardOwner,
        discriminators::INITIALIZE_REWARD => Kind::InitializeReward,
        discriminators::COLLECT_REMAINING_REWARDS => Kind::CollectRemainingRewards,
        discriminators::UPDATE_REWARD_INFOS => Kind::UpdateRewardInfos,
        discriminators::SET_REWARD_PARAMS => Kind::SetRewardParams,
        discriminators::COLLECT_PROTOCOL_FEE => Kind::CollectProtocolFee,
        discriminators::COLLECT_FUND_FEE => Kind::CollectFundFee,
        discriminators::OPEN_POSITION => Kind::OpenPosition,
        discriminators::OPEN_POSITION_V2 => Kind::OpenPositionV2,
        discriminators::OPEN_POSITION_WITH_TOKEN_22_NFT => Kind::OpenPositionWithToken22Nft,
        discriminators::CLOSE_POSITION => Kind::ClosePosition,
        discriminators::INCREASE_LIQUIDITY => Kind::IncreaseLiquidity,
        discriminators::INCREASE_LIQUIDITY_V2 => Kind::IncreaseLiquidityV2,
        discriminators::DECREASE_LIQUIDITY => Kind::DecreaseLiquidity,
        discriminators::DECREASE_LIQUIDITY_V2 => Kind::DecreaseLiquidityV2,
        discriminators::SWAP_ROUTER_BASE_IN => Kind::SwapRouterBaseIn,
        discriminators::CLOSE_PROTOCOL_POSITION => Kind::CloseProtocolPosition,
        discriminators::OPEN_LIMIT_ORDER => Kind::OpenLimitOrder,
        discriminators::INCREASE_LIMIT_ORDER => Kind::IncreaseLimitOrder,
        discriminators::DECREASE_LIMIT_ORDER => Kind::DecreaseLimitOrder,
        discriminators::SETTLE_LIMIT_ORDER => Kind::SettleLimitOrder,
        discriminators::CLOSE_LIMIT_ORDER => Kind::CloseLimitOrder,
        _ => return None,
    };
    metadata.event_type = EventType::RaydiumClmmInstruction;
    Some(TxEvent::RaydiumClmmInstructionEvent(RaydiumClmmInstructionEvent {
        metadata,
        kind,
        execution_events: Vec::new(),
    }))
}

pub fn is_raydium_clmm_swap_instruction(discriminator: &[u8]) -> bool {
    matches!(discriminator, discriminators::SWAP | discriminators::SWAP_V2)
}

/// 解析 Raydium CLMM inner instruction data
///
/// Raydium CLMM 没有 inner instruction 事件
pub fn parse_raydium_clmm_inner_instruction_data(
    _discriminator: &[u8],
    _data: &[u8],
    _metadata: EventMetadata,
) -> Option<TxEvent> {
    None
}

/// 解析 Raydium CLMM 账户数据
///
/// 根据判别器路由到具体的账户解析函数
pub fn parse_raydium_clmm_account_data(
    discriminator: &[u8],
    account: crate::streaming::grpc::AccountFrame,
    metadata: crate::streaming::event_parser::common::EventMetadata,
) -> Option<crate::streaming::event_parser::AccountEvent> {
    match discriminator {
        discriminators::AMM_CONFIG => {
            crate::streaming::event_parser::protocols::raydium_clmm::types::amm_config_parser(account, metadata)
        }
        discriminators::POOL_STATE => {
            crate::streaming::event_parser::protocols::raydium_clmm::types::pool_state_parser(account, metadata)
        }
        discriminators::TICK_ARRAY_STATE => {
            crate::streaming::event_parser::protocols::raydium_clmm::types::tick_array_state_parser(account, metadata)
        }
        discriminators::TICK_ARRAY_BITMAP_EXTENSION => {
            crate::streaming::event_parser::protocols::raydium_clmm::types::tick_array_bitmap_extension_parser(account, metadata)
        }
        _ => None,
    }
}

/// 解析打开仓位V2指令事件
#[allow(dead_code)] // Kept for downstream compatibility while unified instruction events are emitted.
fn parse_open_position_v2_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::RaydiumClmmOpenPositionV2;

    if data.len() < 51 || accounts.len() < 22 {
        return None;
    }
    Some(TxEvent::RaydiumClmmOpenPositionV2Event(RaydiumClmmOpenPositionV2Event {
        metadata,
        tick_lower_index: read_i32_le(data, 0)?,
        tick_upper_index: read_i32_le(data, 4)?,
        tick_array_lower_start_index: read_i32_le(data, 8)?,
        tick_array_upper_start_index: read_i32_le(data, 12)?,
        liquidity: read_u128_le(data, 16)?,
        amount0_max: read_u64_le(data, 32)?,
        amount1_max: read_u64_le(data, 40)?,
        with_metadata: read_u8_le(data, 48)? == 1,
        base_flag: read_option_bool(data, &mut 49)?,
        payer: *accounts.get(0)?,
        position_nft_owner: *accounts.get(1)?,
        position_nft_mint: *accounts.get(2)?,
        position_nft_account: *accounts.get(3)?,
        metadata_account: *accounts.get(4)?,
        pool_state: *accounts.get(5)?,
        protocol_position: *accounts.get(6)?,
        tick_array_lower: *accounts.get(7)?,
        tick_array_upper: *accounts.get(8)?,
        personal_position: *accounts.get(9)?,
        token_account0: *accounts.get(10)?,
        token_account1: *accounts.get(11)?,
        token_vault0: *accounts.get(12)?,
        token_vault1: *accounts.get(13)?,
        rent: *accounts.get(14)?,
        system_program: *accounts.get(15)?,
        token_program: *accounts.get(16)?,
        associated_token_program: *accounts.get(17)?,
        metadata_program: *accounts.get(18)?,
        token_program2022: *accounts.get(19)?,
        vault0_mint: *accounts.get(20)?,
        vault1_mint: *accounts.get(21)?,
        remaining_account_indices: accounts.indices_from(22).collect(),
    }))
}

/// 解析打开仓位v2指令事件
#[allow(dead_code)]
fn parse_open_position_with_token_22_nft_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::RaydiumClmmOpenPositionWithToken22Nft;

    if data.len() < 51 || accounts.len() < 20 {
        return None;
    }
    Some(TxEvent::RaydiumClmmOpenPositionWithToken22NftEvent(
        RaydiumClmmOpenPositionWithToken22NftEvent {
            metadata,
            tick_lower_index: read_i32_le(data, 0)?,
            tick_upper_index: read_i32_le(data, 4)?,
            tick_array_lower_start_index: read_i32_le(data, 8)?,
            tick_array_upper_start_index: read_i32_le(data, 12)?,
            liquidity: read_u128_le(data, 16)?,
            amount0_max: read_u64_le(data, 32)?,
            amount1_max: read_u64_le(data, 40)?,
            with_metadata: read_u8_le(data, 48)? == 1,
            base_flag: read_option_bool(data, &mut 49)?,
            payer: *accounts.get(0)?,
            position_nft_owner: *accounts.get(1)?,
            position_nft_mint: *accounts.get(2)?,
            position_nft_account: *accounts.get(3)?,
            pool_state: *accounts.get(4)?,
            protocol_position: *accounts.get(5)?,
            tick_array_lower: *accounts.get(6)?,
            tick_array_upper: *accounts.get(7)?,
            personal_position: *accounts.get(8)?,
            token_account0: *accounts.get(9)?,
            token_account1: *accounts.get(10)?,
            token_vault0: *accounts.get(11)?,
            token_vault1: *accounts.get(12)?,
            rent: *accounts.get(13)?,
            system_program: *accounts.get(14)?,
            token_program: *accounts.get(15)?,
            associated_token_program: *accounts.get(16)?,
            token_program2022: *accounts.get(17)?,
            vault0_mint: *accounts.get(18)?,
            vault1_mint: *accounts.get(19)?,
        },
    ))
}

/// 解析增加流动性v2指令事件
#[allow(dead_code)]
fn parse_increase_liquidity_v2_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::RaydiumClmmIncreaseLiquidityV2;

    if data.len() < 34 || accounts.len() < 15 {
        return None;
    }
    Some(TxEvent::RaydiumClmmIncreaseLiquidityV2Event(RaydiumClmmIncreaseLiquidityV2Event {
        metadata,
        liquidity: read_u128_le(data, 0)?,
        amount0_max: read_u64_le(data, 16)?,
        amount1_max: read_u64_le(data, 24)?,
        base_flag: read_option_bool(data, &mut 32)?,
        nft_owner: *accounts.get(0)?,
        nft_account: *accounts.get(1)?,
        pool_state: *accounts.get(2)?,
        protocol_position: *accounts.get(3)?,
        personal_position: *accounts.get(4)?,
        tick_array_lower: *accounts.get(5)?,
        tick_array_upper: *accounts.get(6)?,
        token_account0: *accounts.get(7)?,
        token_account1: *accounts.get(8)?,
        token_vault0: *accounts.get(9)?,
        token_vault1: *accounts.get(10)?,
        token_program: *accounts.get(11)?,
        token_program2022: *accounts.get(12)?,
        vault0_mint: *accounts.get(13)?,
        vault1_mint: *accounts.get(14)?,
    }))
}

/// 解析创建池指令事件
#[allow(dead_code)]
fn parse_create_pool_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::RaydiumClmmCreatePool;

    if data.len() < 24 || accounts.len() < 13 {
        return None;
    }
    Some(TxEvent::RaydiumClmmCreatePoolEvent(RaydiumClmmCreatePoolEvent {
        metadata,
        sqrt_price_x64: read_u128_le(data, 0)?,
        open_time: read_u64_le(data, 16)?,
        pool_creator: *accounts.get(0)?,
        amm_config: *accounts.get(1)?,
        pool_state: *accounts.get(2)?,
        token_mint0: *accounts.get(3)?,
        token_mint1: *accounts.get(4)?,
        token_vault0: *accounts.get(5)?,
        token_vault1: *accounts.get(6)?,
        observation_state: *accounts.get(7)?,
        tick_array_bitmap: *accounts.get(8)?,
        token_program0: *accounts.get(9)?,
        token_program1: *accounts.get(10)?,
        system_program: *accounts.get(11)?,
        rent: *accounts.get(12)?,
    }))
}

/// 解析减少流动性v2指令事件
#[allow(dead_code)]
fn parse_decrease_liquidity_v2_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::RaydiumClmmDecreaseLiquidityV2;

    if data.len() < 32 || accounts.len() < 16 {
        return None;
    }
    Some(TxEvent::RaydiumClmmDecreaseLiquidityV2Event(RaydiumClmmDecreaseLiquidityV2Event {
        metadata,
        liquidity: read_u128_le(data, 0)?,
        amount0_min: read_u64_le(data, 16)?,
        amount1_min: read_u64_le(data, 24)?,
        nft_owner: *accounts.get(0)?,
        nft_account: *accounts.get(1)?,
        personal_position: *accounts.get(2)?,
        pool_state: *accounts.get(3)?,
        protocol_position: *accounts.get(4)?,
        token_vault0: *accounts.get(5)?,
        token_vault1: *accounts.get(6)?,
        tick_array_lower: *accounts.get(7)?,
        tick_array_upper: *accounts.get(8)?,
        recipient_token_account0: *accounts.get(9)?,
        recipient_token_account1: *accounts.get(10)?,
        token_program: *accounts.get(11)?,
        token_program2022: *accounts.get(12)?,
        memo_program: *accounts.get(13)?,
        vault0_mint: *accounts.get(14)?,
        vault1_mint: *accounts.get(15)?,
        remaining_account_indices: accounts.indices_from(16).collect(),
    }))
}

/// 解析关闭仓位指令事件
#[allow(dead_code)]
fn parse_close_position_instruction(
    _data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::RaydiumClmmClosePosition;

    if accounts.len() < 6 {
        return None;
    }
    Some(TxEvent::RaydiumClmmClosePositionEvent(RaydiumClmmClosePositionEvent {
        metadata,
        nft_owner: *accounts.get(0)?,
        position_nft_mint: *accounts.get(1)?,
        position_nft_account: *accounts.get(2)?,
        personal_position: *accounts.get(3)?,
        system_program: *accounts.get(4)?,
        token_program: *accounts.get(5)?,
    }))
}

/// 解析交易指令事件
fn parse_swap_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::RaydiumClmmSwap;

    if data.len() < 33 || accounts.len() < 10 {
        return None;
    }

    let amount = read_u64_le(data, 0)?;
    let other_amount_threshold = read_u64_le(data, 8)?;
    let sqrt_price_limit_x64 = read_u128_le(data, 16)?;
    let is_base_input = read_u8_le(data, 32)?;

    Some(TxEvent::RaydiumClmmSwapEvent(RaydiumClmmSwapEvent {
        metadata,
        amount,
        other_amount_threshold,
        sqrt_price_limit_x64,
        is_base_input: is_base_input == 1,
        payer: *accounts.get(0)?,
        amm_config: *accounts.get(1)?,
        pool_state: *accounts.get(2)?,
        input_token_account: *accounts.get(3)?,
        output_token_account: *accounts.get(4)?,
        input_vault: *accounts.get(5)?,
        output_vault: *accounts.get(6)?,
        observation_state: *accounts.get(7)?,
        token_program: *accounts.get(8)?,
        tick_array: *accounts.get(9)?,
        remaining_account_indices: accounts.indices_from(10).collect(),
        ..Default::default()
    }))
}

fn parse_swap_v2_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::RaydiumClmmSwapV2;

    if data.len() < 33 || accounts.len() < 13 {
        return None;
    }

    let amount = read_u64_le(data, 0)?;
    let other_amount_threshold = read_u64_le(data, 8)?;
    let sqrt_price_limit_x64 = read_u128_le(data, 16)?;
    let is_base_input = read_u8_le(data, 32)?;

    Some(TxEvent::RaydiumClmmSwapV2Event(RaydiumClmmSwapV2Event {
        metadata,
        amount,
        other_amount_threshold,
        sqrt_price_limit_x64,
        is_base_input: is_base_input == 1,
        payer: *accounts.get(0)?,
        amm_config: *accounts.get(1)?,
        pool_state: *accounts.get(2)?,
        input_token_account: *accounts.get(3)?,
        output_token_account: *accounts.get(4)?,
        input_vault: *accounts.get(5)?,
        output_vault: *accounts.get(6)?,
        observation_state: *accounts.get(7)?,
        token_program: *accounts.get(8)?,
        token_program2022: *accounts.get(9)?,
        memo_program: *accounts.get(10)?,
        input_vault_mint: *accounts.get(11)?,
        output_vault_mint: *accounts.get(12)?,
        remaining_account_indices: accounts.indices_from(13).collect(),
        ..Default::default()
    }))
}

/// 从 Anchor 事件日志中解析 SwapEvent 数据
///
/// Anchor 事件日志格式: "Program data: <base64_encoded_event>"
/// 事件数据格式: [8字节鉴别器] [事件数据]
pub fn parse_swap_event_from_bytes(decoded: &[u8]) -> Option<SwapEventLogData> {
    if decoded.len() < 8 {
        return None;
    }
    if &decoded[0..8] != discriminators::SWAP_EVENT {
        return None;
    }

    let mut offset = 8;
    let pool_state = Pubkey::new_from_array(decoded.get(offset..offset + 32)?.try_into().ok()?);
    offset += 32;
    let sender = Pubkey::new_from_array(decoded.get(offset..offset + 32)?.try_into().ok()?);
    offset += 32;
    let token_account_0 =
        Pubkey::new_from_array(decoded.get(offset..offset + 32)?.try_into().ok()?);
    offset += 32;
    let token_account_1 =
        Pubkey::new_from_array(decoded.get(offset..offset + 32)?.try_into().ok()?);
    offset += 32;

    let amount_0 = read_u64_le(&decoded, offset)?;
    offset += 8;
    let transfer_fee_0 = read_u64_le(&decoded, offset)?;
    offset += 8;
    let amount_1 = read_u64_le(&decoded, offset)?;
    offset += 8;
    let transfer_fee_1 = read_u64_le(&decoded, offset)?;
    offset += 8;
    let zero_for_one = read_u8_le(&decoded, offset)? != 0;
    offset += 1;
    let sqrt_price_x64 = read_u128_le(&decoded, offset)?;
    offset += 16;
    let liquidity = read_u128_le(&decoded, offset)?;
    offset += 16;
    let tick = read_i32_le(&decoded, offset)?;

    Some(SwapEventLogData {
        pool_state,
        sender,
        token_account_0,
        token_account_1,
        amount_0,
        transfer_fee_0,
        amount_1,
        transfer_fee_1,
        zero_for_one,
        sqrt_price_x64,
        liquidity,
        tick,
    })
}

/// Decode state-changing Anchor events used by the pending CLMM shadow. Unknown events are
/// ignored; malformed known events fail parsing instead of producing a partial state delta.
pub fn parse_execution_event_from_program_data(
    item: &ProgramDataItem,
) -> Option<RaydiumClmmExecutionEvent> {
    if item.program_id != RAYDIUM_CLMM_PROGRAM_ID {
        return None;
    }
    let bytes = item.data;
    let discriminator = bytes.get(..8)?;
    let data = bytes.get(8..)?;
    let pubkey = |offset: usize| Pubkey::try_from(data.get(offset..offset + 32)?).ok();
    let u64_at = |offset: usize| read_u64_le(data, offset);
    let u128_at = |offset: usize| read_u128_le(data, offset);
    let i32_at = |offset: usize| read_i32_le(data, offset);

    if discriminator == [126, 240, 175, 206, 158, 88, 153, 107] {
        return Some(RaydiumClmmExecutionEvent::LiquidityChange {
            pool_state: pubkey(0)?,
            tick: i32_at(32)?,
            tick_lower: i32_at(36)?,
            tick_upper: i32_at(40)?,
            liquidity_before: u128_at(44)?,
            liquidity_after: u128_at(60)?,
        });
    }
    if discriminator == discriminators::SWAP_EVENT {
        let event = parse_swap_event_from_bytes(item.data)?;
        return Some(RaydiumClmmExecutionEvent::Swap {
            pool_state: event.pool_state,
            zero_for_one: event.zero_for_one,
            amount_0: event.amount_0,
            amount_1: event.amount_1,
            sqrt_price_x64: event.sqrt_price_x64,
            liquidity: event.liquidity,
            tick: event.tick,
        });
    }
    let (kind, filled_offset) = match discriminator {
        [106, 24, 71, 85, 57, 169, 158, 216] => (0u8, None),
        [11, 120, 13, 204, 199, 87, 19, 200] => (1, None),
        [88, 119, 77, 164, 125, 124, 10, 194] => (2, Some(77)),
        [70, 48, 40, 221, 219, 237, 212, 163] => (3, Some(77)),
        _ => return None,
    };
    let _ = kind;
    Some(RaydiumClmmExecutionEvent::LimitOrder {
        pool_state: pubkey(0)?,
        limit_order: pubkey(32)?,
        zero_for_one: *data.get(64)? != 0,
        tick: i32_at(65)?,
        total_amount: u64_at(69)?,
        filled_amount: filled_offset.and_then(|offset| u64_at(offset)).unwrap_or(0),
    })
}

/// 从 ProgramDataItem 解析 SwapEvent 数据
pub fn parse_swap_event_from_program_data(
    item: &ProgramDataItem,
    expected_pool_state: &Pubkey,
) -> Option<SwapEventLogData> {
    if item.program_id != RAYDIUM_CLMM_PROGRAM_ID {
        return None;
    }
    let event_data = parse_swap_event_from_bytes(item.data)?;
    if &event_data.pool_state != expected_pool_state {
        return None;
    }
    Some(event_data)
}

/// Classify before allocating or decoding a protocol instruction.
pub(crate) fn instruction_event_type(discriminator: &[u8]) -> Option<EventType> {
    match discriminator {
        discriminators::SWAP => Some(EventType::RaydiumClmmSwap),
        discriminators::SWAP_V2 => Some(EventType::RaydiumClmmSwapV2),
        _ => Some(EventType::RaydiumClmmInstruction),
    }
}

/// Classify before allocating or decoding a protocol account.
pub(crate) fn account_event_type(discriminator: &[u8]) -> Option<EventType> {
    match discriminator {
        discriminators::AMM_CONFIG => Some(EventType::AccountRaydiumClmmAmmConfig),
        discriminators::POOL_STATE => Some(EventType::AccountRaydiumClmmPoolState),
        discriminators::TICK_ARRAY_STATE => Some(EventType::AccountRaydiumClmmTickArrayState),
        discriminators::TICK_ARRAY_BITMAP_EXTENSION => {
            Some(EventType::AccountRaydiumClmmTickArrayBitmapExtension)
        }
        _ => None,
    }
}
