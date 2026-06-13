# ADR-001: Canonical Application Model

## Status

Accepted on 2026-06-13.

## Context

KaspaScript already had a checked AST, opcode IR, compiler artifacts, kernel
blueprints, wallet previews, capability profiles, and indexer schemas. These
surfaces described overlapping ideas with different structures. Constraint
meaning was especially easy to lose: kernel packages could identify a spend
path but often reduced its outputs to generic prose.

The project needs one stable representation that explains a program between
source semantics and backend bytecode without coupling wallet and indexer
integrations to parser internals.

## Options Considered

| Option | Benefit | Cost |
| --- | --- | --- |
| Infer behavior from emitted instructions in each consumer | No new crate | Loses source structure and repeats fragile inference. |
| Put the model inside the kernel | Fits packaging work | Makes compiler artifacts and SDKs depend on a higher-level package layer. |
| Put the model inside opcode IR | Few files | Couples public application contracts to compiler implementation details. |
| Use a small shared model crate | One vocabulary and dependency direction | Adds a versioned public schema that must be maintained. |

## Decision

Create `kaspascript-model` as a dependency-light shared crate.

Semantic analysis still validates source. IR lowering creates both opcode IR
and the canonical application model from the checked AST. Codegen embeds the
model unchanged in deterministic artifacts. Kernel, CLI, and SDK surfaces
consume that same serialized representation.

## Trade-offs

- Compiler artifacts are larger because they preserve source-level intent.
- The v0 schema becomes a compatibility surface.
- Some normalized expressions remain intentionally generic until the language
  gains richer transaction-shape syntax.

These costs are accepted because explicit, inspectable behavior is more
important than minimizing artifact size.

## Consequences

- Wallets and agents can inspect signatures, constraints, output bindings, and
  monetary responsibilities without decoding bytecode.
- Kernel previews no longer need to invent generic transition descriptions.
- Future covenant and sequencing constructs can be modeled before backend
  activation without being mislabeled as deployable.
- Duplicate semantic inference in the CLI, SDK, and indexer integrations should
  be treated as an architectural regression.

## Revisit Triggers

- The language gains explicit transaction templates, named UTXO state, or
  contract composition.
- Multiple backends need backend-specific application semantics.
- The v0 normalized expression representation cannot describe a new source
  construct without ambiguity.
