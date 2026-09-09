//! Decoder outputs captured from the pre-refactor library (a63f80f).
//! Only the documented metadata/source representation changes are normalized.
use serde::Deserialize;
use serde_json::{json, Value};
use solana_sdk::{pubkey::Pubkey, signature::Signature};
use solana_streamer_sdk::streaming::event_parser::{
    common::{EventMetadata, EventType},
    core::dispatcher::EventDispatcher,
    AccountEvent, InstructionAccounts, ParseOptions, ParsePlan, Protocol, TxEvent, TxFrame,
    TxMetadata, TxParser,
};
use yellowstone_grpc_proto::prelude::CompiledInstruction;

#[derive(Deserialize)]
struct Case {
    name: String,
    protocol: String,
    program: [u8; 32],
    keys: Vec<[u8; 32]>,
    discriminator: Vec<u8>,
    data: Vec<u8>,
    expected: Value,
}
#[test]
fn all_protocols_match_pre_refactor_business_fields() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/protocol_baseline.json")).unwrap();
    assert_eq!(cases.len(), 48);
    let mut protocols = std::collections::HashSet::new();
    for case in cases {
        let protocol: Protocol = if case.protocol == "MeteoraDammV2" {
            Protocol::MeteoraDammV2
        } else {
            case.protocol.parse().unwrap()
        };
        protocols.insert(protocol);
        let mut keys: Vec<_> = case.keys.iter().copied().map(Pubkey::new_from_array).collect();
        let indices: Vec<_> = (0..keys.len() as u8).collect();
        let tx_meta = TxMetadata {
            signature: Signature::from([7; 64]),
            slot: 123,
            block_time: 1_700_000_000,
            block_time_ms: 1_700_000_000_123,
            transaction_index: Some(9),
            ..TxMetadata::default()
        };
        let metadata = EventMetadata {
            program_id: Pubkey::new_from_array(case.program),
            ..EventMetadata::default()
        };
        let event = EventDispatcher::dispatch_instruction(
            protocol,
            &case.discriminator,
            &case.data,
            InstructionAccounts::new(&keys, &indices).unwrap(),
            metadata,
            &tx_meta,
        )
        .unwrap_or_else(|| panic!("{} no longer decodes", case.name));
        let actual = serde_json::to_value(&event).unwrap();
        let mut expected = case.expected;
        let payload =
            expected.as_object_mut().unwrap().values_mut().next().unwrap().as_object_mut().unwrap();
        let old_meta = payload["metadata"].clone();
        payload.insert("metadata".into(), json!({
            "protocol":old_meta["protocol"], "event_type": if matches!(event,TxEvent::MeteoraDlmmInstructionEvent(_)) { json!("MeteoraDlmmInstruction") } else {old_meta["event_type"].clone()},
            "program_id":old_meta["program_id"], "instruction_index":0, "outer_index":0, "inner_index":null,
            "swap_compute_units":null, "swap_data":null
        }));
        if let Some(remaining) = payload.remove("remaining_accounts") {
            let indices: Vec<_> = remaining
                .as_array()
                .unwrap()
                .iter()
                .map(|key| case.keys.iter().position(|k| json!(k) == *key).unwrap())
                .collect();
            payload.insert("remaining_account_indices".into(), json!(indices));
        }
        let old_accounts = payload.remove("accounts");
        let old_data = payload.remove("data");
        assert_eq!(actual, expected, "business field regression: {}", case.name);
        // A type-selective plan must classify correctly BEFORE decoding.
        if !matches!(event, TxEvent::PumpFunMigrateEvent(_)) {
            keys.push(Pubkey::new_from_array(case.program));
            let data = [case.discriminator.as_slice(), case.data.as_slice()].concat();
            let input = TxFrame::new(
                tx_meta,
                keys,
                vec![CompiledInstruction {
                    program_id_index: case.keys.len() as u32,
                    accounts: indices,
                    data,
                }],
                vec![],
                vec![],
            )
            .unwrap();
            let plan = ParsePlan::new(
                &[protocol],
                Some(&[event.metadata().event_type]),
                ParseOptions::default(),
            );
            let batch = TxParser::default()
                .parse_owned(input, &plan)
                .unwrap()
                .unwrap_or_else(|| panic!("filter rejected {}", case.name));
            assert_eq!(batch.events.len(), 1, "{}", case.name);
            assert_eq!(batch.events[0], event, "{}", case.name);
            if let (Some(accounts), Some(data)) = (old_accounts, old_data) {
                let source = batch
                    .instruction(0)
                    .expect("modeled events keep their source even with raw retention off");
                assert_eq!(
                    json!(source.accounts.iter().collect::<Vec<_>>()),
                    accounts,
                    "{}",
                    case.name
                );
                assert_eq!(json!(&source.data[case.discriminator.len()..]), data, "{}", case.name);
            }
        }
    }
    assert_eq!(protocols.len(), 10);
}

#[test]
fn transaction_layout_is_independent_of_large_account_snapshots() {
    assert!(size_of::<TxEvent>() <= 1400);
    assert!(size_of::<EventMetadata>() <= 128);
    assert!(size_of::<AccountEvent>() > 10_000);
    assert_ne!(EventType::MeteoraDlmmInstruction, EventType::PumpSwapBuy);
    println!(
        "TxEvent={} AccountEvent={} EventMetadata={}",
        size_of::<TxEvent>(),
        size_of::<AccountEvent>(),
        size_of::<EventMetadata>()
    );
}

#[test]
fn short_protocol_inputs_return_without_panicking() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/protocol_baseline.json")).unwrap();
    for case in cases {
        let protocol: Protocol = case.protocol.parse().unwrap();
        let keys: Vec<_> = case.keys.into_iter().map(Pubkey::new_from_array).collect();
        for count in 0..16 {
            let indices: Vec<u8> = (0..count).collect();
            for len in [0, 1, 8, 16, 24, 32, 64, 512] {
                let _ = EventDispatcher::dispatch_instruction(
                    protocol,
                    &case.discriminator,
                    &case.data[..len],
                    InstructionAccounts::new(&keys, &indices).unwrap(),
                    EventMetadata::default(),
                    &TxMetadata::default(),
                );
            }
        }
    }
}
