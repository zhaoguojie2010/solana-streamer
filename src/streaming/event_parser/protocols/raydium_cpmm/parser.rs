use solana_sdk::pubkey::Pubkey;

use crate::streaming::event_parser::{
    common::{read_u64_le, read_u8, EventMetadata, EventType},
    protocols::raydium_cpmm::{
        discriminators, RaydiumCpmmDepositEvent, RaydiumCpmmInitializeEvent, RaydiumCpmmSwapEvent,
        RaydiumCpmmWithdrawEvent,
    },
    DexEvent,
};

/// Raydium CPMM程序ID
pub const RAYDIUM_CPMM_PROGRAM_ID: Pubkey =
    solana_sdk::pubkey!("CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C");

/// SwapEvent 从 Anchor 事件日志解析出来的数据
#[derive(Debug, Clone, Default)]
pub struct SwapEventLogData {
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
    accounts: &[Pubkey],
    metadata: EventMetadata,
) -> Option<DexEvent> {
    match discriminator {
        discriminators::SWAP_BASE_IN => {
            parse_swap_base_input_instruction(data, accounts, metadata)
        }
        discriminators::SWAP_BASE_OUT => {
            parse_swap_base_output_instruction(data, accounts, metadata)
        }
        discriminators::DEPOSIT => parse_deposit_instruction(data, accounts, metadata),
        discriminators::INITIALIZE => parse_initialize_instruction(data, accounts, metadata),
        discriminators::WITHDRAW => parse_withdraw_instruction(data, accounts, metadata),
        _ => None,
    }
}

/// 解析 Raydium CPMM inner instruction data
///
/// Raydium CPMM 没有 inner instruction 事件
pub fn parse_raydium_cpmm_inner_instruction_data(
    _discriminator: &[u8],
    _data: &[u8],
    _metadata: EventMetadata,
) -> Option<DexEvent> {
    None
}


/// 解析 Raydium CPMM 账户数据
///
/// 根据判别器路由到具体的账户解析函数
pub fn parse_raydium_cpmm_account_data(
    discriminator: &[u8],
    account: &crate::streaming::grpc::AccountPretty,
    metadata: crate::streaming::event_parser::common::EventMetadata,
) -> Option<crate::streaming::event_parser::DexEvent> {
    match discriminator {
        discriminators::AMM_CONFIG => {
            crate::streaming::event_parser::protocols::raydium_cpmm::types::amm_config_parser(account, metadata)
        }
        discriminators::POOL_STATE => {
            crate::streaming::event_parser::protocols::raydium_cpmm::types::pool_state_parser(account, metadata)
        }
        _ => None,
    }
}


/// 解析提款指令事件
fn parse_withdraw_instruction(
    data: &[u8],
    accounts: &[Pubkey],
    mut metadata: EventMetadata,
) -> Option<DexEvent> {
    metadata.event_type = EventType::RaydiumCpmmWithdraw;

    if data.len() < 24 || accounts.len() < 14 {
        return None;
    }
    Some(DexEvent::RaydiumCpmmWithdrawEvent(RaydiumCpmmWithdrawEvent {
        metadata,
        lp_token_amount: read_u64_le(data, 0)?,
        minimum_token0_amount: read_u64_le(data, 8)?,
        minimum_token1_amount: read_u64_le(data, 16)?,
        owner: accounts[0],
        authority: accounts[1],
        pool_state: accounts[2],
        owner_lp_token: accounts[3],
        token0_account: accounts[4],
        token1_account: accounts[5],
        token0_vault: accounts[6],
        token1_vault: accounts[7],
        token_program: accounts[8],
        token_program2022: accounts[9],
        vault0_mint: accounts[10],
        vault1_mint: accounts[11],
        lp_mint: accounts[12],
        memo_program: accounts[13],
    }))
}

/// 解析初始化指令事件
fn parse_initialize_instruction(
    data: &[u8],
    accounts: &[Pubkey],
    mut metadata: EventMetadata,
) -> Option<DexEvent> {
    metadata.event_type = EventType::RaydiumCpmmInitialize;

    if data.len() < 24 || accounts.len() < 20 {
        return None;
    }
    Some(DexEvent::RaydiumCpmmInitializeEvent(RaydiumCpmmInitializeEvent {
        metadata,
        init_amount0: read_u64_le(data, 0)?,
        init_amount1: read_u64_le(data, 8)?,
        open_time: read_u64_le(data, 16)?,
        creator: accounts[0],
        amm_config: accounts[1],
        authority: accounts[2],
        pool_state: accounts[3],
        token0_mint: accounts[4],
        token1_mint: accounts[5],
        lp_mint: accounts[6],
        creator_token0: accounts[7],
        creator_token1: accounts[8],
        creator_lp_token: accounts[9],
        token0_vault: accounts[10],
        token1_vault: accounts[11],
        create_pool_fee: accounts[12],
        observation_state: accounts[13],
        token_program: accounts[14],
        token0_program: accounts[15],
        token1_program: accounts[16],
        associated_token_program: accounts[17],
        system_program: accounts[18],
        rent: accounts[19],
    }))
}

/// 解析存款指令事件
fn parse_deposit_instruction(
    data: &[u8],
    accounts: &[Pubkey],
    mut metadata: EventMetadata,
) -> Option<DexEvent> {
    metadata.event_type = EventType::RaydiumCpmmDeposit;

    if data.len() < 24 || accounts.len() < 13 {
        return None;
    }
    Some(DexEvent::RaydiumCpmmDepositEvent(RaydiumCpmmDepositEvent {
        metadata,
        lp_token_amount: read_u64_le(data, 0)?,
        maximum_token0_amount: read_u64_le(data, 8)?,
        maximum_token1_amount: read_u64_le(data, 16)?,
        owner: accounts[0],
        authority: accounts[1],
        pool_state: accounts[2],
        owner_lp_token: accounts[3],
        token0_account: accounts[4],
        token1_account: accounts[5],
        token0_vault: accounts[6],
        token1_vault: accounts[7],
        token_program: accounts[8],
        token_program2022: accounts[9],
        vault0_mint: accounts[10],
        vault1_mint: accounts[11],
        lp_mint: accounts[12],
    }))
}

/// 解析买入指令事件
fn parse_swap_base_input_instruction(
    data: &[u8],
    accounts: &[Pubkey],
    mut metadata: EventMetadata,
) -> Option<DexEvent> {
    metadata.event_type = EventType::RaydiumCpmmSwapBaseInput;

    if data.len() < 16 || accounts.len() < 13 {
        return None;
    }

    let amount_in = read_u64_le(data, 0)?;
    let minimum_amount_out = read_u64_le(data, 8)?;

    Some(DexEvent::RaydiumCpmmSwapEvent(RaydiumCpmmSwapEvent {
        metadata,
        amount_in,
        minimum_amount_out,
        payer: accounts[0],
        authority: accounts[1],
        amm_config: accounts[2],
        pool_state: accounts[3],
        input_token_account: accounts[4],
        output_token_account: accounts[5],
        input_vault: accounts[6],
        output_vault: accounts[7],
        input_token_program: accounts[8],
        output_token_program: accounts[9],
        input_token_mint: accounts[10],
        output_token_mint: accounts[11],
        observation_state: accounts[12],
        ..Default::default()
    }))
}

fn parse_swap_base_output_instruction(
    data: &[u8],
    accounts: &[Pubkey],
    mut metadata: EventMetadata,
) -> Option<DexEvent> {
    metadata.event_type = EventType::RaydiumCpmmSwapBaseOutput;

    if data.len() < 16 || accounts.len() < 13 {
        return None;
    }

    let max_amount_in = read_u64_le(data, 0)?;
    let amount_out = read_u64_le(data, 8)?;

    Some(DexEvent::RaydiumCpmmSwapEvent(RaydiumCpmmSwapEvent {
        metadata,
        max_amount_in,
        amount_out,
        payer: accounts[0],
        authority: accounts[1],
        amm_config: accounts[2],
        pool_state: accounts[3],
        input_token_account: accounts[4],
        output_token_account: accounts[5],
        input_vault: accounts[6],
        output_vault: accounts[7],
        input_token_program: accounts[8],
        output_token_program: accounts[9],
        input_token_mint: accounts[10],
        output_token_mint: accounts[11],
        observation_state: accounts[12],
        ..Default::default()
    }))
}

/// 从 Anchor 事件日志中解析 SwapEvent 数据
///
/// Anchor 事件日志格式: "Program data: <base64_encoded_event>"
/// 事件数据格式: [8字节鉴别器] [事件数据]
pub fn parse_swap_event_from_log(log_data_base64: &str) -> Option<SwapEventLogData> {
    // 解码 base64
    use base64::{engine::general_purpose::STANDARD, Engine};
    let decoded = STANDARD.decode(log_data_base64).ok()?;
    
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

/// 尝试从交易日志中提取 SwapEvent 数据
///
/// 这个函数可以在有日志数据可用时调用，用于增强 swap 事件的数据
pub fn extract_swap_event_from_logs(logs: &[String], program_id: &Pubkey) -> Option<SwapEventLogData> {
    const PROGRAM_DATA_PREFIX: &str = "Program data: ";
    let program_id_str = program_id.to_string();
    
    // 寻找程序调用和对应的 Program data 日志
    for (i, log) in logs.iter().enumerate() {
        // 检查是否是程序调用
        if log.contains(&program_id_str) && log.contains("invoke") {
            // 在后续日志中查找 Program data
            for j in i+1..logs.len() {
                let data_log = &logs[j];
                
                // 如果遇到下一个程序调用，停止搜索
                if data_log.contains("invoke") {
                    break;
                }
                
                // 尝试提取 Program data
                if let Some(base64_data) = data_log.strip_prefix(PROGRAM_DATA_PREFIX) {
                    if let Some(event_data) = parse_swap_event_from_log(base64_data) {
                        return Some(event_data);
                    }
                }
            }
        }
    }
    
    None
}
