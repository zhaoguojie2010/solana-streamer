use crate::streaming::event_parser::InstructionAccounts;
use crate::streaming::event_parser::{
    common::{read_u64_le, EventMetadata, EventType},
    protocols::raydium_amm_v4::{
        discriminators, RaydiumAmmV4DepositEvent, RaydiumAmmV4Initialize2Event,
        RaydiumAmmV4SwapEvent, RaydiumAmmV4WithdrawEvent, RaydiumAmmV4WithdrawPnlEvent,
    },
    TxEvent,
};
use solana_sdk::pubkey::Pubkey;

/// Raydium AMM V4程序ID
pub const RAYDIUM_AMM_V4_PROGRAM_ID: Pubkey =
    solana_sdk::pubkey!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");

/// 解析 Raydium AMM V4 instruction data
///
/// 根据判别器路由到具体的 instruction 解析函数
pub fn parse_raydium_amm_v4_instruction_data(
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
        discriminators::INITIALIZE2 => parse_initialize2_instruction(data, accounts, metadata),
        discriminators::WITHDRAW => parse_withdraw_instruction(data, accounts, metadata),
        discriminators::WITHDRAW_PNL => parse_withdraw_pnl_instruction(data, accounts, metadata),
        _ => None,
    }
}

/// 解析 Raydium AMM V4 inner instruction data
///
/// Raydium AMM V4 没有 inner instruction 事件
pub fn parse_raydium_amm_v4_inner_instruction_data(
    _discriminator: &[u8],
    _data: &[u8],
    _metadata: EventMetadata,
) -> Option<TxEvent> {
    None
}

/// 解析 Raydium AMM V4 账户数据
///
/// 根据判别器路由到具体的账户解析函数
pub fn parse_raydium_amm_v4_account_data(
    discriminator: &[u8],
    account: crate::streaming::grpc::AccountFrame,
    metadata: crate::streaming::event_parser::common::EventMetadata,
) -> Option<crate::streaming::event_parser::AccountEvent> {
    match discriminator {
        discriminators::AMM_INFO => {
            crate::streaming::event_parser::protocols::raydium_amm_v4::types::amm_info_parser(
                account, metadata,
            )
        }
        _ => None,
    }
}

/// 解析提现指令事件
fn parse_withdraw_pnl_instruction(
    _data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::RaydiumAmmV4WithdrawPnl;

    if accounts.len() < 17 {
        return None;
    }

    Some(TxEvent::RaydiumAmmV4WithdrawPnlEvent(RaydiumAmmV4WithdrawPnlEvent {
        metadata,
        token_program: *accounts.get(0)?,
        amm: *accounts.get(1)?,
        amm_config: *accounts.get(2)?,
        amm_authority: *accounts.get(3)?,
        amm_open_orders: *accounts.get(4)?,
        pool_coin_token_account: *accounts.get(5)?,
        pool_pc_token_account: *accounts.get(6)?,
        coin_pnl_token_account: *accounts.get(7)?,
        pc_pnl_token_account: *accounts.get(8)?,
        pnl_owner_account: *accounts.get(9)?,
        amm_target_orders: *accounts.get(10)?,
        serum_program: *accounts.get(11)?,
        serum_market: *accounts.get(12)?,
        serum_event_queue: *accounts.get(13)?,
        serum_coin_vault_account: *accounts.get(14)?,
        serum_pc_vault_account: *accounts.get(15)?,
        serum_vault_signer: *accounts.get(16)?,
    }))
}

/// 解析移除流动性指令事件
fn parse_withdraw_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::RaydiumAmmV4Withdraw;

    if data.len() < 8 || accounts.len() < 22 {
        return None;
    }
    let amount = read_u64_le(data, 0)?;

    Some(TxEvent::RaydiumAmmV4WithdrawEvent(RaydiumAmmV4WithdrawEvent {
        metadata,
        amount,

        token_program: *accounts.get(0)?,
        amm: *accounts.get(1)?,
        amm_authority: *accounts.get(2)?,
        amm_open_orders: *accounts.get(3)?,
        amm_target_orders: *accounts.get(4)?,
        lp_mint_address: *accounts.get(5)?,
        pool_coin_token_account: *accounts.get(6)?,
        pool_pc_token_account: *accounts.get(7)?,
        pool_withdraw_queue: *accounts.get(8)?,
        pool_temp_lp_token_account: *accounts.get(9)?,
        serum_program: *accounts.get(10)?,
        serum_market: *accounts.get(11)?,
        serum_coin_vault_account: *accounts.get(12)?,
        serum_pc_vault_account: *accounts.get(13)?,
        serum_vault_signer: *accounts.get(14)?,
        user_lp_token_account: *accounts.get(15)?,
        user_coin_token_account: *accounts.get(16)?,
        user_pc_token_account: *accounts.get(17)?,
        user_owner: *accounts.get(18)?,
        serum_event_queue: *accounts.get(19)?,
        serum_bids: *accounts.get(20)?,
        serum_asks: *accounts.get(21)?,
    }))
}

/// 解析初始化指令事件
fn parse_initialize2_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::RaydiumAmmV4Initialize2;

    if data.len() < 25 || accounts.len() < 21 {
        return None;
    }
    let nonce = data[0];
    let open_time = read_u64_le(data, 1)?;
    let init_pc_amount = read_u64_le(data, 9)?;
    let init_coin_amount = read_u64_le(data, 17)?;

    Some(TxEvent::RaydiumAmmV4Initialize2Event(RaydiumAmmV4Initialize2Event {
        metadata,
        nonce,
        open_time,
        init_pc_amount,
        init_coin_amount,

        token_program: *accounts.get(0)?,
        spl_associated_token_account: *accounts.get(1)?,
        system_program: *accounts.get(2)?,
        rent: *accounts.get(3)?,
        amm: *accounts.get(4)?,
        amm_authority: *accounts.get(5)?,
        amm_open_orders: *accounts.get(6)?,
        lp_mint: *accounts.get(7)?,
        coin_mint: *accounts.get(8)?,
        pc_mint: *accounts.get(9)?,
        pool_coin_token_account: *accounts.get(10)?,
        pool_pc_token_account: *accounts.get(11)?,
        pool_withdraw_queue: *accounts.get(12)?,
        amm_target_orders: *accounts.get(13)?,
        pool_temp_lp: *accounts.get(14)?,
        serum_program: *accounts.get(15)?,
        serum_market: *accounts.get(16)?,
        user_wallet: *accounts.get(17)?,
        user_token_coin: *accounts.get(18)?,
        user_token_pc: *accounts.get(19)?,
        user_lp_token_account: *accounts.get(20)?,
    }))
}

/// 解析添加流动性指令事件
fn parse_deposit_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::RaydiumAmmV4Deposit;

    if data.len() < 24 || accounts.len() < 14 {
        return None;
    }
    let max_coin_amount = read_u64_le(data, 0)?;
    let max_pc_amount = read_u64_le(data, 8)?;
    let base_side = read_u64_le(data, 16)?;

    Some(TxEvent::RaydiumAmmV4DepositEvent(RaydiumAmmV4DepositEvent {
        metadata,
        max_coin_amount,
        max_pc_amount,
        base_side,

        token_program: *accounts.get(0)?,
        amm: *accounts.get(1)?,
        amm_authority: *accounts.get(2)?,
        amm_open_orders: *accounts.get(3)?,
        amm_target_orders: *accounts.get(4)?,
        lp_mint_address: *accounts.get(5)?,
        pool_coin_token_account: *accounts.get(6)?,
        pool_pc_token_account: *accounts.get(7)?,
        serum_market: *accounts.get(8)?,
        user_coin_token_account: *accounts.get(9)?,
        user_pc_token_account: *accounts.get(10)?,
        user_lp_token_account: *accounts.get(11)?,
        user_owner: *accounts.get(12)?,
        serum_event_queue: *accounts.get(13)?,
    }))
}

/// 解析买入指令事件
fn parse_swap_base_output_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::RaydiumAmmV4SwapBaseOut;

    if data.len() < 16 || accounts.len() < 17 {
        return None;
    }
    let max_amount_in = read_u64_le(data, 0)?;
    let amount_out = read_u64_le(data, 8)?;

    let accounts = if accounts.len() == 17 { accounts.with_missing(4) } else { accounts };

    Some(TxEvent::RaydiumAmmV4SwapEvent(RaydiumAmmV4SwapEvent {
        metadata,
        max_amount_in,
        amount_out,

        token_program: *accounts.get(0)?,
        amm: *accounts.get(1)?,
        amm_authority: *accounts.get(2)?,
        amm_open_orders: *accounts.get(3)?,
        amm_target_orders: Some(*accounts.get(4)?),
        pool_coin_token_account: *accounts.get(5)?,
        pool_pc_token_account: *accounts.get(6)?,
        serum_program: *accounts.get(7)?,
        serum_market: *accounts.get(8)?,
        serum_bids: *accounts.get(9)?,
        serum_asks: *accounts.get(10)?,
        serum_event_queue: *accounts.get(11)?,
        serum_coin_vault_account: *accounts.get(12)?,
        serum_pc_vault_account: *accounts.get(13)?,
        serum_vault_signer: *accounts.get(14)?,
        user_source_token_account: *accounts.get(15)?,
        user_destination_token_account: *accounts.get(16)?,
        user_source_owner: *accounts.get(17)?,

        ..Default::default()
    }))
}

/// 解析买入指令事件
fn parse_swap_base_input_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::RaydiumAmmV4SwapBaseIn;

    if data.len() < 16 || accounts.len() < 17 {
        return None;
    }
    let amount_in = read_u64_le(data, 0)?;
    let minimum_amount_out = read_u64_le(data, 8)?;

    let accounts = if accounts.len() == 17 { accounts.with_missing(4) } else { accounts };

    Some(TxEvent::RaydiumAmmV4SwapEvent(RaydiumAmmV4SwapEvent {
        metadata,
        amount_in,
        minimum_amount_out,

        token_program: *accounts.get(0)?,
        amm: *accounts.get(1)?,
        amm_authority: *accounts.get(2)?,
        amm_open_orders: *accounts.get(3)?,
        amm_target_orders: Some(*accounts.get(4)?),
        pool_coin_token_account: *accounts.get(5)?,
        pool_pc_token_account: *accounts.get(6)?,
        serum_program: *accounts.get(7)?,
        serum_market: *accounts.get(8)?,
        serum_bids: *accounts.get(9)?,
        serum_asks: *accounts.get(10)?,
        serum_event_queue: *accounts.get(11)?,
        serum_coin_vault_account: *accounts.get(12)?,
        serum_pc_vault_account: *accounts.get(13)?,
        serum_vault_signer: *accounts.get(14)?,
        user_source_token_account: *accounts.get(15)?,
        user_destination_token_account: *accounts.get(16)?,
        user_source_owner: *accounts.get(17)?,

        ..Default::default()
    }))
}

/// Classify before allocating or decoding a protocol instruction.
pub(crate) fn instruction_event_type(discriminator: &[u8]) -> Option<EventType> {
    match discriminator {
        discriminators::SWAP_BASE_IN => Some(EventType::RaydiumAmmV4SwapBaseIn),
        discriminators::SWAP_BASE_OUT => Some(EventType::RaydiumAmmV4SwapBaseOut),
        discriminators::DEPOSIT => Some(EventType::RaydiumAmmV4Deposit),
        discriminators::INITIALIZE2 => Some(EventType::RaydiumAmmV4Initialize2),
        discriminators::WITHDRAW => Some(EventType::RaydiumAmmV4Withdraw),
        discriminators::WITHDRAW_PNL => Some(EventType::RaydiumAmmV4WithdrawPnl),
        _ => None,
    }
}

/// Classify before allocating or decoding a protocol account.
pub(crate) fn account_event_type(discriminator: &[u8]) -> Option<EventType> {
    match discriminator {
        discriminators::AMM_INFO => Some(EventType::AccountRaydiumAmmV4AmmInfo),
        _ => None,
    }
}
