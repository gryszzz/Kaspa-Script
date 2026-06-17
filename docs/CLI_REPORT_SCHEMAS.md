# CLI Report Schemas

KaspaScript publishes JSON Schema files and golden snapshots for the
agent-facing CLI report payloads. These files are API contracts for agents, CI,
wallets, SDKs, and indexers that consume `--json` output.

The CLI report schemas are versioned independently from the kernel package
schema. A report schema version must not make incompatible shape changes
without a new `schema_version` value.

`kaspascript.cli.toccata.status.v0` is the upgrade intelligence payload for the
Rusty Kaspa `v2.0.x` Toccata line. It currently reports `v2.0.1` as the active
upgrade release and `v2.0.0` as the baseline activation release, plus tagged
guide links, release assets, activation guard, node requirements, Toccata fee
policy, v1 transaction field changes, KIP mapping, and integrator actions.

## Schemas

| CLI command | Schema version | Schema file | Golden snapshot |
| --- | --- | --- | --- |
| `kaspascript toccata status --json` | `kaspascript.cli.toccata.status.v0` | [`docs/schemas/kaspascript.cli.toccata.status.v0.schema.json`](schemas/kaspascript.cli.toccata.status.v0.schema.json) | [`tests/golden/cli/toccata.status.json`](../tests/golden/cli/toccata.status.json) |
| `kaspascript toccata targets --json` | `kaspascript.cli.toccata.targets.v0` | [`docs/schemas/kaspascript.cli.toccata.targets.v0.schema.json`](schemas/kaspascript.cli.toccata.targets.v0.schema.json) | [`tests/golden/cli/toccata.targets.json`](../tests/golden/cli/toccata.targets.json) |
| `kaspascript toccata fee --compute-grams 1000 --tx-bytes 400 --json` | `kaspascript.cli.toccata.fee.v0` | [`docs/schemas/kaspascript.cli.toccata.fee.v0.schema.json`](schemas/kaspascript.cli.toccata.fee.v0.schema.json) | [`tests/golden/cli/toccata.fee.json`](../tests/golden/cli/toccata.fee.json) |
| `kaspascript kernel check tests/contracts/escrow.ks --target verified-tn12 --compute-grams 1000 --tx-bytes 400 --json` | `kaspascript.cli.kernel.check.v0` | [`docs/schemas/kaspascript.cli.kernel.check.v0.schema.json`](schemas/kaspascript.cli.kernel.check.v0.schema.json) | [`tests/golden/cli/kernel.check.escrow.verified-tn12.json`](../tests/golden/cli/kernel.check.escrow.verified-tn12.json) |
| `kaspascript kernel preview tests/contracts/escrow.ks --target verified-tn12 --transition release --json` | `kaspascript.cli.kernel.preview.v0` | [`docs/schemas/kaspascript.cli.kernel.preview.v0.schema.json`](schemas/kaspascript.cli.kernel.preview.v0.schema.json) | [`tests/golden/cli/kernel.preview.escrow.release.verified-tn12.json`](../tests/golden/cli/kernel.preview.escrow.release.verified-tn12.json) |

`kaspascript doctor <contract.ks> --json` is an alias for
`kaspascript kernel check <contract.ks> --json` and uses
`kaspascript.cli.kernel.check.v0`.

## Compatibility Rules

- `schema_version` is required in every report payload.
- Existing required fields cannot be removed within the same schema version.
- Existing field types cannot change within the same schema version.
- Additive optional fields are allowed within the same schema version.
- New incompatible payload shapes must use a new schema version.
- Golden snapshots are tested in CI against the report builders.
- The report schemas are intentionally stricter at the top level than in nested
  kernel-owned metadata, where `kaspascript.kernel.package.v0` is the deeper
  contract.

## Local Checks

```bash
cargo test -p kaspascript-cli cli_report_golden_snapshots_match
cargo test -p kaspascript-cli cli_report_schema_files_are_valid_json
```
