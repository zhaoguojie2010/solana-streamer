mod support;
use base64::{engine::general_purpose::STANDARD, Engine};
use solana_sdk::pubkey::Pubkey;
use solana_streamer_sdk::{
    rpc::{
        transaction_frame,
        types::{EncodedTransaction, TransactionBinaryEncoding},
    },
    streaming::event_parser::{ParseOptions, TxExecutionStatus, TxParser},
};
use support::*;

fn encoded(tx: &solana_sdk::transaction::VersionedTransaction) -> EncodedTransaction {
    EncodedTransaction::Binary(
        STANDARD.encode(bincode::serialize(tx).unwrap()),
        TransactionBinaryEncoding::Base64,
    )
}

fn rpc_fixture() -> serde_json::Value {
    let (tx, keys, meta) = versioned_fixture();
    let rpc_input = serde_json::json!({
        "slot":123, "blockTime":1_700_000_000,
        "transaction":encoded(&tx),
        "meta": {
            "err":null, "status":{"Ok":null}, "fee":5000,
            "preBalances":vec![0;7], "postBalances":vec![0;7],
            "loadedAddresses": {
                "writable":keys[4..6].iter().map(ToString::to_string).collect::<Vec<_>>(),
                "readonly":keys[6..].iter().map(ToString::to_string).collect::<Vec<_>>()
            },
            "innerInstructions": meta.inner_instructions.iter().map(|group| serde_json::json!({
                "index":group.index,
                "instructions":group.instructions.iter().map(|ix| serde_json::json!({
                    "programIdIndex":ix.program_id_index, "accounts":ix.accounts,
                    "data":solana_sdk::bs58::encode(&ix.data).into_string(),
                    "stackHeight":ix.stack_height
                })).collect::<Vec<_>>()
            })).collect::<Vec<_>>(),
            "logMessages":["rpc log"],
            "preTokenBalances":[{
                "accountIndex":4, "mint":Pubkey::new_from_array([4;32]).to_string(),
                "owner":keys[0].to_string(), "programId":spl_token::ID.to_string(),
                "uiTokenAmount":{"decimals":6,"amount":"10","uiAmount":0.00001,"uiAmountString":"0.00001"}
            }],
            "postTokenBalances":[{
                "accountIndex":4, "mint":Pubkey::new_from_array([4;32]).to_string(),
                "owner":keys[0].to_string(), "programId":spl_token::ID.to_string(),
                "uiTokenAmount":{"decimals":6,"amount":"20","uiAmount":0.00002,"uiAmountString":"0.00002"}
            }]
        }
    });
    rpc_input
}

#[test]
fn rpc_adapter_preserves_events_keys_source_and_audit() {
    let rpc_input = rpc_fixture();
    let mut parser = TxParser::default();
    let grpc =
        parser.parse_owned(frame(fixture()), &plan(ParseOptions::default())).unwrap().unwrap();
    let response =
        transaction_frame(serde_json::from_value(rpc_input.clone()).unwrap(), 456).unwrap();
    let rpc_response = parser
        .parse_owned(
            response,
            &plan(ParseOptions {
                balance_audit: true,
                retain_logs: true,
                ..ParseOptions::default()
            }),
        )
        .unwrap()
        .unwrap();
    assert_eq!(rpc_response.events, grpc.events);
    assert_eq!(rpc_response.keys, grpc.keys);
    assert_eq!(rpc_response.meta.signature, signature());
    assert_eq!(rpc_response.meta.recv_us, 456);
    assert_eq!(rpc_response.meta.execution_status, TxExecutionStatus::Success);
    assert_eq!(rpc_response.meta.block_time_ms, 1_700_000_000_000);
    assert_eq!(rpc_response.logs.as_ref().unwrap(), &["rpc log"]);
    let audit = rpc_response.audit.as_ref().unwrap();
    assert_eq!(audit.token_balance_changes[0].pre_amount, Some(10));
    assert_eq!(audit.token_balance_changes[0].post_amount, Some(20));
    for (a, b) in rpc_response.view().instructions().zip(grpc.view().instructions()) {
        assert_eq!(a.data, b.data);
        assert_eq!(a.account_indices, b.account_indices);
    }
}

#[test]
fn rpc_adapter_rejects_bad_loaded_keys_and_instruction_encoding() {
    let rpc_input = rpc_fixture();
    let mut invalid = rpc_input.clone();
    invalid["meta"]["loadedAddresses"]["writable"][0] = "invalid key".into();
    assert!(transaction_frame(serde_json::from_value(invalid).unwrap(), 0).is_err());
    let mut invalid = rpc_input;
    invalid["meta"]["innerInstructions"][0]["instructions"][0]["data"] = "!".into();
    assert!(transaction_frame(serde_json::from_value(invalid).unwrap(), 0).is_err());
}

#[test]
fn rpc_adapter_preserves_failed_and_unknown_execution_status() {
    let mut input = rpc_fixture();
    input["meta"]["err"] = "AccountNotFound".into();
    input["meta"]["status"] = serde_json::json!({"Err":"AccountNotFound"});
    let failed = transaction_frame(serde_json::from_value(input).unwrap(), 0).unwrap();
    assert_eq!(failed.metadata().execution_status, TxExecutionStatus::Failed);

    let (mut tx, keys, _) = versioned_fixture();
    let solana_sdk::message::VersionedMessage::V0(message) = &mut tx.message else {
        unreachable!()
    };
    message.account_keys = keys;
    message.address_table_lookups.clear();
    let input =
        serde_json::json!({"slot":123,"blockTime":null,"meta":null,"transaction":encoded(&tx)});
    let unknown = transaction_frame(serde_json::from_value(input).unwrap(), 0).unwrap();
    assert_eq!(unknown.metadata().execution_status, TxExecutionStatus::Unknown);
    assert_eq!(unknown.metadata().block_time, 0);
}
