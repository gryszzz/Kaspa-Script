# Buyer/Seller Escrow

Status: experimental native implementation; TN10 only; no funds moved.

## Spending Agreement

This example has two participants and no arbiter, administrator, agent key,
platform recipient, or service fee. The buyer and seller must be different
Schnorr public keys. Both participants review the complete script and terms
before funding its P2SH address.

- Release: buyer AND seller authorize one payment to the seller.
- Timeout refund: buyer authorizes one payment back to the buyer once the
  agreed absolute DAA-score deadline has passed.
- Either path has exactly one input and one output. Its explicit miner fee
  comes from the locked amount and cannot exceed the agreed cap.
- Both paths are committed into the SAME redeem script before funding.
- The timeout is a DAA score, not a block height or a guaranteed wall-clock
  time. Script checks alone do not establish node-level finalization.

This agreement cannot determine whether goods were delivered. The buyer can
withhold cooperative release and later take the timeout refund. After the
deadline, a cooperative release and refund can conflict; node/DAG acceptance
determines the accepted spend. There is no automatic scheduler or dispute
resolution. Losing the buyer key can make recovery impossible. If fees exceed
the cap, this intentionally single-input example may remain unspendable until
fees fall. These are terms to review, not production security guarantees.

## Source Boundary

The governing readiness rule is the Kaspa skill's
`docs/kaspa/mainnet-readiness-gate.md`: protocol activation does not prove
application, wallet, or indexer readiness. This work retains the mainnet gate.

Native behavior is pinned to Rusty Kaspa v2.1.0 commit
`01b532e8b553523216471682649693af92f0fd16`:

- [Signature, multisig, lock-time and introspection opcodes](https://github.com/kaspanet/rusty-kaspa/blob/01b532e8b553523216471682649693af92f0fd16/crypto/txscript/src/opcodes/mod.rs)
- [Standard P2SH construction](https://github.com/kaspanet/rusty-kaspa/blob/01b532e8b553523216471682649693af92f0fd16/crypto/txscript/src/standard.rs)
- [Transaction mass calculation](https://github.com/kaspanet/rusty-kaspa/blob/01b532e8b553523216471682649693af92f0fd16/consensus/core/src/mass/mod.rs)
- [DAA/time finalization](https://github.com/kaspanet/rusty-kaspa/blob/01b532e8b553523216471682649693af92f0fd16/consensus/src/processes/transaction_validator/tx_validation_in_header_context.rs)

## Build Checklist

- [x] Add readable `.ks` source with explicit two-party payout constraints.
- [x] Compile both paths into a single native P2SH script.
- [x] Add unsigned spend previews, exact amounts, fee cap and native mass.
- [x] Add read-only RPC checks for TN10, sync, UTXOs and fee estimates.
- [x] Test unauthorized signatures, payout changes, early refund and fee abuse.
- [x] Run workspace and native tests, formatting, Clippy and diff checks.
- [ ] Complete wallet-provider signing and rejection handling.
- [ ] Execute an explicitly approved, funded TN10 lifecycle and capture receipts.
- [ ] Reconcile accepted state and conflicting spends through a DAG-aware indexer.

Only the first six items can be completed without a wallet integration and a
scoped live test. Offline signatures use public fixture keys. They must never
be used for real funding.

## Inspect Without A Wallet

From the repository root, inspect the public OFFLINE fixture terms:

```bash
cargo run --manifest-path compatibility/toccata-v2/Cargo.toml --locked \
  --features rpc-client --example escrow -- \
  compatibility/toccata-v2/fixtures/escrow-terms.offline.json
```

The fixture keys are public, known keys. Never fund the displayed fixture
address. Replace the public-key terms with independently verified participant
keys before a separately scoped test. The inspector does not generate keys.

Add a Borsh wRPC URL to query your node, with no signing or submission:

```bash
cargo run --manifest-path compatibility/toccata-v2/Cargo.toml --locked \
  --features rpc-client --example escrow -- \
  /path/to/public-terms.json ws://127.0.0.1:17210
```

After observing a real funded test outpoint, the read-only tool can produce an
unsigned preview. Supply the outpoint, path, exact fee in sompi and explicit
compute budget:

```text
... /path/to/public-terms.json WRPC_URL TXID:INDEX release FEE_SOMPI COMPUTE_BUDGET
... /path/to/public-terms.json WRPC_URL TXID:INDEX refund FEE_SOMPI COMPUTE_BUDGET
```

The tool verifies node network identity, sync and UTXO-index support, finds the
exact outpoint, calculates storage mass with pinned native TN10 parameters,
and displays the node's fee estimates separately. It does not claim that a
chosen fee meets node policy or that the transaction is accepted. Refund
previews require observed virtual DAA strictly greater than the deadline.
The RPC read sequence is not an atomic snapshot; a provider or wallet must
refresh UTXO/network/fee checks before signing and submission. Remote nodes
are a trust boundary, not independent acceptance evidence.

The optional transport reuses the upstream `kaspa-wrpc-client`; it is
feature-gated to keep network dependencies out of the default offline lane.
There is no hand-written replacement for Kaspa's wire protocol.

## Verification

```bash
cargo test --workspace --features testnet-integration --locked
cargo clippy --workspace --all-targets --features testnet-integration --locked -- -D warnings
cargo test --manifest-path compatibility/toccata-v2/Cargo.toml --locked --all-features
cargo clippy --manifest-path compatibility/toccata-v2/Cargo.toml --locked --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo fmt --manifest-path compatibility/toccata-v2/Cargo.toml -- --check
git diff --check
```

Regression coverage also checks positive script-number sign boundaries. The
first native positive escrow run exposed incorrect signed-number encoding for
the fee cap `128`; the compiler now emits a zero sign byte when needed and
rejects positive values outside the native signed numeric range.

The legacy 0.15.0 SDK now refuses multi-path instantiation instead of silently
locking only one path. Source inspection/compilation is still available, but
this escrow's two-path native builder must be used for its shared P2SH script.

## Checkpoint: 2026-10-20 UTC

- Workspace: 85 tests passed; two live tests remain ignored.
- Native all-features lane: 19 tests passed, including 12 escrow controls.
- Both format and Clippy passes and `git diff --check` passed.
- Read-only inspector compiled and displayed the public fixture terms/script.
- A deliberately unreachable loopback node timed out and exited with no
  signing or submission, as expected. No live TN10 node result is claimed.
- Agent repository: 55 tests and 19 behavioral reference cases passed;
  extracted skill package: 43 tests passed. Both installed skill copies were
  backed up and synchronized with the tested package.

Wallet-provider signing, visual approval UX, a funded live lifecycle and
DAG-aware recovery are still open. No real keys were read, funds moved,
release published, or mainnet readiness claimed. At this pre-integration
checkpoint, changes were local and the remote draft PRs were unchanged.

## Integration Authorization

The user subsequently authorized commits, pushes and merges. These changes
are being integrated through [PR #2](https://github.com/gryszzz/Kaspa-Script/pull/2)
after fresh local checks and GitHub CI. The PR's commit/check/merge history is
the integration receipt. This authorization does not include release
publication, private-key access, signing, funding or transaction submission.
