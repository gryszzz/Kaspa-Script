# Kernel Package v0 Schema

Updated: 2026-06-05.

`kaspascript kernel package <contract.ks>` emits one JSON object that combines
the compiled txscript artifact with the KaspaScript kernel package.

The package is meant for wallets, indexers, SDKs, and review tools. It is not a
replacement for node, wallet, or consensus validation.

## Generate A Package

```bash
cargo run -p kaspascript-cli -- kernel package tests/contracts/escrow.ks \
  --target verified-tn12 \
  --output /tmp/escrow.kernel.json \
  --compute-grams 1000 \
  --tx-bytes 400
```

Without `--output`, the CLI writes beside the source with a `.kernel.json`
extension.

## Root Object

```json
{
  "schema_version": "kaspascript.kernel.package.v0",
  "package_target": "verified-tn12",
  "source_snapshots": [],
  "artifact": {},
  "bytecode_hex": "...",
  "bytecode_asm": "...",
  "kernel": {},
  "fee_estimate": {}
}
```

| Field | Type | Notes |
| --- | --- | --- |
| `schema_version` | string | Current value: `kaspascript.kernel.package.v0`. |
| `package_target` | string | Package target selected by the CLI. |
| `source_snapshots` | array<object> | Pinned upstream source snapshots used by the package evidence set. |
| `artifact` | object | Compiler artifact summary. |
| `bytecode_hex` | string | Hex-encoded compiled txscript bytes. |
| `bytecode_asm` | string | Human-readable txscript assembly. |
| `kernel` | object | Wallet, indexer, readiness, blueprint, and fee policy metadata. |
| `fee_estimate` | object | Fee estimate using the selected package assumptions. |

Package targets:

- `verified-tn12`
- `tn10-toccata`
- `toccata-preview`
- `future-mainnet`

## Source Snapshots

```json
{
  "upstream_repo": "https://github.com/kaspanet/rusty-kaspa",
  "tag": "v1.3.0-toc.5",
  "commit": "04b0d135f8c8023676ea74dcf496c99d5d0bc2a5",
  "audit_date": "2026-06-05"
}
```

The v0 package pins Rusty Kaspa snapshots for `v1.3.0-toc.5` and `tn10-toc3`.
Consumers should treat these as evidence metadata, not as a replacement for
node validation.

## Artifact Summary

```json
{
  "backend": "kaspa-txscript",
  "target": "verified-tn12",
  "compiler_version": "0.1.0",
  "bytecode_bytes": 76,
  "finality_depth": 10,
  "kip_requirements": [10],
  "contracts": ["Escrow"],
  "spends": ["Escrow.release", "Escrow.refund"]
}
```

| Field | Type | Notes |
| --- | --- | --- |
| `backend` | string | Current value: `kaspa-txscript`. |
| `target` | string | Compiler target label such as `verified-tn12`. |
| `compiler_version` | string | Compiler crate version. |
| `bytecode_bytes` | number | Compiled bytecode length in bytes. |
| `finality_depth` | number or null | Contract finality depth if declared. |
| `kip_requirements` | array<number> | KIPs required by the compiled bytecode or guarded features. |
| `contracts` | array<string> | Contract names found in the artifact. |
| `spends` | array<string> | Fully qualified spend path names. |

## Kernel Object

```json
{
  "schema_version": "kaspascript.kernel.package.v0",
  "blueprint": {},
  "readiness": {},
  "wallet_previews": [],
  "indexer_schema": {},
  "fee_policy": {
    "sompi_per_unit": 100
  }
}
```

| Field | Type | Notes |
| --- | --- | --- |
| `schema_version` | string | Current value: `kaspascript.kernel.package.v0`. |
| `blueprint` | object | Contract state-machine model. |
| `readiness` | object | Evidence-based readiness report. |
| `wallet_previews` | array<object> | Wallet-facing transition previews. |
| `indexer_schema` | object | Suggested indexer tables and columns. |
| `fee_policy.sompi_per_unit` | number | Toccata pre-activation minimum fee unit. |

## Blueprint

```json
{
  "name": "Escrow",
  "network": "Tn12",
  "state": [],
  "transitions": [],
  "evidence": []
}
```

| Field | Type | Notes |
| --- | --- | --- |
| `name` | string | Contract or package name. |
| `network` | enum string | `Mainnet`, `Tn10`, `Tn12`, `Simnet`, `Devnet`, or `Unknown`. |
| `state` | array<object> | State fields derived from contract params or blueprint state. |
| `transitions` | array<object> | Spend/state transition definitions. |
| `evidence` | array<object> | Source evidence attached to package claims. |

State field:

```json
{
  "name": "buyer",
  "ty": "PublicKey",
  "description": "compiled parameter from contract Escrow"
}
```

Current `ty` values are `PublicKey`, `Signature`, `Sompi`, `DaaScore`,
`BlockHeight`, `CovenantId`, `Hash`, `Bool`, `U64`, and `Bytes`.

Transition:

```json
{
  "name": "release",
  "kind": "Spend",
  "consumes": ["Escrow compiled locking state"],
  "creates": ["transaction outputs selected by the spend path"],
  "signers": ["sig_a", "sig_b"],
  "requirements": [],
  "proof": {
    "verifier": "None",
    "public_inputs": [],
    "payload_hint": "no proof payload"
  },
  "wallet_warnings": []
}
```

Current `kind` values are `Deposit`, `Spend`, `Timeout`, `Recover`, `Close`,
or `{"Custom": "label"}`.

Requirement:

```json
{
  "feature": "BaseScript",
  "minimum_evidence": "BranchCode",
  "reason": "compiled artifact emitted verified Kaspa txscript bytecode"
}
```

Current `feature` values are `BaseScript`, `TransactionIntrospection`,
`CovenantIds`, `SequencingCommitments`, `ZkVerification`, `FeePolicy`,
`WalletPreview`, and `IndexerLineage`.

Current evidence levels are `Unknown`, `ResearchSignal`, `DocsSignal`,
`BranchCode`, `MergedKip`, `MergedCode`, `TestnetActivation`,
`MainnetPreActivation`, and `MainnetActivation`.

Evidence:

```json
{
  "label": "KaspaScript compiled artifact",
  "url": "tests/contracts/escrow.ks",
  "audit_date": "2026-06-04T03:33:39Z",
  "network": "Tn12",
  "level": "BranchCode",
  "features": ["BaseScript", "WalletPreview", "IndexerLineage"],
  "note": "local compiler artifact verified before kernel package emission"
}
```

## Wallet Previews

```json
{
  "contract": "Escrow",
  "transition": "release",
  "network": "Tn12",
  "classification": "CovenantStateTransition",
  "consumes": ["Escrow compiled locking state"],
  "creates": ["transaction outputs selected by the spend path"],
  "signers": ["sig_a", "sig_b"],
  "proof": {
    "verifier": "None",
    "public_inputs": [],
    "payload_hint": "no proof payload"
  },
  "warnings": []
}
```

Current `classification` values are `OrdinaryPayment`,
`CovenantStateTransition`, and `ProofBearingTransition`.

Current proof verifier values are `None`, `Groth16`, `Risc0Succinct`, or
`{"External": "label"}`.

Wallets should treat `warnings` as sign-time copy and should not render
`CovenantStateTransition` as a plain payment.

## Indexer Schema

```json
{
  "contract": "Escrow",
  "network": "Tn12",
  "tables": [
    {
      "name": "covenant_lineage",
      "columns": [
        {
          "name": "covenant_id",
          "ty": "bytes32",
          "required": true
        }
      ]
    }
  ]
}
```

Column `ty` is a descriptive storage type. The current package emits covenant
lineage, covenant transition, and wallet preview audit table suggestions.

## Readiness Report

```json
{
  "contract": "Escrow",
  "network": "Tn12",
  "level": "verified",
  "ready": true,
  "blockers": [],
  "features": []
}
```

Readiness levels:

- `verified`: every transition requirement is satisfied for a non-preview
  target network.
- `preview`: evidence is sufficient for analysis, but the package target is
  intentionally preview-scoped.
- `blocked`: at least one blocker prevents the package from being treated as
  ready.

Feature readiness line:

```json
{
  "transition": "release",
  "feature": "BaseScript",
  "required": "BranchCode",
  "best": "BranchCode",
  "level": "verified",
  "satisfied": true,
  "source_label": "KaspaScript compiled artifact"
}
```

`ready` is true only when every transition requirement is satisfied by the
available evidence for the package network. Mainnet packages remain blocked
until `MainnetActivation` evidence exists for required mainnet features.

## Fee Estimate

```json
{
  "policy": "toccata-rpc-minimum-standard-fee",
  "source": "https://github.com/kaspanet/rusty-kaspa/releases/tag/v1.3.0-toc.5",
  "compute_grams": 1000,
  "transaction_bytes": 400,
  "minimum_standard_fee_sompi": 100000,
  "assumption": "caller-provided fee estimate inputs"
}
```

The current formula is:

```text
100 sompi * max(compute_grams, 2 * transaction_bytes)
```

When `--tx-bytes` is omitted, the CLI uses compiled bytecode length as a lower
bound for `transaction_bytes`. When `--compute-grams` is omitted, it uses `0`.

## Compatibility Notes

- Enum strings are serialized as Rust variant names, for example `Tn12` and
  `BaseScript`.
- Readiness levels are serialized as lowercase v0 labels: `verified`,
  `preview`, and `blocked`.
- The schema is additive while the CLI is pre-1.0. Consumers should ignore
  unknown fields and require the root fields listed above.
- Machine-readable JSON Schema should be generated from this v0 shape next.
