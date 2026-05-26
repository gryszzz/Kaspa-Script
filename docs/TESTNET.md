# Testnet Harness

KaspaScript includes a feature-gated live testnet harness for `tn10`, `tn11`,
and `tn12`. Offline tests never require a node. Live tests are ignored by
default and require explicit environment configuration.

See also:

- [TRANSACTION_BUILDER.md](TRANSACTION_BUILDER.md)
- [TESTNET_PROOFS.md](TESTNET_PROOFS.md)

## Status

| Surface | Status |
|---|---|
| Testnet wRPC connection | Implemented |
| Node network check | Hard-fails unless the node reports the selected target |
| UTXO index / balance / fee estimate | Implemented where the node exposes it |
| Key loading and testnet address derivation | Implemented |
| Contract ABI instantiation | Implemented |
| Lock transaction build/sign/validate | Implemented with Rusty Kaspa APIs |
| Spend transaction build/sign/validate | Implemented for signature-only spend args |
| Broadcast | Explicit only: `--broadcast` or `KASPA_BROADCAST=true` |
| Mainnet | Rejected |
| Hidden fee injection | Removed / unsupported |
| Confirmation/finality by txid | Implemented through UTXO-index polling and DAA-depth checks |

Contracts that declare `finality_depth` wait for the lock output to appear in
the indexed UTXO set and for its observed DAA-depth to reach the declared
window before the spend transaction is submitted.

TN12 mass parameters are target-gated in Rusty Kaspa crate `0.15.0`: network IDs
support arbitrary testnet suffixes, but wallet network params are suffix-specific
for TN10/TN11. The builder records a warning for TN12 and lets the node enforce
final acceptance.

## Offline

```bash
cargo test --workspace
cargo test --workspace --features testnet-integration
```

The legacy feature alias still works:

```bash
cargo test --workspace --features tn12-integration
```

## Live Environment

```bash
export KASPA_TARGET=tn12
export KASPA_RPC_URL=ws://127.0.0.1:17210
export KASPA_TESTNET_PRIVATE_KEY=<32-byte-hex-testnet-key>
export KASPA_TESTNET_ADDRESS=<derived-kaspatest-address>
```

Dry-run is default. Broadcast requires:

```bash
export KASPA_BROADCAST=true
```

## CLI Flow

```bash
cargo run -p kaspascript-cli --features testnet-integration -- \
  wallet balance --target tn12 --rpc-url ws://127.0.0.1:17210

cargo run -p kaspascript-cli --features testnet-integration -- \
  tx lock tests/contracts/timelock.ks --target tn12 --amount 0.01 --dry-run

cargo run -p kaspascript-cli --features testnet-integration -- \
  tx lock tests/contracts/timelock.ks --target tn12 --amount 0.01 --broadcast

cargo run -p kaspascript-cli --features testnet-integration -- \
  proof verify tests/proofs/tn12/timelock.proof.json
```

## Ignored Live Tests

```bash
cargo test --workspace --features testnet-integration -- --ignored --nocapture
```

Proof files are written to:

```text
tests/proofs/<target>/<contract>.proof.json
```
