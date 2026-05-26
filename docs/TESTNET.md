# TN12 Testnet Harness

KaspaScript includes an ignored, feature-gated TN12 harness for live network
checks. It is designed to prove what is real today and gate what is not.

Current status:

| Surface | Status |
| --- | --- |
| TN12 wRPC connection | Implemented |
| Node network check | Hard-fails unless the node reports `testnet-12` |
| UTXO index queries | Implemented |
| Balance queries | Implemented |
| Fee estimate query | Implemented when supported by the node |
| Testnet key loading and address derivation | Implemented |
| Schnorr digest signing | Implemented |
| Contract compile to artifact/script proof | Implemented |
| Locking transaction broadcast | Gated |
| Spend-path broadcast | Gated |
| Confirmation/finality by txid | Gated until accepted-transaction indexing is wired |

No hidden fee injection exists in the SDK. The testnet harness never broadcasts
to mainnet, never accepts a non-TN12 node, and never prints private keys.

---

## 1. Offline Baseline

These commands must pass without a live node:

```console
$ cargo test --workspace
```

Feature-gated TN12 code can be compiled without running live tests:

```console
$ cargo test --features tn12-integration --no-run
```

The Rusty Kaspa 0.15 RPC crates require Rust 1.81+ for the TN12 feature path.
The core compiler remains offline and does not require live network access.

---

## 2. Run or Configure a TN12 Node

Use a Rusty Kaspa node with:

- Testnet enabled
- Network suffix `12`
- UTXO index enabled
- wRPC Borsh listener enabled

From a Rusty Kaspa checkout:

```console
$ cargo run --release --bin kaspad -- \
    --testnet \
    --netsuffix=12 \
    --utxoindex \
    --rpclisten-borsh=default
```

Rusty Kaspa's default Borsh wRPC port for testnet is `17210`, so the local URL
is usually:

```console
ws://127.0.0.1:17210
```

You can also point the harness at any trusted TN12 wRPC endpoint:

```console
$ export KASPA_TN12_RPC_URL=ws://127.0.0.1:17210
```

The harness reads the node-reported network ID and refuses anything other than:

```text
testnet-12
```

---

## 3. Configure a Test Wallet

Provide a 32-byte testnet private key in lowercase or uppercase hex:

```console
$ export KASPA_TN12_PRIVATE_KEY=<32-byte-hex-private-key>
```

Optional:

```console
$ export KASPA_TN12_FAUCET_ADDRESS=kaspatest:<address>
```

To derive and display the testnet address without printing the private key:

```console
$ cargo test -p kaspascript-sdk \
    --features tn12-integration \
    tn12_rpc_wallet_preflight \
    -- --ignored --nocapture
```

The test prints:

```text
TN12 wallet address: kaspatest:...
TN12 wallet key fingerprint: <sha256-prefix>
TN12 wallet balance: <sompi> sompi
```

Fund the printed `kaspatest:` address from a TN12 faucet or another funded
testnet wallet. Do not use a mainnet key.

---

## 4. Compile Contracts

The normal compiler path produces deterministic artifact JSON:

```console
$ cargo run -p kaspascript-cli -- compile tests/contracts/escrow.ks
```

Inspect the generated artifact:

```console
$ cargo run -p kaspascript-cli -- verify tests/contracts/escrow.artifact.json
```

The testnet harness also compiles all contract fixtures offline:

```console
$ cargo test -p kaspascript-sdk \
    --features tn12-integration \
    offline_contract_deployment_plans_are_deterministic
```

Contracts covered:

| Contract | Fixture |
| --- | --- |
| Escrow | `tests/contracts/escrow.ks` |
| Timelock | `tests/contracts/timelock.ks` |
| Multisig | `tests/contracts/multisig.ks` |
| Atomic swap | `tests/contracts/atomic_swap.ks` |
| Vault | `tests/contracts/vault.ks` |

---

## 5. Lock and Spend Flow

Run the live ignored suite:

```console
$ cargo test --features tn12-integration -- --ignored --nocapture
```

Today this performs:

1. Connect to the configured TN12 node.
2. Verify the node reports `testnet-12`.
3. Verify the node has the UTXO index enabled.
4. Load the testnet wallet and derive a `kaspatest:` address.
5. Fetch balance and UTXOs.
6. Query fees when supported.
7. Compile each contract fixture into a deterministic artifact and script hex.
8. Write gated proof files.

Today this does not broadcast lock or spend transactions. The reason is
intentional: the current SDK transaction builder is a preview model, not a
Rusty Kaspa transaction constructor. KaspaScript artifacts still contain
compiler-level script templates; real lock/spend execution requires parameter
instantiation, signature-script construction, transaction mass/fee handling, and
accepted-transaction confirmation tracking.

Until that backend is implemented, the harness returns:

```text
unsupported TN12 operation: contract lock/spend broadcasting is gated
```

That gate is a safety feature. It prevents fake success and prevents accidental
fund movement through an incomplete builder.

---

## 6. Proof Files

The ignored contract suite writes:

```text
tests/proofs/escrow.tn12.proof.json
tests/proofs/timelock.tn12.proof.json
tests/proofs/multisig.tn12.proof.json
tests/proofs/atomic_swap.tn12.proof.json
tests/proofs/vault.tn12.proof.json
```

Proof schema:

```json
{
  "contract_name": "escrow",
  "source_hash": "...",
  "artifact_hash": "...",
  "script_hex": "...",
  "lock_txid": null,
  "spend_txid": null,
  "network": "testnet-12",
  "node_version": "...",
  "timestamp": 1780000000,
  "result": "gated",
  "error": "unsupported TN12 operation: ..."
}
```

When real transaction construction lands, successful proof files must change to:

- `result: "pass"`
- non-null `lock_txid`
- non-null `spend_txid`
- same deterministic source/artifact hashes for the same source
- node-reported `network: "testnet-12"`

---

## 7. Exact Testnet Flow

```console
# 1. Prove offline compiler correctness.
$ cargo test --workspace

# 2. Compile the live TN12 feature path.
$ cargo test --features tn12-integration --no-run

# 3. Point at a TN12 node.
$ export KASPA_TN12_RPC_URL=ws://127.0.0.1:17210

# 4. Load a testnet-only key. Never use a mainnet key.
$ export KASPA_TN12_PRIVATE_KEY=<32-byte-hex-private-key>

# 5. Optional faucet/account label.
$ export KASPA_TN12_FAUCET_ADDRESS=kaspatest:<address>

# 6. Run live TN12 preflight and gated contract proofs.
$ cargo test --features tn12-integration -- --ignored --nocapture

# 7. Inspect proof files.
$ ls tests/proofs/*.tn12.proof.json
$ cat tests/proofs/escrow.tn12.proof.json
```

If any node reports a network other than `testnet-12`, the harness fails before
UTXO queries or transaction submission.
