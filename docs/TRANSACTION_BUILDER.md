# KaspaScript Transaction Builder

KaspaScript’s testnet transaction builder is feature-gated behind:

```bash
cargo test --features testnet-integration
```

It uses Rusty Kaspa crates for transaction structure, script generation, signature hashing, script validation, RPC submission, and mass/fee calculation. It does not invent transaction formats.

## Source-Grounded APIs

| Area | Rusty Kaspa API Used | Local Source | Status |
|---|---|---|---|
| P2PK/P2SH scripts | `kaspa_txscript::pay_to_address_script`, `pay_to_script_hash_script`, `extract_script_pub_key_address` | `crypto/txscript/src/standard.rs` | Verified |
| Signature hashing | `kaspa_consensus_core::sign::sign_input` | `consensus/core/src/sign.rs` | Verified |
| Script validation | `kaspa_txscript::TxScriptEngine::from_transaction_input(...).execute()` | `crypto/txscript/src/lib.rs` | Verified |
| Sig-op count | `kaspa_txscript::get_sig_op_count` | `crypto/txscript/src/lib.rs` | Verified |
| Transaction structs | `kaspa_consensus_core::tx::{Transaction, TransactionInput, TransactionOutput, UtxoEntry}` | `consensus/core/src/tx.rs` | Verified |
| RPC submit | `kaspa_rpc_core::api::rpc::RpcApi::submit_transaction` with typed `RpcTransaction` | `rpc/core/src/api/rpc.rs`, `rpc/core/src/convert/tx.rs` | Verified |
| Mass / visible fee | `kaspa_wallet_core::tx::mass::MassCalculator` | `wallet/core/src/tx/mass.rs` | Verified |
| TN12 suffix-specific mass params | `kaspa_wallet_core::utxo::NetworkParams::from(NetworkId)` lacks TN12 in crate `0.15.0` | `wallet/core/src/utxo/settings.rs` | Gated |

For TN12, the builder records a warning and uses the closest source-available testnet mass params. The node remains the final accept/reject authority.

## Contract Instantiation

Compiled artifacts now include contract ABI and per-spend IR. The builder:

1. Selects a contract and spend path.
2. Validates contract param types.
3. Replaces contract params with concrete script bytes.
4. Leaves spend arguments out of the redeem script.
5. Supplies spend arguments through the P2SH `signature_script`.
6. Hashes sorted instantiated params into proof metadata.

Public keys are encoded as raw 32-byte Schnorr keys for `OP_CHECKSIG`. When a public key is compared to `output(n).script`, the builder encodes the corresponding Rusty Kaspa P2PK `ScriptPublicKey` bytes.

## Locking Flow

```text
wallet P2PK UTXO
    -> lock transaction
    -> P2SH(contract redeem script)
```

The lock transaction:

- selects wallet UTXOs from the configured testnet address
- creates a P2SH output for the instantiated contract script
- adds visible change output when needed
- calculates mass and minimum relay fee using Rusty Kaspa
- signs funding inputs with Rusty Kaspa sighash/signing logic
- validates scripts locally before broadcast

## Spending Flow

```text
P2SH(contract output) + wallet fee UTXO
    -> spend transaction
    -> wallet P2PK output
```

The spend transaction:

- spends the contract output as input `0`
- uses an extra wallet UTXO for visible fees
- builds the P2SH `signature_script` with spend signatures plus redeem script
- sets CLTV sequence/lock-time when required by the spend path
- signs and validates through the Rusty Kaspa script engine

For artifacts that declare `finality_depth`, live broadcast waits for the lock
output to appear in the indexed UTXO set and then waits until the node's virtual
DAA score gives the output the required observed depth before submitting the
spend transaction.

Non-signature runtime spend arguments are currently rejected in the live builder. Offline compiler support may parse them, but live P2SH execution only supports `Signature` spend args until the language has first-class stack variable lowering.

## CLI

```bash
cargo run -p kaspascript-cli --features testnet-integration -- \
  wallet balance --target tn12 --rpc-url ws://127.0.0.1:17210

cargo run -p kaspascript-cli --features testnet-integration -- \
  tx lock tests/contracts/timelock.ks --target tn12 --amount 0.01 --dry-run

cargo run -p kaspascript-cli --features testnet-integration -- \
  tx lock tests/contracts/timelock.ks --target tn12 --amount 0.01 --broadcast
```

Broadcast is never default. Mainnet is rejected.
