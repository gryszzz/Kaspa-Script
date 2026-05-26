# Preview Contracts

This folder contains future-facing KaspaScript contracts intended to exercise
language design. They are not verified TN12 bytecode targets until primary
Kaspa sources define the required covenant, sequencing-script, and ZK surfaces.

## DAGSafeVault.ks

`DAGSafeVault` is a future-gated whole-UTXO vault with three explicit spend
paths:

- `withdraw`: owner sweep after covenant lineage depth and sequencing finality.
- `recover`: recovery-key emergency sweep with the same finality gate.
- `rotate`: owner-controlled key rotation preserving value and covenant ID.

The verified compiler target intentionally rejects its unsupported covenant and
sequencing features. Keep it as a preview design fixture, not a production
deployment artifact.
