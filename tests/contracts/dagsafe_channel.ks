contract DAGSafeChannel {
  params {
    depositor: PublicKey,
    counterparty: PublicKey,
    mediator: PublicKey,
    settlement_hash: Hash,
    dispute_timeout: BlockHeight,
    finality_depth: 20,
  }

  spend cooperative_close(sig_a: Signature, sig_b: Signature, settlement: Bytes) {
    require multisig(2, [depositor, counterparty, mediator], [sig_a, sig_b]);
    require sha256(settlement) == settlement_hash;
    require output(0).value >= input(0).value;
  }

  spend timeout_refund(sig: Signature) {
    require sig.verify(depositor);
    require block.height >= dispute_timeout;
    require output(0).script == depositor;
  }

  spend mediated_close(sig_a: Signature, sig_b: Signature) {
    require multisig(2, [depositor, counterparty, mediator], [sig_a, sig_b]);
    require output(0).value >= input(0).value;
  }
}
