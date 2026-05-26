contract TeamMultisig {
  params {
    key_a: PublicKey,
    key_b: PublicKey,
    key_c: PublicKey,
  }

  spend approve(sig_a: Signature, sig_b: Signature) {
    require multisig(2, [key_a, key_b, key_c], [sig_a, sig_b]);
  }
}
