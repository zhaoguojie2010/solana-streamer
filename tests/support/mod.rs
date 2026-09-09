use solana_sdk::{pubkey::Pubkey, signature::Signature};
use solana_streamer_sdk::streaming::{
    event_parser::{
        protocols::whirlpool::parser::WHIRLPOOL_PROGRAM_ID, ParseOptions, ParsePlan, Protocol,
        TxFrame,
    },
    grpc::TransactionPretty,
};
use yellowstone_grpc_proto::{prelude::*, prost_types::Timestamp};

pub const COMPUTE_BUDGET: Pubkey =
    solana_sdk::pubkey!("ComputeBudget111111111111111111111111111111");
pub const JITO_TIP: Pubkey = solana_sdk::pubkey!("96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5");
pub const INITIALIZE_POOL: [u8; 8] = [95, 180, 10, 172, 84, 174, 232, 40];
pub fn signature() -> Signature {
    Signature::from([7; 64])
}
pub const BLOCK_TIME: Option<Timestamp> =
    Some(Timestamp { seconds: 1_700_000_000, nanos: 123_000_000 });

pub fn instruction(accounts: Vec<u8>, marker: u8) -> CompiledInstruction {
    let mut data = INITIALIZE_POOL.to_vec();
    data.extend_from_slice(&[marker, marker + 1]);
    CompiledInstruction { program_id_index: 3, accounts, data }
}

pub fn fixture() -> SubscribeUpdateTransactionInfo {
    let payer = Pubkey::new_from_array([1; 32]);
    let token = Pubkey::new_from_array([2; 32]);
    let pool = Pubkey::new_from_array([3; 32]);
    let balance = |amount: &str| TokenBalance {
        account_index: 4,
        mint: Pubkey::new_from_array([4; 32]).to_string(),
        owner: payer.to_string(),
        program_id: spl_token::ID.to_string(),
        ui_token_amount: Some(UiTokenAmount {
            decimals: 6,
            amount: amount.to_owned(),
            ..Default::default()
        }),
    };
    let inner = instruction(vec![4, 5, 0], 22);
    let second_inner = instruction(vec![5, 4, 0], 33);
    SubscribeUpdateTransactionInfo {
        signature: signature().as_ref().to_vec(),
        index: 9,
        transaction: Some(Transaction {
            message: Some(Message {
                account_keys: [payer, Pubkey::default(), COMPUTE_BUDGET, WHIRLPOOL_PROGRAM_ID]
                    .iter()
                    .map(|key| key.to_bytes().to_vec())
                    .collect(),
                instructions: vec![
                    CompiledInstruction {
                        program_id_index: 2,
                        data: [&[2][..], &200_000u32.to_le_bytes()].concat(),
                        ..Default::default()
                    },
                    instruction(vec![5, 4, 0], 11),
                    CompiledInstruction {
                        program_id_index: 2,
                        data: [&[3][..], &42u64.to_le_bytes()].concat(),
                        ..Default::default()
                    },
                ],
                versioned: true,
                ..Default::default()
            }),
            ..Default::default()
        }),
        meta: Some(TransactionStatusMeta {
            loaded_writable_addresses: vec![token.to_bytes().to_vec(), pool.to_bytes().to_vec()],
            loaded_readonly_addresses: vec![JITO_TIP.to_bytes().to_vec()],
            inner_instructions: vec![InnerInstructions {
                index: 1,
                instructions: vec![
                    InnerInstruction {
                        program_id_index: inner.program_id_index,
                        accounts: inner.accounts,
                        data: inner.data,
                        stack_height: Some(2),
                    },
                    InnerInstruction {
                        program_id_index: 1,
                        accounts: vec![0, 6],
                        data: [&2u32.to_le_bytes()[..], &5_000u64.to_le_bytes()].concat(),
                        stack_height: Some(2),
                    },
                    InnerInstruction {
                        program_id_index: second_inner.program_id_index,
                        accounts: second_inner.accounts,
                        data: second_inner.data,
                        stack_height: Some(2),
                    },
                ],
            }],
            pre_token_balances: vec![balance("10")],
            post_token_balances: vec![balance("20")],
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub fn frame(info: SubscribeUpdateTransactionInfo) -> TxFrame {
    TxFrame::try_from(TransactionPretty {
        grpc_tx: info,
        slot: 123,
        block_time: BLOCK_TIME,
        ..TransactionPretty::default()
    })
    .unwrap()
}
pub fn plan(options: ParseOptions) -> ParsePlan {
    ParsePlan::new(&[Protocol::Whirlpool], None, options)
}

pub fn versioned_fixture(
) -> (solana_sdk::transaction::VersionedTransaction, Vec<Pubkey>, TransactionStatusMeta) {
    use solana_sdk::{
        message::{compiled_instruction::CompiledInstruction as Ix, v0, VersionedMessage},
        transaction::VersionedTransaction,
    };
    let input = fixture();
    let message = input.transaction.unwrap().message.unwrap();
    let meta = input.meta.unwrap();
    let keys = message
        .account_keys
        .iter()
        .chain(&meta.loaded_writable_addresses)
        .chain(&meta.loaded_readonly_addresses)
        .map(|key| Pubkey::try_from(key.as_slice()).unwrap())
        .collect::<Vec<_>>();
    let tx = VersionedTransaction {
        signatures: vec![signature()],
        message: VersionedMessage::V0(v0::Message {
            header: solana_sdk::message::MessageHeader {
                num_required_signatures: 1,
                num_readonly_signed_accounts: 0,
                num_readonly_unsigned_accounts: 2,
            },
            account_keys: keys[..4].to_vec(),
            address_table_lookups: vec![v0::MessageAddressTableLookup {
                account_key: Pubkey::new_from_array([10; 32]),
                writable_indexes: vec![4, 5],
                readonly_indexes: vec![6],
            }],
            instructions: message
                .instructions
                .into_iter()
                .map(|ix| Ix {
                    program_id_index: ix.program_id_index as u8,
                    accounts: ix.accounts,
                    data: ix.data,
                })
                .collect(),
            ..v0::Message::default()
        }),
    };
    (tx, keys, meta)
}
