//! One invocation scan for both execution data and compute-unit observations.
use crate::streaming::event_parser::{
    core::common_event_parser::COMPUTE_BUDGET_PROGRAM_ID, TxFrame,
};
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

#[derive(Clone, Copy, Debug)]
pub struct ProgramDataItem<'a> {
    pub data: &'a [u8],
    pub program_id: Pubkey,
    pub log_index: usize,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Observation {
    pub first_data: Option<usize>,
    last_data: Option<usize>,
    pub consumed_cu: Option<u32>,
    pub complete: bool,
}
#[derive(Clone, Debug)]
pub(crate) struct DataLocation {
    pub log_index: usize,
    pub next: Option<usize>,
}
#[derive(Debug)]
struct Call {
    program: Pubkey,
    instruction: Option<usize>,
    depth: u32,
}
#[derive(Debug, Default)]
pub(crate) struct InvocationIndex {
    pub observations: Vec<Observation>,
    pub data: Vec<DataLocation>,
    stack: Vec<Call>,
}

fn has_no_invocation_log(program: &Pubkey) -> bool {
    *program == COMPUTE_BUDGET_PROGRAM_ID
        || *program == solana_sdk::pubkey!("Ed25519SigVerify111111111111111111111111111")
        || *program == solana_sdk::pubkey!("KeccakSecp256k11111111111111111111111111111")
        || *program == solana_sdk::pubkey!("Secp256r1SigVerify1111111111111111111111111")
}
impl InvocationIndex {
    pub fn build(&mut self, frame: &TxFrame, collect_data: bool, collect_cu: bool) {
        self.observations.clear();
        self.data.clear();
        self.stack.clear();
        self.observations.resize_with(frame.instructions.len(), Observation::default);
        let mut next_outer = 0;
        let mut next_inner = None;
        let mut group_end = 0;
        let mut aligned = true;
        for (log_index, log) in frame.logs.iter().enumerate() {
            if log.contains("Log truncated") {
                break;
            }
            if let Some(rest) = log.strip_prefix("Program ") {
                if let Some((program, depth)) = rest.split_once(" invoke [") {
                    let (Ok(program), Some(depth)) = (
                        Pubkey::from_str(program),
                        depth.strip_suffix(']').and_then(|s| s.parse::<u32>().ok()),
                    ) else {
                        continue;
                    };
                    if depth == 0 {
                        continue;
                    }
                    let instruction = if depth == 1 {
                        // A missing exit or mismatching program makes positional attribution ambiguous.
                        if !self.stack.is_empty() {
                            aligned = false;
                            self.stack.clear();
                        }
                        while next_outer < frame.instructions.len()
                            && frame.keys[frame.instructions[next_outer].program_id_index as usize]
                                != program
                            && has_no_invocation_log(
                                &frame.keys
                                    [frame.instructions[next_outer].program_id_index as usize],
                            )
                        {
                            next_outer = frame.instructions[next_outer].group_end;
                        }
                        let ix = frame.instructions.get(next_outer);
                        if aligned
                            && ix.is_some_and(|ix| {
                                frame.keys[ix.program_id_index as usize] == program
                            })
                        {
                            let index = next_outer;
                            group_end = ix.unwrap().group_end;
                            next_outer = group_end;
                            next_inner = Some(index + 1);
                            Some(index)
                        } else {
                            aligned = false;
                            next_inner = None;
                            None
                        }
                    } else if self.stack.last().is_some_and(|c| c.depth + 1 == depth) {
                        next_inner.and_then(|index| {
                            let ix = frame.instructions.get(index)?;
                            if index < group_end
                                && frame.keys[ix.program_id_index as usize] == program
                                && ix.stack_height.is_none_or(|h| h == depth)
                            {
                                next_inner = Some(index + 1);
                                Some(index)
                            } else {
                                next_inner = None;
                                None
                            }
                        })
                    } else {
                        for call in &self.stack {
                            if let Some(ix) = call.instruction {
                                self.observations[ix] = Observation::default();
                            }
                        }
                        self.stack.clear();
                        aligned = false;
                        next_inner = None;
                        None
                    };
                    self.stack.push(Call { program, instruction, depth });
                    continue;
                }
                if let Some((program, tail)) = rest.split_once(" consumed ") {
                    if collect_cu {
                        if let (Ok(program), Some(cu)) = (
                            Pubkey::from_str(program),
                            tail.split_once(" of ").and_then(|(cu, _)| cu.parse::<u32>().ok()),
                        ) {
                            if let Some(call) = self.stack.last().filter(|c| c.program == program) {
                                if let Some(ix) = call.instruction {
                                    self.observations[ix].consumed_cu = Some(cu);
                                }
                            }
                        }
                    }
                    continue;
                }
                let exit = rest
                    .strip_suffix(" success")
                    .or_else(|| rest.split_once(" failed:").map(|(program, _)| program));
                if let Some(program) = exit {
                    if let Ok(program) = Pubkey::from_str(program) {
                        if self.stack.last().is_some_and(|c| c.program == program) {
                            if let Some(ix) = self.stack.pop().unwrap().instruction {
                                self.observations[ix].complete = true;
                            }
                        } else {
                            self.stack.clear();
                            next_inner = None;
                            aligned = false;
                        }
                    }
                    continue;
                }
            }
            if collect_data && log.starts_with("Program data: ") {
                if let Some(ix) = self.stack.last().and_then(|c| c.instruction) {
                    let location = self.data.len();
                    self.data.push(DataLocation { log_index, next: None });
                    let observation = &mut self.observations[ix];
                    if let Some(last) = observation.last_data {
                        self.data[last].next = Some(location);
                    } else {
                        observation.first_data = Some(location);
                    }
                    observation.last_data = Some(location);
                }
            }
        }
        // Incomplete invocations expose no CU or execution enrichment. Never borrow a sibling's logs.
        for observation in &mut self.observations {
            if !observation.complete {
                observation.first_data = None;
                observation.consumed_cu = None;
            }
        }
    }
    pub fn trim(&mut self, max_instructions: usize, max_logs: usize) {
        self.stack.clear();
        if self.stack.capacity() > max_instructions {
            self.stack = Vec::new();
        }
        if self.observations.capacity() > max_instructions {
            self.observations = Vec::new();
        }
        if self.data.capacity() > max_logs {
            self.data = Vec::new();
        }
    }
}
