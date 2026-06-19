# KaspaScript Program Model

Updated: 2026-06-13.

## Definition

A KaspaScript program is a deterministic package of related layers:

1. readable `.ks` source
2. checked contract state and transition intent
3. a canonical UTXO application model
4. typed, opcode-agnostic script IR
5. target-gated txscript and artifact metadata
6. wallet, indexer, fee, evidence, and readiness contracts

It is not only a script compiler, transaction builder, covenant state machine,
or application package. It is the common description those components use to
agree on the same program.

The canonical machine-readable layer is
`kaspascript.application.v0`. Its JSON Schema is
[`schemas/kaspascript.application.v0.schema.json`](schemas/kaspascript.application.v0.schema.json).

## Why The Application Model Exists

Opcode IR answers how a checked expression lowers. It does not, by itself,
give a wallet or reviewer a concise answer to:

- which inputs and outputs a transition references
- which keys and signature arguments authorize it
- which value, script, timelock, hash, covenant, proof, or sequencing
  constraints apply
- whether additional inputs or outputs remain unconstrained
- whether successor ownership or covenant lineage is bound
- who must choose fees and change
- what compilation proves and what another system must verify

The application model preserves those answers before backend emission and is
embedded in compiler artifacts and compiled kernel packages.

## Transition Model

Every `spend` becomes a transition containing:

| Field | Meaning |
| --- | --- |
| `arguments` | Typed transition arguments from source. |
| `signing_requirements` | Recognized single-signature or multisig intent. |
| `constraints` | Every source `require`, normalized and classified. |
| `transaction_shape` | Referenced input/output indexes, exact input/output counts, and count limitations. |
| `monetary_policy` | Explicit fee/change ownership and confirmation that the compiler injects no outputs or recipients. |
| `output_bindings` | Output value, script, or covenant fields constrained by source. |
| `continuation` | `unspecified`, `named-output`, `output-script-bound`, or `covenant-lineage-bound`. |

Constraint categories are:

- `authorization`
- `value`
- `script`
- `timelock`
- `hashlock`
- `covenant`
- `sequencing`
- `proof`
- `transaction-shape`
- `generic`

The normalized expression remains inspectable instead of being reduced to a
human-only summary string.

## Transaction Shape Syntax

Exact transaction counts are declared with KIP-10 count builtins:

```kaspascript
require input_count == 1;
require output_count == 2;
```

The application model records those values as `exact_input_count` and
`exact_output_count`, and marks additional inputs or outputs as not permitted
for that transition.

Named continuation outputs are declared with a metadata-bearing require:

```kaspascript
require continuation("state", output(0));
```

The emitted script verifies that the named output index exists. The name is
preserved in `continuation.named_successor_outputs` for wallets, SDKs, and
indexers. Ownership or lineage strength still comes from ordinary source
constraints such as `output(0).script == owner` or
`output(0).covenant_id == covenant_id`.

## Compilation Guarantees

A successful compiler artifact proves:

- the source parsed and passed KaspaScript semantic checks
- lowering into typed IR and application metadata was deterministic
- the selected backend accepted every emitted instruction under its target
  grounding rules
- recognized signing intent and source constraints were preserved for
  inspection
- compilation did not create outputs, recipients, fees, or change

Compilation does not prove that a future transaction will be accepted by a
node. It also does not prove current mainnet activation, wallet compatibility,
indexer correctness, fee sufficiency, UTXO availability, signature validity,
or application-level liveness.

## External Obligations

The application model assigns remaining duties explicitly:

| Actor | Duty |
| --- | --- |
| Application | Instantiate concrete inputs, outputs, arguments, and continuation records that satisfy every constraint. |
| Wallet | Display every recipient, fee, change output, signature request, and unconstrained transaction field before signing. |
| Node | Enforce consensus, standardness, mass, fee policy, and target-network rules. |
| Indexer | Track accepted UTXOs, covenant lineage, duplicate transitions, and reorg effects. |
| Operator | Verify release, KIP, network, and activation claims against current primary sources. |

These obligations are serialized as stable assurance IDs so humans and agents
can distinguish compiler guarantees from integration assumptions.

## Inspection

Human-readable transition explanation:

```bash
cargo run -p kaspascript-cli -- inspect tests/contracts/escrow.ks
```

Machine-readable application model:

```bash
cargo run -p kaspascript-cli -- inspect tests/contracts/escrow.ks --json
```

Inspect a compiled artifact with target context:

```bash
cargo run -p kaspascript-cli -- inspect \
  tests/golden/escrow.artifact.json
```

## Evolution Rules

- The application model may describe recognized future concepts without
  granting backend support.
- Target gates and source evidence decide whether bytecode emission is
  verified, preview-only, or blocked.
- New language constructs must lower into the same model or deliberately
  version it.
- Wallet, SDK, kernel, and indexer surfaces should consume this model instead
  of re-inferring contract meaning from bytecode or prose.
- Additive v0 fields are allowed before 1.0. Semantic changes require a new
  schema version.
