use crate::streaming::event_parser::{
    common::{read_u64_le, EventMetadata, EventType},
    protocols::meteora_dlmm::{
        discriminators, meteora_dlmm_swap2_event_decode, meteora_dlmm_swap_event_decode,
        MeteoraDlmmSwap2Event, MeteoraDlmmSwapEvent,
    },
    DexEvent,
};
use solana_sdk::pubkey::Pubkey;

/// Meteora DLMM 程序ID
pub const METEORA_DLMM_PROGRAM_ID: Pubkey =
    solana_sdk::pubkey!("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo");

#[derive(Clone, Debug)]
struct ParsedSwapAccounts {
    lb_pair: Pubkey,
    bin_array_bitmap_extension: Option<Pubkey>,
    reserve_x: Option<Pubkey>,
    reserve_y: Option<Pubkey>,
    user_token_in: Option<Pubkey>,
    user_token_out: Option<Pubkey>,
    token_x_mint: Option<Pubkey>,
    token_y_mint: Option<Pubkey>,
    oracle: Option<Pubkey>,
    host_fee_in: Option<Pubkey>,
    user: Pubkey,
    token_x_program: Pubkey,
    token_y_program: Pubkey,
    memo_program: Option<Pubkey>,
    event_authority: Pubkey,
    program: Pubkey,
    remaining_accounts: Vec<Pubkey>,
}

/// 解析 Meteora DLMM instruction data
pub fn parse_meteora_dlmm_instruction_data(
    discriminator: &[u8],
    data: &[u8],
    accounts: &[Pubkey],
    metadata: EventMetadata,
) -> Option<DexEvent> {
    match discriminator {
        discriminators::SWAP_IX => parse_swap_instruction(data, accounts, metadata),
        discriminators::SWAP2_IX => parse_swap2_instruction(data, accounts, metadata),
        _ => None,
    }
}

pub fn is_meteora_dlmm_swap_instruction(discriminator: &[u8]) -> bool {
    matches!(discriminator, discriminators::SWAP_IX | discriminators::SWAP2_IX)
}

/// 解析 Meteora DLMM inner instruction data (CPI events)
pub fn parse_meteora_dlmm_inner_instruction_data(
    discriminator: &[u8],
    data: &[u8],
    metadata: EventMetadata,
) -> Option<DexEvent> {
    match discriminator {
        discriminators::SWAP_EVENT => parse_swap_inner_instruction(data, metadata),
        discriminators::SWAP2_EVENT => parse_swap2_inner_instruction(data, metadata),
        _ => None,
    }
}

/// 解析 Meteora DLMM 账户数据
///
/// 根据判别器路由到具体的账户解析函数
pub fn parse_meteora_dlmm_account_data(
    discriminator: &[u8],
    account: crate::streaming::grpc::AccountPretty,
    metadata: crate::streaming::event_parser::common::EventMetadata,
) -> Option<crate::streaming::event_parser::DexEvent> {
    match discriminator {
        discriminators::LB_PAIR => {
            crate::streaming::event_parser::protocols::meteora_dlmm::types::lb_pair_parser(
                account,
                metadata,
            )
        }
        discriminators::BIN_ARRAY => {
            crate::streaming::event_parser::protocols::meteora_dlmm::types::bin_array_parser(
                account,
                metadata,
            )
        }
        discriminators::BIN_ARRAY_BITMAP_EXTENSION => {
            crate::streaming::event_parser::protocols::meteora_dlmm::types::bin_array_bitmap_extension_parser(
                account,
                metadata,
            )
        }
        _ => None,
    }
}

fn parse_swap_instruction(
    data: &[u8],
    accounts: &[Pubkey],
    mut metadata: EventMetadata,
) -> Option<DexEvent> {
    metadata.event_type = EventType::MeteoraDlmmSwap;
    if data.len() < 16 {
        return None;
    }

    let amount_in = read_u64_le(data, 0)?;
    let min_amount_out = read_u64_le(data, 8)?;
    let parsed_accounts = parse_swap_accounts(accounts, false)?;

    Some(DexEvent::MeteoraDlmmSwapEvent(MeteoraDlmmSwapEvent {
        metadata,
        amount_in,
        min_amount_out,
        lb_pair: parsed_accounts.lb_pair,
        bin_array_bitmap_extension: parsed_accounts.bin_array_bitmap_extension,
        reserve_x: parsed_accounts.reserve_x,
        reserve_y: parsed_accounts.reserve_y,
        user_token_in: parsed_accounts.user_token_in,
        user_token_out: parsed_accounts.user_token_out,
        token_x_mint: parsed_accounts.token_x_mint,
        token_y_mint: parsed_accounts.token_y_mint,
        oracle: parsed_accounts.oracle,
        host_fee_in: parsed_accounts.host_fee_in,
        user: parsed_accounts.user,
        token_x_program: parsed_accounts.token_x_program,
        token_y_program: parsed_accounts.token_y_program,
        event_authority: parsed_accounts.event_authority,
        program: parsed_accounts.program,
        remaining_accounts: parsed_accounts.remaining_accounts,
        ..Default::default()
    }))
}

fn parse_swap2_instruction(
    data: &[u8],
    accounts: &[Pubkey],
    mut metadata: EventMetadata,
) -> Option<DexEvent> {
    metadata.event_type = EventType::MeteoraDlmmSwap2;
    if data.len() < 16 {
        return None;
    }

    let amount_in = read_u64_le(data, 0)?;
    let min_amount_out = read_u64_le(data, 8)?;
    let parsed_accounts = parse_swap_accounts(accounts, true)?;

    Some(DexEvent::MeteoraDlmmSwap2Event(MeteoraDlmmSwap2Event {
        metadata,
        amount_in,
        min_amount_out,
        lb_pair: parsed_accounts.lb_pair,
        bin_array_bitmap_extension: parsed_accounts.bin_array_bitmap_extension,
        reserve_x: parsed_accounts.reserve_x,
        reserve_y: parsed_accounts.reserve_y,
        user_token_in: parsed_accounts.user_token_in,
        user_token_out: parsed_accounts.user_token_out,
        token_x_mint: parsed_accounts.token_x_mint,
        token_y_mint: parsed_accounts.token_y_mint,
        oracle: parsed_accounts.oracle,
        host_fee_in: parsed_accounts.host_fee_in,
        user: parsed_accounts.user,
        token_x_program: parsed_accounts.token_x_program,
        token_y_program: parsed_accounts.token_y_program,
        memo_program: parsed_accounts.memo_program.unwrap_or_default(),
        event_authority: parsed_accounts.event_authority,
        program: parsed_accounts.program,
        remaining_accounts: parsed_accounts.remaining_accounts,
        ..Default::default()
    }))
}

fn parse_swap_inner_instruction(data: &[u8], mut metadata: EventMetadata) -> Option<DexEvent> {
    metadata.event_type = EventType::MeteoraDlmmSwap;
    let cpi_event = meteora_dlmm_swap_event_decode(data)?;
    Some(DexEvent::MeteoraDlmmSwapEvent(MeteoraDlmmSwapEvent {
        metadata,
        lb_pair: cpi_event.lb_pair,
        from: cpi_event.from,
        start_bin_id: cpi_event.start_bin_id,
        end_bin_id: cpi_event.end_bin_id,
        cpi_amount_in: cpi_event.amount_in,
        cpi_amount_out: cpi_event.amount_out,
        swap_for_y: cpi_event.swap_for_y,
        fee: cpi_event.fee,
        protocol_fee: cpi_event.protocol_fee,
        fee_bps: cpi_event.fee_bps,
        host_fee: cpi_event.host_fee,
        ..Default::default()
    }))
}

fn parse_swap2_inner_instruction(data: &[u8], mut metadata: EventMetadata) -> Option<DexEvent> {
    metadata.event_type = EventType::MeteoraDlmmSwap2;
    let cpi_event = meteora_dlmm_swap2_event_decode(data)?;
    Some(DexEvent::MeteoraDlmmSwap2Event(MeteoraDlmmSwap2Event {
        metadata,
        lb_pair: cpi_event.lb_pair,
        from: cpi_event.from,
        start_bin_id: cpi_event.start_bin_id,
        end_bin_id: cpi_event.end_bin_id,
        swap_for_y: cpi_event.swap_for_y,
        fee_bps: cpi_event.fee_bps,
        swap_result: cpi_event.swap_result,
        ..Default::default()
    }))
}

fn parse_swap_accounts(accounts: &[Pubkey], has_memo_program: bool) -> Option<ParsedSwapAccounts> {
    if accounts.len() < 8 {
        return None;
    }

    let event_authority =
        Pubkey::find_program_address(&[b"__event_authority"], &METEORA_DLMM_PROGRAM_ID).0;
    let event_authority_index = accounts
        .windows(2)
        .position(|window| window[0] == event_authority && window[1] == METEORA_DLMM_PROGRAM_ID)?;

    let (user_index, token_x_program_index, token_y_program_index, memo_program_index) =
        if has_memo_program {
            if event_authority_index < 4 {
                return None;
            }
            (
                event_authority_index - 4,
                event_authority_index - 3,
                event_authority_index - 2,
                Some(event_authority_index - 1),
            )
        } else {
            if event_authority_index < 3 {
                return None;
            }
            (event_authority_index - 3, event_authority_index - 2, event_authority_index - 1, None)
        };

    let prefix = &accounts[..user_index];
    let (
        lb_pair,
        bin_array_bitmap_extension,
        reserve_x,
        reserve_y,
        user_token_in,
        user_token_out,
        token_x_mint,
        token_y_mint,
        oracle,
        host_fee_in,
    ) = parse_swap_prefix(prefix)?;

    let remaining_start = event_authority_index + 2;
    let remaining_accounts = if remaining_start < accounts.len() {
        accounts[remaining_start..].to_vec()
    } else {
        vec![]
    };

    Some(ParsedSwapAccounts {
        lb_pair,
        bin_array_bitmap_extension,
        reserve_x,
        reserve_y,
        user_token_in,
        user_token_out,
        token_x_mint,
        token_y_mint,
        oracle,
        host_fee_in,
        user: *accounts.get(user_index)?,
        token_x_program: *accounts.get(token_x_program_index)?,
        token_y_program: *accounts.get(token_y_program_index)?,
        memo_program: memo_program_index.and_then(|idx| accounts.get(idx).copied()),
        event_authority: *accounts.get(event_authority_index)?,
        program: *accounts.get(event_authority_index + 1)?,
        remaining_accounts,
    })
}

#[allow(clippy::type_complexity)]
fn parse_swap_prefix(
    prefix: &[Pubkey],
) -> Option<(
    Pubkey,
    Option<Pubkey>,
    Option<Pubkey>,
    Option<Pubkey>,
    Option<Pubkey>,
    Option<Pubkey>,
    Option<Pubkey>,
    Option<Pubkey>,
    Option<Pubkey>,
    Option<Pubkey>,
)> {
    if prefix.len() < 8 {
        return None;
    }

    let lb_pair = *prefix.first()?;
    let mut bin_array_bitmap_extension: Option<Pubkey> = None;
    let mut host_fee_in: Option<Pubkey> = None;

    let reserve_x_index = match prefix.len() {
        // [lb_pair, reserve_x, reserve_y, user_in, user_out, mint_x, mint_y, oracle]
        8 => 1,
        // Ambiguous:
        // - [lb_pair, bin_ext, reserve_x, reserve_y, user_in, user_out, mint_x, mint_y, oracle]
        // - [lb_pair, reserve_x, reserve_y, user_in, user_out, mint_x, mint_y, oracle, host_fee]
        // DLMM swap paths commonly include bin extension; default to that layout.
        9 => {
            bin_array_bitmap_extension = Some(*prefix.get(1)?);
            2
        }
        // [lb_pair, bin_ext, reserve_x, reserve_y, user_in, user_out, mint_x, mint_y, oracle, host_fee]
        10 => {
            bin_array_bitmap_extension = Some(*prefix.get(1)?);
            host_fee_in = Some(*prefix.get(9)?);
            2
        }
        _ => return None,
    };

    let reserve_x = prefix.get(reserve_x_index).copied();
    let reserve_y = prefix.get(reserve_x_index + 1).copied();
    let user_token_in = prefix.get(reserve_x_index + 2).copied();
    let user_token_out = prefix.get(reserve_x_index + 3).copied();
    let token_x_mint = prefix.get(reserve_x_index + 4).copied();
    let token_y_mint = prefix.get(reserve_x_index + 5).copied();
    let oracle = prefix.get(reserve_x_index + 6).copied();

    Some((
        lb_pair,
        bin_array_bitmap_extension,
        reserve_x,
        reserve_y,
        user_token_in,
        user_token_out,
        token_x_mint,
        token_y_mint,
        oracle,
        host_fee_in,
    ))
}
