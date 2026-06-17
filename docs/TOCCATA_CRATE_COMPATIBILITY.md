# Toccata Crate Compatibility Spike

Prepared: 2026-06-05. Updated: 2026-06-17.

This spike checks whether the Kaspa crates used by the SDK can move from the
published `0.15.0` crates.io line toward the tagged Toccata `v2.0.x` line.

## Local SDK Crates

`sdk/Cargo.toml` currently uses these Kaspa crates at `0.15.0`:

- `kaspa-addresses`
- `kaspa-consensus-client`
- `kaspa-consensus-core`
- `kaspa-rpc-core`
- `kaspa-txscript`
- `kaspa-wallet-core`
- `kaspa-wrpc-client`

`cargo search kaspa-txscript --limit 5` was rechecked on 2026-06-05 and still
reports `kaspa-txscript = "0.15.0"` as the published crates.io line.

## Toccata v2.0.1 Target

Rusty Kaspa `v2.0.1` is now the explicit compatibility target:

- upstream tag: `v2.0.1`
- tag commit: `cfafeb4c093fa37a303f1b9f19c58f986b870ce3`
- workspace MSRV observed in the tagged `Cargo.toml`: `1.91.0`
- workspace edition observed in the tagged `Cargo.toml`: `2024`
- workspace package version observed in the tagged `Cargo.toml`: `2.0.1`

Future Toccata compatibility probes should use:

```toml
[dependencies]
kaspa-addresses = { git = "https://github.com/kaspanet/rusty-kaspa.git", tag = "v2.0.1" }
kaspa-consensus-client = { git = "https://github.com/kaspanet/rusty-kaspa.git", tag = "v2.0.1" }
kaspa-consensus-core = { git = "https://github.com/kaspanet/rusty-kaspa.git", tag = "v2.0.1" }
kaspa-rpc-core = { git = "https://github.com/kaspanet/rusty-kaspa.git", tag = "v2.0.1" }
kaspa-txscript = { git = "https://github.com/kaspanet/rusty-kaspa.git", tag = "v2.0.1" }
kaspa-wallet-core = { git = "https://github.com/kaspanet/rusty-kaspa.git", tag = "v2.0.1" }
kaspa-wrpc-client = { git = "https://github.com/kaspanet/rusty-kaspa.git", tag = "v2.0.1" }
```

The repo should introduce this as a non-default compatibility feature or
facade. The global SDK dependencies stay on `0.15.0` until the transaction
builder and RPC JSON model can preserve Toccata v1 transaction fields.

## Historical `v1.3.0-toc.5` Probe

Disposable scratch crate used before the final tag:

```toml
[dependencies]
kaspa-addresses = { git = "https://github.com/kaspanet/rusty-kaspa.git", tag = "v1.3.0-toc.5" }
kaspa-consensus-client = { git = "https://github.com/kaspanet/rusty-kaspa.git", tag = "v1.3.0-toc.5" }
kaspa-consensus-core = { git = "https://github.com/kaspanet/rusty-kaspa.git", tag = "v1.3.0-toc.5" }
kaspa-rpc-core = { git = "https://github.com/kaspanet/rusty-kaspa.git", tag = "v1.3.0-toc.5" }
kaspa-txscript = { git = "https://github.com/kaspanet/rusty-kaspa.git", tag = "v1.3.0-toc.5" }
kaspa-wallet-core = { git = "https://github.com/kaspanet/rusty-kaspa.git", tag = "v1.3.0-toc.5" }
kaspa-wrpc-client = { git = "https://github.com/kaspanet/rusty-kaspa.git", tag = "v1.3.0-toc.5" }
```

Historical results:

- `rustc 1.95.0` is new enough for the Toccata workspace MSRV
  `1.91.0`.
- `cargo metadata --no-deps` resolves the git dependencies from
  `v1.3.0-toc.5`.
- After cleaning generated build artifacts, `cargo check` completed against
  the direct Toccata git crate set.
- An API smoke check importing the same Kaspa modules used by
  `sdk/src/testnet.rs` also passed against `v1.3.0-toc.5`.

The first attempt failed with `No space left on device` when only about
`117MiB` was free. After `cargo clean` freed about `6.7GiB`, the Toccata check
completed successfully.

## Compatibility Risks

- Toccata `kaspa-txscript` pulls in ZK dependencies such as Arkworks and RISC0,
  making the graph much heavier than the current `0.15.0` line.
- The final Toccata workspace uses edition 2024 and version `2.0.x`.
- The SDK currently assumes transaction constructors, mass calculation, script
  validation, RPC conversions, and wallet network params from `0.15.0`; each
  import in `sdk/src/testnet.rs` needs an API check before bumping.
- The `v2.0.1` release is the current upgrade release and `v2.0.0` is the
  baseline activation release, but activation is still scheduled for DAA score
  `474,165,565`; neither tag is proof of activation by itself.

## Moving-Master Watch

The pinned compatibility spike should now anchor to `v2.0.1`, while retaining
`v2.0.0` as the baseline activation comparison point. The older `v1.3.0-toc.5`
probe remains useful only as a historical diff point.

The moving-master lane should be non-blocking and should watch for:

- `tx.mass` to `tx.storage_mass`
- `input.mass` to `input.compute_commit`
- required `storage_mass` in `RpcTransaction` JSON
- wallet generator covenant bindings
- seq-commit state and lane-proof RPCs
- txscript WASM script builder flags
- TN10 reenablement and activation posture

See [`RUSTY_KASPA_UPSTREAM_WATCH.md`](RUSTY_KASPA_UPSTREAM_WATCH.md).

## Next Check

The scratch check used a stable target directory:

```bash
CARGO_TARGET_DIR=/Users/anthonygryszkin/Desktop/kaspa-script/target/toccata-spike \
  cargo check --manifest-path /tmp/kaspa-toccata-compat-spike/Cargo.toml -j 1
```

The next repo change should introduce a Cargo feature such as
`toccata-v2-git-deps` or a small compatibility facade instead of replacing the
current `0.15.0` dependencies globally. The follow-up check should compile
`sdk/src/testnet.rs` itself against the Toccata feature, not only an isolated
API smoke crate.
