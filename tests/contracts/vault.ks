contract Vault {
  params {
    owner: PublicKey,
    recovery: PublicKey,
    unlock_height: BlockHeight,
    finality_depth: 10,
  }

  spend withdraw(sig: Signature) {
    require sig.verify(owner);
    require block.height >= unlock_height;
    require output(0).value >= input(0).value;
    require output(0).script == owner;
  }

  spend cancel(sig: Signature) {
    require sig.verify(recovery);
    require output(0).script == recovery;
  }
}
