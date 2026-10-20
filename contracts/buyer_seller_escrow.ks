contract BuyerSellerEscrow {
  params {
    buyer: PublicKey,
    seller: PublicKey,
    refund_daa_score: BlockHeight,
    max_fee: Amount,
  }

  spend release(buyer_sig: Signature, seller_sig: Signature) {
    require multisig(2, [buyer, seller], [buyer_sig, seller_sig]);
    require input_count == 1;
    require output_count == 1;
    require input(0).value > max_fee;
    require output(0).value >= input(0).value - max_fee;
    require output(0).script == seller;
  }

  spend refund(buyer_sig: Signature) {
    require buyer_sig.verify(buyer);
    require block.height >= refund_daa_score;
    require input_count == 1;
    require output_count == 1;
    require input(0).value > max_fee;
    require output(0).value >= input(0).value - max_fee;
    require output(0).script == buyer;
  }
}
