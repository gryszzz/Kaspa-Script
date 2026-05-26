contract AtomicSwap {
  params {
    receiver: PublicKey,
    refund: PublicKey,
    hash: Hash,
    timeout: BlockHeight,
  }

  spend claim(sig: Signature, secret: Bytes) {
    require sig.verify(receiver);
    require sha256(secret) == hash;
  }

  spend refund_path(sig: Signature) {
    require sig.verify(refund);
    require block.height >= timeout;
  }
}
