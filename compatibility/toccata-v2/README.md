# Native Toccata SDK Integration

This isolated companion crate constructs real Rusty Kaspa v1 transactions using
the exact v2.1.0 release commit. It preserves native compute budgets, storage
mass, covenant bindings, authorizing input, genesis IDs, and RPC serialization.
It provides no secret-key storage, signing, or broadcast operation. The optional
escrow example connects to a caller-selected node for read-only inspection.

`Tn10Plan` requires explicit inputs, every recipient/change output, an exact
integer-sompi fee, and network identity `kaspa-testnet-10`. The caller must
verify that identity against its node and supply wallet/node-calculated storage
mass. Offline script validation delegates to the actual v2.1.0 script engine.
It does not replace node UTXO, fee-policy, consensus, or DAG acceptance checks.

Run from the repository root:

```bash
cargo test --manifest-path compatibility/toccata-v2/Cargo.toml --locked
cargo test --manifest-path compatibility/toccata-v2/Cargo.toml --features silverscript --locked
cargo clippy --manifest-path compatibility/toccata-v2/Cargo.toml --all-targets -- -D warnings
```

The optional interoperability test compiles source with released Silverscript
v1.0.0 and executes the resulting bytecode in the pinned v2.1.0 native engine.
Both compiler paths have positive and underpayment controls: KaspaScript spend
instantiation and Silverscript ABI dispatch are exercised independently.
The dependency graph intentionally retains Silverscript's own Rusty Kaspa pin;
bytecode is the boundary, not an assumed shared Rust struct layout.

This remains an isolated validation/integration lane. The existing SDK's
`0.15.0` testnet feature is historical and does not gain Toccata support from
this crate. Existing preview/mainnet gates remain in effect.

All amounts, outpoints, scripts, and storage mass in unit tests are offline
fixtures. Test success must not be reported as accepted TN10 transactions or a
funded lifecycle. Live TN10 verification requires an explicitly scoped funded
test wallet, a verified node, accepted transaction IDs, and indexer evidence.

The signed fixture covers genesis, continuation, native exit, modified-payment
rejection, replay rejection in a local UTXO ledger, and deterministic fixture
rollback/reapply. It uses a public deterministic fixture key. The library itself
never handles signing keys. The ledger is a test harness, not a DAG indexer.

## Buyer/Seller Escrow

`escrow::BuyerSellerEscrow` compiles the repository's
`contracts/buyer_seller_escrow.ks` into one P2SH lock containing cooperative
buyer-and-seller release and buyer timeout refund. It has no arbiter or agent
key. Payout, exact input/output counts and fee cap are script constraints.

The optional `rpc-client` feature enables a read-only `escrow` example for
public terms, node inspection and unsigned spend previews. It never reads
wallet secrets, signs, funds or submits. See
[the agreement, risks and commands](../../docs/BUYER_SELLER_ESCROW.md).
Native fixture success is not a live funded TN10 lifecycle or an audit.
