contract Vault {
  params {
    owner: PublicKey,
    recovery: PublicKey,
    delay: BlockHeight,
    finality_depth: 10,
  }

  spend withdraw(sig: Signature) {
    require sig.verify(owner);
    require covenant_id.depth >= delay;
    require output(0).value >= input(0).value;
    require output(0).covenant_id == covenant_id;
  }

  spend cancel(sig: Signature) {
    require sig.verify(recovery);
    require output(0).script == recovery;
  }
}
