# Kaspa Programmability Kernel

Prepared: 2026-06-04.

KaspaScript is growing from a compiler into a Kaspa-native programmability
kernel. The kernel treats contracts as UTXO state machines: a contract consumes
stateful outputs, validates a transition, and creates successor outputs that
preserve lineage.

This is smart-contract-like, but it is not an Ethereum-style global account
runtime. The kernel is designed around Kaspa's BlockDAG and Toccata-era
building blocks.

## Kernel Contract Model

A kernel contract package contains:

- contract state fields
- transition definitions
- source evidence and target network posture
- wallet previews
- covenant lineage indexer schema
- fee-policy math
- readiness report

The first crate is `kaspascript-kernel`.

```rust
use kaspascript_kernel::{dagsafe_vault_blueprint, ToccataFeePolicy};

let package = dagsafe_vault_blueprint().package()?;
let release_preview = package
    .wallet_previews
    .iter()
    .find(|preview| preview.transition == "release_after_unlock");

let minimum_fee = ToccataFeePolicy::default()
    .minimum_standard_fee(1_000, 400)?;
```

CLI package command:

```console
$ kaspascript kernel package tests/contracts/escrow.ks \
    --compute-grams 1000 \
    --tx-bytes 400
tests/contracts/escrow.kernel.json
```

The output JSON includes:

- compiled artifact summary
- bytecode hex and ASM
- wallet previews
- covenant lineage indexer schema
- readiness report
- Toccata fee estimate and explicit fee assumptions

See [`KERNEL_PACKAGE_SCHEMA.md`](KERNEL_PACKAGE_SCHEMA.md) for the current
emitted JSON shape.

## Why This Is A Kernel

The compiler answers: "Can this source become deterministic Kaspa txscript?"

The kernel answers: "Can a builder safely ship this Kaspa app shape?"

That second question needs more than bytecode:

- Wallets must show state transitions instead of ordinary payment copy.
- Indexers must track covenant IDs, genesis outputs, continuations,
  authorizing inputs, accepted DAA context, and reorg state.
- Fee estimation must follow Toccata policy instead of stale fixed-fee
  assumptions.
- Mainnet claims must remain locked until final activation evidence is pinned.

## First Flagship Blueprint

`DAGSafeVault` is the first kernel blueprint.

State:

- `owner`
- `recovery_key`
- `unlock_daa`
- `covenant_id`
- `policy_hash`

Transitions:

- `deposit`: creates a successor covenant output.
- `release_after_unlock`: consumes the vault and creates an owner output.
- `emergency_recover`: changes control through a recovery path.

The package emits wallet previews and a covenant lineage schema before any
production bytecode lowering is claimed.

## Evidence Posture

The bundled kernel evidence is pinned to the June 4, 2026 source audit:

- `v1.3.0-toc.5` is mainnet pre-activation evidence only. It does not activate
  Toccata on mainnet.
- PR #1000 is merged Toccata implementation evidence.
- `tn10-toc3` is TN10 ZK hardening activation evidence.
- KIP-17, KIP-20, and KIP-21 merged files indicate TN10 activation status for
  the relevant covenant and sequencing surfaces.

Mainnet blueprints remain blocked until a final mainnet activation release,
activation schedule, and support posture are pinned.

## Next Kernel Upgrades

1. Add kernel package golden snapshots for `escrow.ks` and `vault.ks`.
2. Add target selection to `kaspascript kernel package`.
3. Add Toccata git-tag compatibility fixtures to validate Kaspa crate APIs.
4. Add wallet-preview golden tests for each production contract pattern.
5. Add indexer fixtures for covenant genesis, continuation, reorg, and
   wrong-network cases.
6. Feed real node/RPC fee estimates into the package when available.

See [`PROJECT_STATUS.md`](PROJECT_STATUS.md) for the completion roadmap.
