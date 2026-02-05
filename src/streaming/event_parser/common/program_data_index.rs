use crate::streaming::event_parser::common::extract_program_data;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

#[derive(Clone, Debug)]
pub struct ProgramDataItem {
    pub base64: String,
    pub program_id: Pubkey,
    pub depth: usize,
    pub log_index: usize,
}

#[derive(Clone, Debug, Default)]
pub struct ProgramDataIndex {
    pub outer: Vec<Option<ProgramDataItem>>,
    pub inner: Vec<Vec<Option<ProgramDataItem>>>,
}

impl ProgramDataIndex {
    pub fn get_outer(&self, outer_index: i64) -> Option<&ProgramDataItem> {
        if outer_index < 0 {
            return None;
        }
        self.outer.get(outer_index as usize)?.as_ref()
    }

    pub fn get_inner(&self, outer_index: i64, inner_index: i64) -> Option<&ProgramDataItem> {
        if outer_index < 0 || inner_index < 0 {
            return None;
        }
        let outer = self.inner.get(outer_index as usize)?;
        outer.get(inner_index as usize)?.as_ref()
    }
}

#[derive(Clone, Debug)]
struct InvocationSpan {
    program_id: Pubkey,
    depth: usize,
    start: usize,
    end: usize,
}

fn parse_invoke_line(log: &str) -> Option<(Pubkey, usize)> {
    const PREFIX: &str = "Program ";
    const INVOKE_MARK: &str = " invoke [";
    if !log.starts_with(PREFIX) || !log.contains(INVOKE_MARK) {
        return None;
    }
    let rest = &log[PREFIX.len()..];
    let mut parts = rest.split(INVOKE_MARK);
    let program_id_str = parts.next()?.trim();
    let depth_part = parts.next()?.trim();
    let depth_str = depth_part.trim_end_matches(']').trim();
    let program_id = Pubkey::from_str(program_id_str).ok()?;
    let depth = depth_str.parse::<usize>().ok()?;
    Some((program_id, depth))
}

fn is_success_or_failed_line(log: &str) -> bool {
    log.starts_with("Program ") && (log.contains(" success") || log.contains(" failed:"))
}

fn parse_invocation_spans(logs: &[String]) -> Vec<InvocationSpan> {
    let mut spans: Vec<InvocationSpan> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();

    for (idx, log) in logs.iter().enumerate() {
        if let Some((program_id, depth)) = parse_invoke_line(log) {
            spans.push(InvocationSpan {
                program_id,
                depth,
                start: idx,
                end: idx,
            });
            stack.push(spans.len() - 1);
            continue;
        }

        if is_success_or_failed_line(log) {
            if let Some(span_idx) = stack.pop() {
                spans[span_idx].end = idx;
            }
        }
    }

    let last_idx = logs.len().saturating_sub(1);
    for span_idx in stack {
        spans[span_idx].end = last_idx;
    }

    spans
}

fn is_within_child_span(idx: usize, span: &InvocationSpan, spans: &[InvocationSpan]) -> bool {
    spans.iter().any(|child| {
        child.depth > span.depth
            && child.start >= span.start
            && child.end <= span.end
            && idx >= child.start
            && idx <= child.end
    })
}

fn find_program_data_in_span(
    span: &InvocationSpan,
    spans: &[InvocationSpan],
    logs: &[String],
) -> Option<ProgramDataItem> {
    if logs.is_empty() || span.start >= logs.len() {
        return None;
    }
    let end = span.end.min(logs.len() - 1);
    for idx in span.start..=end {
        if is_within_child_span(idx, span, spans) {
            continue;
        }
        if let Some(base64) = extract_program_data(&logs[idx]) {
            return Some(ProgramDataItem {
                base64: base64.to_string(),
                program_id: span.program_id,
                depth: span.depth,
                log_index: idx,
            });
        }
    }
    None
}

pub fn build_program_data_index(
    logs: &[String],
    outer_len: usize,
    inner_instructions: &[yellowstone_grpc_proto::prelude::InnerInstructions],
) -> ProgramDataIndex {
    let mut index = ProgramDataIndex {
        outer: vec![None; outer_len],
        inner: vec![Vec::new(); outer_len],
    };

    for inner in inner_instructions.iter() {
        let outer_idx = inner.index as usize;
        if outer_idx < outer_len {
            index.inner[outer_idx] = vec![None; inner.instructions.len()];
        }
    }

    if logs.is_empty() || outer_len == 0 {
        return index;
    }

    let spans = parse_invocation_spans(logs);
    let outer_spans: Vec<&InvocationSpan> = spans.iter().filter(|span| span.depth == 1).collect();

    for outer_idx in 0..outer_len {
        let Some(outer_span) = outer_spans.get(outer_idx).copied() else {
            continue;
        };
        index.outer[outer_idx] = find_program_data_in_span(outer_span, &spans, logs);

        if index.inner[outer_idx].is_empty() {
            continue;
        }
        let mut inner_spans: Vec<&InvocationSpan> = spans
            .iter()
            .filter(|span| {
                span.depth >= 2
                    && span.start >= outer_span.start
                    && span.end <= outer_span.end
            })
            .collect();
        inner_spans.sort_by_key(|span| span.start);

        for inner_idx in 0..index.inner[outer_idx].len() {
            if let Some(inner_span) = inner_spans.get(inner_idx).copied() {
                index.inner[outer_idx][inner_idx] =
                    find_program_data_in_span(inner_span, &spans, logs);
            }
        }
    }

    index
}
