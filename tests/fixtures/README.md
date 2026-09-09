# Protocol baseline

`protocol_baseline.json` contains 48 direct instruction decoder results captured from
`a63f80f` before this refactor, covering all 10 supported protocols. Inputs are synthetic:
64 distinct keys (with DLMM event-authority/program positions fixed), protocol discriminators,
and a 512-byte zero-filled body. Metadata uses signature `[7; 64]`, slot 123,
block time 1700000000 / 1700000000123 ms and transaction index 9.

Cases are deduplicated by enum variant and EventType. DAMM v2 initialization cases also
exercise the metadata-based activation-point fallback. These are decoder regression fixtures,
not assertions that these synthetic transactions would execute on chain.

`protocol_golden.rs` normalizes only the documented representation changes. The original
expected business fields remain stored here. CPI merging, nonzero log observations, filtering
dependencies, invalid inputs and ownership are tested separately in `parser_ownership.rs`.
