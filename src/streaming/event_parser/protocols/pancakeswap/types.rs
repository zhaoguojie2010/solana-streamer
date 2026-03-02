use crate::streaming::{
    event_parser::{
        common::{EventMetadata, EventType},
        protocols::{
            pancakeswap::{
                PancakeSwapPoolStateAccountEvent,
                PancakeSwapTickArrayBitmapExtensionAccountEvent,
                PancakeSwapTickArrayStateAccountEvent,
            },
            raydium_clmm::types as clmm_types,
        },
        DexEvent,
    },
    grpc::AccountPretty,
};

pub type PoolState = clmm_types::PoolState;
pub type TickArrayState = clmm_types::TickArrayState;
pub type TickArrayBitmapExtension = clmm_types::TickArrayBitmapExtension;

pub const POOL_STATE_SIZE: usize = clmm_types::POOL_STATE_SIZE;
pub const TICK_ARRAY_STATE_SIZE: usize = clmm_types::TICK_ARRAY_STATE_SIZE;
pub const TICK_ARRAY_BITMAP_EXTENSION_SIZE: usize = clmm_types::TICK_ARRAY_BITMAP_EXTENSION_SIZE;

pub fn pool_state_decode(data: &[u8]) -> Option<PoolState> {
    clmm_types::pool_state_decode(data)
}

pub fn tick_array_state_decode(data: &[u8]) -> Option<TickArrayState> {
    clmm_types::tick_array_state_decode(data)
}

pub fn tick_array_bitmap_extension_decode(data: &[u8]) -> Option<TickArrayBitmapExtension> {
    clmm_types::tick_array_bitmap_extension_decode(data)
}

pub fn pool_state_parser(account: AccountPretty, mut metadata: EventMetadata) -> Option<DexEvent> {
    metadata.event_type = EventType::AccountPancakeSwapPoolState;

    if account.data.len() < POOL_STATE_SIZE + 8 {
        return None;
    }
    let pool_state = pool_state_decode(&account.data[8..POOL_STATE_SIZE + 8])?;
    Some(DexEvent::PancakeSwapPoolStateAccountEvent(
        PancakeSwapPoolStateAccountEvent {
            metadata,
            pubkey: account.pubkey,
            executable: account.executable,
            lamports: account.lamports,
            owner: account.owner,
            rent_epoch: account.rent_epoch,
            raw_account_data: account.data,
            pool_state,
        },
    ))
}

pub fn tick_array_state_parser(
    account: AccountPretty,
    mut metadata: EventMetadata,
) -> Option<DexEvent> {
    metadata.event_type = EventType::AccountPancakeSwapTickArrayState;

    if account.data.len() < TICK_ARRAY_STATE_SIZE + 8 {
        return None;
    }
    let tick_array_state = tick_array_state_decode(&account.data[8..TICK_ARRAY_STATE_SIZE + 8])?;
    Some(DexEvent::PancakeSwapTickArrayStateAccountEvent(
        PancakeSwapTickArrayStateAccountEvent {
            metadata,
            pubkey: account.pubkey,
            executable: account.executable,
            lamports: account.lamports,
            owner: account.owner,
            rent_epoch: account.rent_epoch,
            raw_account_data: account.data,
            tick_array_state,
        },
    ))
}

pub fn tick_array_bitmap_extension_parser(
    account: AccountPretty,
    mut metadata: EventMetadata,
) -> Option<DexEvent> {
    metadata.event_type = EventType::AccountPancakeSwapTickArrayBitmapExtension;

    if account.data.len() < TICK_ARRAY_BITMAP_EXTENSION_SIZE + 8 {
        return None;
    }
    let tick_array_bitmap_extension =
        tick_array_bitmap_extension_decode(&account.data[8..TICK_ARRAY_BITMAP_EXTENSION_SIZE + 8])?;
    Some(DexEvent::PancakeSwapTickArrayBitmapExtensionAccountEvent(
        PancakeSwapTickArrayBitmapExtensionAccountEvent {
            metadata,
            pubkey: account.pubkey,
            executable: account.executable,
            lamports: account.lamports,
            owner: account.owner,
            rent_epoch: account.rent_epoch,
            raw_account_data: account.data,
            tick_array_bitmap_extension,
        },
    ))
}
