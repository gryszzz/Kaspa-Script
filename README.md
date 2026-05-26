# KaspaScript

KaspaScript is a Rust compiler foundation for a post-Toccata Kaspa smart
contract language. The current repository contains the first production-grade
compiler slice: lexer, parser, semantic checks, and a real DAG-aware vault
contract example.

## Current Status

Implemented now:

- `compiler/lexer`: position-tagged KaspaScript lexer with keyword, type,
  literal, operator, delimiter, and comment support.
- `compiler/parser`: V1 contract parser and AST for `contract`, `params`,
  `spend`, `require`, calls, member access, arrays, and comparisons.
- `compiler/protocol`: target manifest and feature gates for Kaspa protocol
  surfaces. Bytecode emission is locked until exact consensus opcode
  definitions are pinned.
- `compiler/semantic`: safety checks for duplicate names, finality depth,
  missing spend guards, parameter shadowing, unknown `require` roots, and
  required Kaspa feature extraction.
- `contracts/production/DAGSafeVault.ks`: whole-UTXO vault contract using
  covenant IDs, finality depth, and sequencing commitments.

Not implemented yet:

- IR generation
- Toccata opcode backend
- bytecode serializer
- WASM SDK and TypeScript transaction builder
- CLI

## Repository Layout

```text
compiler/
  lexer/      Tokenization with line, column, and byte offsets
  parser/     V1 AST parser for contract source
  protocol/   Kaspa target manifests and feature gating
  semantic/   Compiler-front-end validation checks
contracts/
  production/ Production-oriented KaspaScript contracts
```

## Test

```bash
cargo test --workspace
```
