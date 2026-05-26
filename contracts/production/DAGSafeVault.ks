/*
  DAGSafeVault

  Production-oriented KaspaScript V1 vault contract targeting the post-Toccata
  Kaspa execution model.

  Security model:
  - Whole-vault spends only. No partial withdrawal path is exposed because V1
    source should not depend on implicit subtraction or hidden change logic.
  - Owner withdrawal requires covenant lineage depth, finality depth, and a
    sequencing commitment guard before value can leave the covenant.
  - Emergency recovery is explicit and sweeps the entire vault to the recovery
    key. It does not preserve the covenant.
  - Key rotation preserves value and covenant lineage under the same covenant
    ID, avoiding untracked state migration.

  Toccata dependencies:
  - KIP-17 transaction introspection for input/output value and script checks.
  - KIP-20 covenant IDs for lineage continuity.
  - KIP-21 sequencing commitments for DAG-aware ordering gates.
*/

contract DAGSafeVault {
  params {
    owner:          PublicKey,
    recovery:       PublicKey,
    delay:          BlockHeight,
    vault_id:       CovenantID,
    finality_depth: 10,
  }

  /*
    Owner withdrawal path.

    This path intentionally spends the entire vault value to the owner. The
    all-value invariant avoids implicit change handling and makes value
    conservation auditable in source.
  */
  spend withdraw(sig: Signature) {
    require sig.verify(owner);
    require input(0).covenant_id == vault_id;
    require covenant.id.depth >= delay;
    require sequencing.depth >= finality_depth;
    require output(0).value == input(0).value;
    require output(0).script == owner.p2pk();
  }

  /*
    Emergency recovery path.

    The recovery key can sweep the entire vault value. This is deliberately a
    separate spend path so recovery authority is visible and reviewable.
  */
  spend recover(sig: Signature) {
    require sig.verify(recovery);
    require input(0).covenant_id == vault_id;
    require sequencing.depth >= finality_depth;
    require output(0).value == input(0).value;
    require output(0).script == recovery.p2pk();
  }

  /*
    Key rotation path.

    Owner rotates both owner and recovery keys while preserving all value and
    the same covenant ID. This keeps state threaded through KIP-20 lineage
    instead of silently migrating to an unrelated script.
  */
  spend rotate(
    sig: Signature,
    next_owner: PublicKey,
    next_recovery: PublicKey
  ) {
    require sig.verify(owner);
    require input(0).covenant_id == vault_id;
    require sequencing.depth >= finality_depth;
    require output(0).value == input(0).value;
    require output(0).covenant_id == vault_id;
    require output(0).script == covenant.with_keys(next_owner, next_recovery);
  }
}
