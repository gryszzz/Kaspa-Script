# KaspaScript

KaspaScript is a Rust compiler for a post-Toccata Kaspa smart contract
language. The workspace now runs the source-to-artifact pipeline end to end:
lexer, parser, semantic analyzer, IR, Toccata bytecode backend, SDK, CLI, and
contract pattern tests.

## Current Status

Implemented now:

- `compiler/ir`: validated V1 AST lowering into deterministic compiler IR with
  instruction IDs, protocol limits, and integer overflow checks.
- `compiler/lexer`: position-tagged KaspaScript lexer with keyword, type,
  literal, operator, delimiter, and comment support.
- `compiler/parser`: V1 contract parser and AST for `contract`, `params`,
  `spend`, `require`, calls, member access, arrays, and comparisons.
- `compiler/codegen`: deterministic Toccata backend and compiled artifact
  metadata with source-grounded warnings for gated protocol assumptions.
- `sdk`: Rust SDK compile entry point plus spend transaction builder with
  finality-depth enforcement and 10bps treasury fee injection.
- `cli`: `compile`, `verify`, and `inspect` commands.
- `compiler/semantic`: safety checks for duplicate names, finality depth,
  missing spend guards, parameter shadowing, unknown `require` roots, and
  required Kaspa feature extraction.
- `contracts/production/DAGSafeVault.ks`: whole-UTXO vault contract using
  covenant IDs, finality depth, and sequencing commitments.
- `tests/contracts`: escrow, timelock, multisig, atomic swap, and vault V1
  pattern contracts.
- `docs/source-grounding.md`: local-source verification table for backend
  opcodes, builtins, KIPs, transaction assumptions, and artifact fields.

## Repository Layout

```text
compiler/
  codegen/    Toccata backend and compiled artifact generation
  ir/         Validated AST to KaspaScript IR lowering
  lexer/      Tokenization with line, column, and byte offsets
  parser/     V1 AST parser for contract source
  protocol/   Kaspa target manifests and feature gating
  semantic/   Compiler-front-end validation checks
sdk/          Rust SDK compile and transaction builder API
cli/          kaspascript command-line tool
tests/        Contract pattern corpus
docs/         Source-grounded verification docs
contracts/
  production/ Production-oriented KaspaScript contracts
```

## Test

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
