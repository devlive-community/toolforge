Source: https://github.com/shepmaster/sxd-xpath/tree/v0.4.2
Version: 0.4.2 (MIT OR Apache-2.0; license files retained).
Local changes: cooperative evaluation checkpoints, charged string/node comparisons, bounded sum; XPath existential !=/relational and fractional-position corrections. Deprecated Rust syntax has been modernized without lint suppression. Public API otherwise unchanged. Regression coverage lives in ../../src/xpath_test.rs.

- Normalize both signs of numeric zero to `0` for XPath string conversion; the upstream regression test covers this on current Rust.

- Expose function-call discovery through the existing tokenizer so `and (`, `or (`, `div (` and `mod (` remain operators during capability checks.
