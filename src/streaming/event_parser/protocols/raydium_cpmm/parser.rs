use crate::streaming::event_parser::InstructionAccounts;
use solana_sdk::pubkey::Pubkey;

use crate::streaming::event_parser::{
    common::{read_u64_le, read_u8, EventMetadata, EventType, ProgramDataItem},
    protocols::raydium_cpmm::{
        discriminators, RaydiumCpmmDepositEvent, RaydiumCpmmInitializeEvent, RaydiumCpmmSwapEvent,
        RaydiumCpmmWithdrawEvent,
    },
    TxEvent,
};

/// Raydium CPMM程序ID
pub const RAYDIUM_CPMM_PROGRAM_ID: Pubkey =
    solana_sdk::pubkey!("CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C");

/// SwapEvent 从 Anchor 事件日志解析出来的数据
#[derive(Debug, Clone, Default)]
pub struct SwapEventLogData {
    pub pool_id: Pubkey,
    pub input_vault_before: u64,
    pub output_vault_before: u64,
    pub input_amount: u64,
    pub output_amount: u64,
    pub input_transfer_fee: u64,
    pub output_transfer_fee: u64,
    pub base_input: bool,
    pub trade_fee: u64,
    pub creator_fee: u64,
    pub creator_fee_on_input: bool,
}

/// 解析 Raydium CPMM instruction data
///
/// 根据判别器路由到具体的 instruction 解析函数
pub fn parse_raydium_cpmm_instruction_data(
    discriminator: &[u8],
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    metadata: EventMetadata,
) -> Option<TxEvent> {
    match discriminator {
        discriminators::SWAP_BASE_IN => parse_swap_base_input_instruction(data, accounts, metadata),
        discriminators::SWAP_BASE_OUT => {
            parse_swap_base_output_instruction(data, accounts, metadata)
        }
        discriminators::DEPOSIT => parse_deposit_instruction(data, accounts, metadata),
        discriminators::INITIALIZE => parse_initialize_instruction(data, accounts, metadata),
        discriminators::INITIALIZE_WITH_PERMISSION => {
            parse_initialize_with_permission_instruction(data, accounts, metadata)
        }
        discriminators::WITHDRAW => parse_withdraw_instruction(data, accounts, metadata),
        _ => None,
    }
}

pub fn is_raydium_cpmm_swap_instruction(discriminator: &[u8]) -> bool {
    matches!(discriminator, discriminators::SWAP_BASE_IN | discriminators::SWAP_BASE_OUT)
}

/// 解析 Raydium CPMM inner instruction data
///
/// Raydium CPMM 没有 inner instruction 事件
pub fn parse_raydium_cpmm_inner_instruction_data(
    _discriminator: &[u8],
    _data: &[u8],
    _metadata: EventMetadata,
) -> Option<TxEvent> {
    None
}

/// 解析 Raydium CPMM 账户数据
///
/// 根据判别器路由到具体的账户解析函数
pub fn parse_raydium_cpmm_account_data(
    discriminator: &[u8],
    account: crate::streaming::grpc::AccountFrame,
    metadata: crate::streaming::event_parser::common::EventMetadata,
) -> Option<crate::streaming::event_parser::AccountEvent> {
    match discriminator {
        discriminators::AMM_CONFIG => {
            crate::streaming::event_parser::protocols::raydium_cpmm::types::amm_config_parser(
                account, metadata,
            )
        }
        discriminators::POOL_STATE => {
            crate::streaming::event_parser::protocols::raydium_cpmm::types::pool_state_parser(
                account, metadata,
            )
        }
        _ => None,
    }
}

/// 解析提款指令事件
fn parse_withdraw_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::RaydiumCpmmWithdraw;

    if data.len() < 24 || accounts.len() < 14 {
        return None;
    }
    Some(TxEvent::RaydiumCpmmWithdrawEvent(RaydiumCpmmWithdrawEvent {
        liquidity_state: None,
        metadata,
        lp_token_amount: read_u64_le(data, 0)?,
        minimum_token0_amount: read_u64_le(data, 8)?,
        minimum_token1_amount: read_u64_le(data, 16)?,
        owner: *accounts.get(0)?,
        authority: *accounts.get(1)?,
        pool_state: *accounts.get(2)?,
        owner_lp_token: *accounts.get(3)?,
        token0_account: *accounts.get(4)?,
        token1_account: *accounts.get(5)?,
        token0_vault: *accounts.get(6)?,
        token1_vault: *accounts.get(7)?,
        token_program: *accounts.get(8)?,
        token_program2022: *accounts.get(9)?,
        vault0_mint: *accounts.get(10)?,
        vault1_mint: *accounts.get(11)?,
        lp_mint: *accounts.get(12)?,
        memo_program: *accounts.get(13)?,
    }))
}

/// 解析初始化指令事件
fn parse_initialize_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::RaydiumCpmmInitialize;

    if data.len() < 24 || accounts.len() < 20 {
        return None;
    }
    Some(TxEvent::RaydiumCpmmInitializeEvent(RaydiumCpmmInitializeEvent {
        metadata,
        init_amount0: read_u64_le(data, 0)?,
        init_amount1: read_u64_le(data, 8)?,
        open_time: read_u64_le(data, 16)?,
        creator_fee_on: 0,
        enable_creator_fee: false,
        payer: *accounts.get(0)?,
        creator: *accounts.get(0)?,
        amm_config: *accounts.get(1)?,
        authority: *accounts.get(2)?,
        pool_state: *accounts.get(3)?,
        token0_mint: *accounts.get(4)?,
        token1_mint: *accounts.get(5)?,
        lp_mint: *accounts.get(6)?,
        creator_token0: *accounts.get(7)?,
        creator_token1: *accounts.get(8)?,
        creator_lp_token: *accounts.get(9)?,
        token0_vault: *accounts.get(10)?,
        token1_vault: *accounts.get(11)?,
        create_pool_fee: *accounts.get(12)?,
        observation_state: *accounts.get(13)?,
        permission: Pubkey::default(),
        token_program: *accounts.get(14)?,
        token0_program: *accounts.get(15)?,
        token1_program: *accounts.get(16)?,
        associated_token_program: *accounts.get(17)?,
        system_program: *accounts.get(18)?,
        rent: *accounts.get(19)?,
    }))
}

/// Parse permissioned pool creation. Its account layout differs from legacy initialize
/// because payer and pool creator are separate and the permission PDA is explicit.
fn parse_initialize_with_permission_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::RaydiumCpmmInitializeWithPermission;

    if data.len() < 25 || accounts.len() < 21 {
        return None;
    }
    let creator_fee_on = read_u8(data, 24)?;
    if creator_fee_on > 2 {
        return None;
    }
    Some(TxEvent::RaydiumCpmmInitializeEvent(RaydiumCpmmInitializeEvent {
        metadata,
        init_amount0: read_u64_le(data, 0)?,
        init_amount1: read_u64_le(data, 8)?,
        open_time: read_u64_le(data, 16)?,
        creator_fee_on,
        enable_creator_fee: true,
        payer: *accounts.get(0)?,
        creator: *accounts.get(1)?,
        amm_config: *accounts.get(2)?,
        authority: *accounts.get(3)?,
        pool_state: *accounts.get(4)?,
        token0_mint: *accounts.get(5)?,
        token1_mint: *accounts.get(6)?,
        lp_mint: *accounts.get(7)?,
        creator_token0: *accounts.get(8)?,
        creator_token1: *accounts.get(9)?,
        creator_lp_token: *accounts.get(10)?,
        token0_vault: *accounts.get(11)?,
        token1_vault: *accounts.get(12)?,
        create_pool_fee: *accounts.get(13)?,
        observation_state: *accounts.get(14)?,
        permission: *accounts.get(15)?,
        token_program: *accounts.get(16)?,
        token0_program: *accounts.get(17)?,
        token1_program: *accounts.get(18)?,
        associated_token_program: *accounts.get(19)?,
        system_program: *accounts.get(20)?,
        rent: Pubkey::default(),
    }))
}

/// 解析存款指令事件
fn parse_deposit_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::RaydiumCpmmDeposit;

    if data.len() < 24 || accounts.len() < 13 {
        return None;
    }
    Some(TxEvent::RaydiumCpmmDepositEvent(RaydiumCpmmDepositEvent {
        liquidity_state: None,
        metadata,
        lp_token_amount: read_u64_le(data, 0)?,
        maximum_token0_amount: read_u64_le(data, 8)?,
        maximum_token1_amount: read_u64_le(data, 16)?,
        owner: *accounts.get(0)?,
        authority: *accounts.get(1)?,
        pool_state: *accounts.get(2)?,
        owner_lp_token: *accounts.get(3)?,
        token0_account: *accounts.get(4)?,
        token1_account: *accounts.get(5)?,
        token0_vault: *accounts.get(6)?,
        token1_vault: *accounts.get(7)?,
        token_program: *accounts.get(8)?,
        token_program2022: *accounts.get(9)?,
        vault0_mint: *accounts.get(10)?,
        vault1_mint: *accounts.get(11)?,
        lp_mint: *accounts.get(12)?,
    }))
}

/// 解析买入指令事件
fn parse_swap_base_input_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::RaydiumCpmmSwapBaseInput;

    if data.len() < 16 || accounts.len() < 13 {
        return None;
    }

    let amount_in = read_u64_le(data, 0)?;
    let minimum_amount_out = read_u64_le(data, 8)?;

    Some(TxEvent::RaydiumCpmmSwapEvent(RaydiumCpmmSwapEvent {
        metadata,
        amount_in,
        minimum_amount_out,
        payer: *accounts.get(0)?,
        authority: *accounts.get(1)?,
        amm_config: *accounts.get(2)?,
        pool_state: *accounts.get(3)?,
        input_token_account: *accounts.get(4)?,
        output_token_account: *accounts.get(5)?,
        input_vault: *accounts.get(6)?,
        output_vault: *accounts.get(7)?,
        input_token_program: *accounts.get(8)?,
        output_token_program: *accounts.get(9)?,
        input_token_mint: *accounts.get(10)?,
        output_token_mint: *accounts.get(11)?,
        observation_state: *accounts.get(12)?,
        ..Default::default()
    }))
}

fn parse_swap_base_output_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::RaydiumCpmmSwapBaseOutput;

    if data.len() < 16 || accounts.len() < 13 {
        return None;
    }

    let max_amount_in = read_u64_le(data, 0)?;
    let amount_out = read_u64_le(data, 8)?;

    Some(TxEvent::RaydiumCpmmSwapEvent(RaydiumCpmmSwapEvent {
        metadata,
        max_amount_in,
        amount_out,
        payer: *accounts.get(0)?,
        authority: *accounts.get(1)?,
        amm_config: *accounts.get(2)?,
        pool_state: *accounts.get(3)?,
        input_token_account: *accounts.get(4)?,
        output_token_account: *accounts.get(5)?,
        input_vault: *accounts.get(6)?,
        output_vault: *accounts.get(7)?,
        input_token_program: *accounts.get(8)?,
        output_token_program: *accounts.get(9)?,
        input_token_mint: *accounts.get(10)?,
        output_token_mint: *accounts.get(11)?,
        observation_state: *accounts.get(12)?,
        ..Default::default()
    }))
}

/// 从 Anchor 事件日志中解析 SwapEvent 数据
///
/// Anchor 事件日志格式: "Program data: <base64_encoded_event>"
/// 事件数据格式: [8字节鉴别器] [事件数据]
pub fn parse_swap_event_from_bytes(decoded: &[u8]) -> Option<SwapEventLogData> {
    // 解码 base64

    // 检查长度和鉴别器
    if decoded.len() < 8 {
        return None;
    }

    // 验证鉴别器
    if &decoded[0..8] != discriminators::SWAP_EVENT {
        return None;
    }

    // 解析事件数据
    // SwapEvent 结构（从 raydium-cp-swap 源码）:
    // - pool_id: Pubkey (32 bytes)
    // - input_vault_before: u64 (8 bytes)
    // - output_vault_before: u64 (8 bytes)
    // - input_amount: u64 (8 bytes)
    // - output_amount: u64 (8 bytes)
    // - input_transfer_fee: u64 (8 bytes)
    // - output_transfer_fee: u64 (8 bytes)
    // - base_input: bool (1 byte)
    // - input_mint: Pubkey (32 bytes)
    // - output_mint: Pubkey (32 bytes)
    // - trade_fee: u64 (8 bytes)
    // - creator_fee: u64 (8 bytes)
    // - creator_fee_on_input: bool (1 byte)

    let pool_id = Pubkey::new_from_array(decoded.get(8..40)?.try_into().ok()?);
    let mut offset = 8 + 32; // 跳过鉴别器和 pool_id

    let input_vault_before = read_u64_le(&decoded, offset)?;
    offset += 8;

    let output_vault_before = read_u64_le(&decoded, offset)?;
    offset += 8;

    let input_amount = read_u64_le(&decoded, offset)?;
    offset += 8;

    let output_amount = read_u64_le(&decoded, offset)?;
    offset += 8;

    let input_transfer_fee = read_u64_le(&decoded, offset)?;
    offset += 8;

    let output_transfer_fee = read_u64_le(&decoded, offset)?;
    offset += 8;

    let base_input = read_u8(&decoded, offset)? != 0;
    offset += 1;

    offset += 32; // 跳过 input_mint
    offset += 32; // 跳过 output_mint

    let trade_fee = read_u64_le(&decoded, offset)?;
    offset += 8;

    let creator_fee = read_u64_le(&decoded, offset)?;
    offset += 8;

    let creator_fee_on_input = read_u8(&decoded, offset)? != 0;

    Some(SwapEventLogData {
        pool_id,
        input_vault_before,
        output_vault_before,
        input_amount,
        output_amount,
        input_transfer_fee,
        output_transfer_fee,
        base_input,
        trade_fee,
        creator_fee,
        creator_fee_on_input,
    })
}

/// 从 ProgramDataItem 解析 SwapEvent 数据
pub fn parse_swap_event_from_program_data(
    item: &ProgramDataItem,
    expected_pool_id: &Pubkey,
) -> Option<SwapEventLogData> {
    if item.program_id != RAYDIUM_CPMM_PROGRAM_ID {
        return None;
    }
    let event_data = parse_swap_event_from_bytes(item.data)?;
    if &event_data.pool_id != expected_pool_id {
        return None;
    }
    Some(event_data)
}

/// Decode only the complete, program-authenticated LpChangeEvent layout.
pub fn parse_liquidity_state_from_program_data(
    item: &ProgramDataItem,
    pool: Pubkey,
    change_type: u8,
) -> Option<super::events::RaydiumCpmmLiquidityState> {
    if item.program_id != RAYDIUM_CPMM_PROGRAM_ID {
        return None;
    }
    let bytes = item.data;
    if bytes.len() != 97
        || bytes.get(..8)? != [121, 163, 205, 201, 57, 218, 117, 60]
        || Pubkey::new_from_array(bytes.get(8..40)?.try_into().ok()?) != pool
        || *bytes.get(96)? != change_type
    {
        return None;
    }
    Some(super::events::RaydiumCpmmLiquidityState {
        lp_amount_before: read_u64_le(&bytes, 40)?,
        token_0_vault_before: read_u64_le(&bytes, 48)?,
        token_1_vault_before: read_u64_le(&bytes, 56)?,
        token_0_amount: read_u64_le(&bytes, 64)?,
        token_1_amount: read_u64_le(&bytes, 72)?,
        token_0_transfer_fee: read_u64_le(&bytes, 80)?,
        token_1_transfer_fee: read_u64_le(&bytes, 88)?,
        change_type,
    })
}

/// Classify before allocating or decoding a protocol instruction.
pub(crate) fn instruction_event_type(discriminator: &[u8]) -> Option<EventType> {
    match discriminator {
        discriminators::SWAP_BASE_IN => Some(EventType::RaydiumCpmmSwapBaseInput),
        discriminators::SWAP_BASE_OUT => Some(EventType::RaydiumCpmmSwapBaseOutput),
        discriminators::DEPOSIT => Some(EventType::RaydiumCpmmDeposit),
        discriminators::INITIALIZE => Some(EventType::RaydiumCpmmInitialize),
        discriminators::INITIALIZE_WITH_PERMISSION => {
            Some(EventType::RaydiumCpmmInitializeWithPermission)
        }
        discriminators::WITHDRAW => Some(EventType::RaydiumCpmmWithdraw),
        _ => None,
    }
}

/// Classify before allocating or decoding a protocol account.
pub(crate) fn account_event_type(discriminator: &[u8]) -> Option<EventType> {
    match discriminator {
        discriminators::AMM_CONFIG => Some(EventType::AccountRaydiumCpmmAmmConfig),
        discriminators::POOL_STATE => Some(EventType::AccountRaydiumCpmmPoolState),
        _ => None,
    }
}
