# KaspaScript

KaspaScript is a Rust compiler architecture for a Kaspa smart-contract language
with testnet/source-gated protocol support. It does not claim mainnet smart
contract support. The verified backend currently emits deterministic Kaspa
txscript bytes only for behavior grounded in pinned Kaspa sources.

## Current Status

Implemented now:

- `compiler/lexer`: position-tagged lexer with keyword, type, literal,
  operator, delimiter, and comment support.
- `compiler/parser`: V1 contract parser and AST for `contract`, `params`,
  `spend`, `require`, calls, member access, arrays, and comparisons.
- `compiler/semantic`: collecting analyzer for scope, type, multisig, finality,
  input/output index, and unsupported-pattern checks.
- `compiler/ir`: validated AST lowering into opcode-agnostic IR.
- `compiler/codegen`: verified TN12 txscript backend, target gates, artifact
  metadata, bytecode verification, hex/ASM disassembly, and golden artifacts.
- `sdk`: Rust compile entry point plus a preview transaction model with
  finality-depth enforcement. It does not build real rusty-kaspa transactions yet.
- `cli`: `compile`, `verify`, and `inspect` commands.
- `tests/contracts`: escrow, timelock, multisig, atomic swap, and verified vault
  contract patterns.
- `tests/golden`: committed artifact JSON, expected hex, and expected ASM for
  every verified contract pattern.
- `docs/kaspa-source-audit.md`: source-grounded protocol audit.

Removed:

- Hidden treasury fee injection. The SDK creates no treasury output and applies
  no invisible fee.

## Source-Grounded Targets

| Target | Behavior |
| --- | --- |
| `verified-tn12` | Default. Emits only opcodes and KIP behavior verified from pinned Kaspa sources. |
| `toccata-preview` | Allows gated future references as artifact warnings, but still fails unsupported bytecode emission. |
| `future-mainnet` | Locked for gated behavior until mainnet sources are pinned. |

Verified protocol evidence currently covers KIP-10 transaction introspection and
base txscript opcodes from `kaspanet/rusty-kaspa`. KIP-15 is verified as a
block-header sequencing commitment, not as a txscript opcode. Covenant IDs, ZK
verification opcodes, and script-level sequencing access are gated or
unsupported until primary Kaspa sources are provided.

## Repository Layout

```text
compiler/
  codegen/    Verified txscript backend, target gates, artifacts
  ir/         AST to KaspaScript IR lowering
  lexer/      Tokenization with line, column, and byte offsets
  parser/     V1 AST parser for contract source
  protocol/   Target manifests and feature gates
  semantic/   Compiler-front-end validation checks
sdk/          Rust SDK compile API and preview transaction model
cli/          kaspascript command-line tool
tests/        Contract corpus, golden artifacts, integration tests
docs/         Source-grounded verification docs
contracts/   Preview contracts and future-gated examples
```

## Test

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
