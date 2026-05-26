contract Timelock {
  params {
    owner: PublicKey,
    unlock_height: BlockHeight,
  }

  spend claim(sig: Signature) {
    require sig.verify(owner);
    require block.height >= unlock_height;
  }
}
