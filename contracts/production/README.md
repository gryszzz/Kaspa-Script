# Production Contracts

This folder contains KaspaScript contracts intended to exercise real compiler
surface area rather than toy syntax.

## DAGSafeVault.ks

`DAGSafeVault` is a whole-UTXO vault with three explicit spend paths:

- `withdraw`: owner sweep after covenant lineage depth and sequencing finality.
- `recover`: recovery-key emergency sweep with the same finality gate.
- `rotate`: owner-controlled key rotation preserving value and covenant ID.

The contract is included in parser and semantic unit tests so compiler changes
must keep the production example syntactically and semantically valid.
