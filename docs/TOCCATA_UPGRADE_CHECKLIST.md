# Toccata Upgrade Checklist

Updated: 2026-06-19.

This checklist is for operators, app builders, wallet/indexer integrators, and
KaspaScript maintainers preparing for the Toccata activation window. It is not
proof that mainnet activation has occurred.

## Current Status

Primary source posture:

- Rusty Kaspa `v2.0.1` is the current mainnet Toccata maintenance release:
  https://github.com/kaspanet/rusty-kaspa/releases/tag/v2.0.1
- Rusty Kaspa `v2.0.0` is the baseline release that scheduled mainnet
  activation at DAA score `474,165,565`, roughly 2026-06-30 16:15 UTC:
  https://github.com/kaspanet/rusty-kaspa/releases/tag/v2.0.0
- The tagged Toccata guide is the node/operator guide used by this repo:
  https://github.com/kaspanet/rusty-kaspa/blob/v2.0.1/docs/toccata-guide.md

KaspaScript posture:

- Treat Toccata as scheduled, not live.
- Keep `future-mainnet` blocked until activation is independently verified
  from primary sources.
- Use `verified-tn12` for deterministic V1 artifact tests.
- Use `tn10-toccata` and `toccata-preview` for compatibility analysis and
  readiness work, not as production mainnet claims.

## Operator Checklist

Before upgrading:

- Record current node version, network, data directory path, RPC ports, service
  unit, and rollback/resync plan.
- Back up operator-owned configuration and wallet/key material. Do not rely on
  a downgraded node database as the rollback path.
- Confirm hardware headroom against the Toccata guide. This repo's status
  report records minimum `8` CPU cores, `16 GB` RAM, `640 GB SSD`, and about
  `80 Mbit/s`; preferred capacity is higher.
- Rehearse on Testnet-10 or a disposable node before touching production
  infrastructure.
- Upgrade mainnet nodes to Rusty Kaspa `v2.0.1` before the activation window.
- Plan for P2P protocol version `10`: release notes say updated nodes connect
  only to new-version peers starting 24 hours before activation.

After upgrading:

- Verify the node reports the expected Rusty Kaspa version.
- Verify sync progress, peer count, RPC health, pruning/indexer settings, and
  mempool behavior.
- Keep logs and metrics around the activation DAA score.
- For miners/pools, verify block-template to submit-block flows preserve
  transaction v1 fields, covenant data, and input `compute_commit`.
- For wallets/indexers, verify `storage_mass` / `storageMass` handling and
  avoid building new integrations around deprecated `mass` naming.

## App And Wallet Prep

- Do not assume old fixed-fee logic. Toccata standard fee policy is
  `100 sompi * max(compute grams, 2 * transaction bytes)`.
- Prefer node-provided fee estimation where available.
- Show every unconstrained input, output, fee, change output, recipient, and
  signing request before signing.
- Treat covenant IDs, ZK verification, and sequencing proofs as gated until
  the builder, wallet, indexer, and proof fixtures are present.

## KaspaScript Prep Commands

Run the local health gates:

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Inspect the pinned Toccata posture:

```bash
cargo run -p kaspascript-cli -- toccata status --json
cargo run -p kaspascript-cli -- toccata targets --json
cargo run -p kaspascript-cli -- toccata fee \
  --compute-grams 1000 \
  --tx-bytes 400 \
  --json
```

Check a contract package before handing it to wallets or indexers:

```bash
cargo run -p kaspascript-cli -- kernel check \
  tests/contracts/state_channel.ks \
  --target verified-tn12 \
  --compute-grams 1000 \
  --tx-bytes 400 \
  --json

cargo run -p kaspascript-cli -- kernel preview \
  tests/contracts/state_channel.ks \
  --target verified-tn12 \
  --transition advance \
  --json

cargo run -p kaspascript-cli -- kernel package \
  tests/contracts/state_channel.ks \
  --target verified-tn12 \
  --compute-grams 1000 \
  --tx-bytes 400
```

Confirm production mainnet remains guarded:

```bash
cargo run -p kaspascript-cli -- doctor \
  tests/contracts/state_channel.ks \
  --target future-mainnet \
  --json
```

Expected result before activation evidence: `future-mainnet` stays blocked.

## Activation-Day Checks

Before changing KaspaScript mainnet posture:

- Verify activation at DAA score `474,165,565` from primary network/node
  sources, not from the existence of a release tag alone.
- Verify the current Rusty Kaspa release/tag and any emergency follow-up
  release.
- Re-run local compiler/kernel/SDK gates.
- Re-run wallet-preview and indexer fixture checks for production contract
  patterns.
- Confirm final fee/mass behavior against upgraded nodes.
- Update `docs/kaspa-source-audit.md`, `docs/PROJECT_STATUS.md`, and the CLI
  Toccata status report before changing `future-mainnet` posture.
