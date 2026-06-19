contract StateChannel {
  params {
    party_a: PublicKey,
    party_b: PublicKey,
    state_hash: Hash,
    dispute_timeout: BlockHeight,
    finality_depth: 12,
  }

  spend advance(sig_a: Signature, sig_b: Signature, next_state: Bytes) {
    require multisig(2, [party_a, party_b], [sig_a, sig_b]);
    require input_count == 1;
    require output_count == 2;
    require continuation("state", output(0));
    require sha256(next_state) == state_hash;
    require output(0).script == party_a;
    require output(0).value <= input(0).value;
    require output(1).script == party_b;
  }

  spend timeout_refund(sig: Signature) {
    require sig.verify(party_a);
    require block.height >= dispute_timeout;
    require input_count == 1;
    require output_count == 1;
    require output(0).script == party_a;
    require output(0).value <= input(0).value;
  }
}
