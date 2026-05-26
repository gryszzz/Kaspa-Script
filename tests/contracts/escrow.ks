contract Escrow {
  params {
    buyer: PublicKey,
    seller: PublicKey,
    arbiter: PublicKey,
    timeout: BlockHeight,
    finality_depth: 10,
  }

  spend release(sig_a: Signature, sig_b: Signature) {
    require multisig(2, [buyer, seller, arbiter], [sig_a, sig_b]);
    require output(0).value >= input(0).value;
  }

  spend refund(sig: Signature) {
    require sig.verify(buyer);
    require block.height >= timeout;
    require output(0).script == buyer;
  }
}
