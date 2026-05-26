# Testnet Proof Files

Live testnet runs write proof JSON under:

```text
tests/proofs/<target>/<contract>.proof.json
```

Example:

```text
tests/proofs/tn12/timelock.proof.json
```

## Required Environment

```bash
export KASPA_TARGET=tn12
export KASPA_RPC_URL=ws://127.0.0.1:17210
export KASPA_TESTNET_PRIVATE_KEY=<32-byte-hex-testnet-key>
export KASPA_TESTNET_ADDRESS=<derived-kaspatest-address>
```

Dry-run is default. To broadcast:

```bash
export KASPA_BROADCAST=true
```

The builder refuses mainnet targets and never prints private keys.

## Run Modes

Offline compiler and transaction-template tests:

```bash
cargo test --workspace
cargo test --workspace --features testnet-integration
```

Ignored live testnet flow:

```bash
cargo test --workspace --features testnet-integration -- --ignored --nocapture
```

## Proof Fields

| Field | Meaning |
|---|---|
| `target` | Node-reported network ID, for example `testnet-12` |
| `node_version` | Connected Rusty Kaspa node version |
| `compiler_version` | KaspaScript compiler version |
| `contract_name` | Contract fixture or source contract |
| `source_hash` | SHA-256 of source |
| `artifact_hash` | SHA-256 of compiled artifact JSON |
| `instantiated_params_hash` | SHA-256 of concrete contract params |
| `locking_script_hash` | SHA-256 of P2SH locking script bytes |
| `script_hex` | Redeem script hex |
| `lock_txid` | Real lock transaction ID when broadcast succeeds |
| `spend_txid` | Real spend transaction ID when broadcast succeeds |
| `fee` | Visible fee in sompi |
| `mass` | Rusty Kaspa mass total |
| `valid_spend_result` | Local/script or live result |
| `invalid_spend_rejection_result` | Negative-path proof |
| `warnings` | Target gates and source-grounding notes |

`result: "pass"` requires both `lock_txid` and `spend_txid`.

## Verify A Proof

```bash
cargo run -p kaspascript-cli --features testnet-integration -- \
  proof verify tests/proofs/tn12/timelock.proof.json
```

The verifier checks proof shape locally. Network inclusion must still be checked against the target node or an indexer using the recorded txids.

