# Toccata Crate Compatibility Spike

Prepared: 2026-06-04.

This spike checks whether the Kaspa crates used by the SDK can move from the
published `0.15.0` crates.io line toward the Toccata `v1.3.0-toc.5` line.

## Local SDK Crates

`sdk/Cargo.toml` currently uses these Kaspa crates at `0.15.0`:

- `kaspa-addresses`
- `kaspa-consensus-client`
- `kaspa-consensus-core`
- `kaspa-rpc-core`
- `kaspa-txscript`
- `kaspa-wallet-core`
- `kaspa-wrpc-client`

`cargo search kaspa-txscript --limit 5` still reports `kaspa-txscript =
"0.15.0"` as the published crates.io line.

## Toccata Probe

Disposable scratch crate:

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

Results:

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
- The Toccata workspace uses edition 2024 and version `1.3.0-toc.5`.
- The SDK currently assumes transaction constructors, mass calculation, script
  validation, RPC conversions, and wallet network params from `0.15.0`; each
  import in `sdk/src/testnet.rs` needs an API check before bumping.
- The `v1.3.0-toc.5` release is a mainnet pre-activation pre-release, not final
  mainnet activation.

## Moving-Master Watch

The pinned compatibility spike should remain anchored to `v1.3.0-toc.5` until a
new release becomes the explicit target. However, `kaspanet/rusty-kaspa`
`master` was observed 10 commits ahead of `v1.3.0-toc.5` on 2026-06-05.

The moving-master lane should be non-blocking and should watch for:

- `tx.mass` to `tx.storage_mass`
- `input.mass` to `input.compute_commit`
- required `storage_mass` in `RpcTransaction` JSON
- wallet generator covenant bindings
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
`toccata-git-deps` or a small compatibility facade instead of replacing the
current `0.15.0` dependencies globally. The follow-up check should compile
`sdk/src/testnet.rs` itself against the Toccata feature, not only an isolated
API smoke crate.
