use kaspa_consensus_core::{
    Hash,
    constants::LOCK_TIME_THRESHOLD,
    hashing::{
        sighash::{SigHashReusedValuesUnsync, calc_schnorr_signature_hash},
        sighash_type::SIG_HASH_ALL,
    },
    network::{NetworkId, NetworkType},
};
use kaspa_rpc_core::{
    GetServerInfoResponse, RpcFeeEstimate, RpcFeerateBucket, RpcUtxosByAddressesEntry,
};
use kaspa_txscript::{opcodes::codes::OpTrue, pay_to_address_script};
use kaspascript_toccata_native::{escrow::*, *};
use secp256k1::{Keypair, Message, SECP256K1};

// Public deterministic keys for offline fixtures ONLY. Never fund these keys.
fn key(byte: u8) -> Keypair {
    Keypair::from_seckey_slice(SECP256K1, &[byte; 32]).unwrap()
}

fn terms() -> EscrowTerms {
    EscrowTerms {
        buyer_public_key: hex::encode(key(7).x_only_public_key().0.serialize()),
        seller_public_key: hex::encode(key(8).x_only_public_key().0.serialize()),
        refund_daa_score: 128,
        max_fee_sompi: 128,
    }
}

fn escrow() -> BuyerSellerEscrow {
    BuyerSellerEscrow::new(terms()).unwrap()
}

fn locked(escrow: &BuyerSellerEscrow) -> UtxoEntry {
    UtxoEntry::new(
        100_000_000,
        escrow.script_public_key().clone(),
        1,
        false,
        None,
    )
}

fn outpoint() -> TransactionOutpoint {
    TransactionOutpoint::new(Hash::from_u64_word(123), 0)
}

fn unsigned(escrow: &BuyerSellerEscrow, path: EscrowPath) -> NativeTransaction {
    escrow
        .unsigned_spend(TN10_NETWORK, outpoint(), locked(escrow), path, 100, 1000)
        .unwrap()
        .0
}

fn sign(
    native: &mut NativeTransaction,
    escrow: &BuyerSellerEscrow,
    path: EscrowPath,
    keys: &[Keypair],
) {
    let hash = calc_schnorr_signature_hash(
        &PopulatedTransaction::new(&native.transaction, native.entries.clone()),
        0,
        SIG_HASH_ALL,
        &SigHashReusedValuesUnsync::new(),
    );
    let signatures: Vec<Vec<u8>> = keys
        .iter()
        .map(|key| {
            let mut signature = key
                .sign_schnorr(Message::from_digest(hash.as_bytes()))
                .as_ref()
                .to_vec();
            signature.push(SIG_HASH_ALL.to_u8());
            signature
        })
        .collect();
    native.transaction.inputs[0].signature_script =
        escrow.signature_script(path, &signatures).unwrap();
    native.transaction.finalize();
}

#[test]
fn both_paths_execute_from_the_same_p2sh_lock() {
    let escrow = escrow();
    let mut release = unsigned(&escrow, EscrowPath::Release);
    assert!(
        release.validate_scripts().is_err(),
        "unsigned is not authorized"
    );
    sign(
        &mut release,
        &escrow,
        EscrowPath::Release,
        &[key(7), key(8)],
    );
    release.validate_scripts().unwrap();
    let mut refund = unsigned(&escrow, EscrowPath::Refund);
    sign(&mut refund, &escrow, EscrowPath::Refund, &[key(7)]);
    refund.validate_scripts().unwrap();
    assert_eq!(
        release.entries[0].script_public_key,
        refund.entries[0].script_public_key
    );
    assert_ne!(
        release.transaction.outputs[0].script_public_key,
        refund.transaction.outputs[0].script_public_key
    );
    assert!(escrow.address().to_string().starts_with("kaspatest:"));
}

#[test]
fn numeric_fee_cap_boundaries_execute_in_the_native_engine() {
    for cap in [127, 128, 255, 256, 32768, 8388608] {
        let mut t = terms();
        t.max_fee_sompi = cap;
        let escrow = BuyerSellerEscrow::new(t).unwrap();
        let mut native = unsigned(&escrow, EscrowPath::Release);
        sign(&mut native, &escrow, EscrowPath::Release, &[key(7), key(8)]);
        native.validate_scripts().unwrap();
    }
}

#[test]
fn requires_buyer_and_seller_without_an_arbiter_or_agent_key() {
    let escrow = escrow();
    for keys in [
        [key(7), key(7)],
        [key(8), key(8)],
        [key(7), key(9)],
        [key(9), key(8)],
    ] {
        let mut release = unsigned(&escrow, EscrowPath::Release);
        sign(&mut release, &escrow, EscrowPath::Release, &keys);
        assert!(release.validate_scripts().is_err());
    }
    for wrong_key in [key(8), key(9)] {
        let mut refund = unsigned(&escrow, EscrowPath::Refund);
        sign(&mut refund, &escrow, EscrowPath::Refund, &[wrong_key]);
        assert!(refund.validate_scripts().is_err());
    }
    assert!(escrow.signature_script(EscrowPath::Release, &[]).is_err());
    let mut non_all = vec![0; 65];
    non_all[64] = 2;
    assert!(
        escrow
            .signature_script(EscrowPath::Refund, &[non_all])
            .is_err()
    );
}

#[test]
fn native_script_rejects_redirected_payout_even_with_both_signatures() {
    let escrow = escrow();
    for path in [EscrowPath::Release, EscrowPath::Refund] {
        let mut native = unsigned(&escrow, path);
        native.transaction.outputs[0].script_public_key =
            ScriptPublicKey::new(0, vec![OpTrue].into());
        let keys = if path == EscrowPath::Release {
            vec![key(7), key(8)]
        } else {
            vec![key(7)]
        };
        sign(&mut native, &escrow, path, &keys);
        assert!(native.validate_scripts().is_err());
    }
}

#[test]
fn native_script_enforces_fee_cap_and_exact_transaction_shape() {
    let escrow = escrow();
    for path in [EscrowPath::Release, EscrowPath::Refund] {
        let keys = if path == EscrowPath::Release {
            vec![key(7), key(8)]
        } else {
            vec![key(7)]
        };
        let mut excess_fee = unsigned(&escrow, path);
        excess_fee.transaction.outputs[0].value = excess_fee.entries[0].amount - 129;
        sign(&mut excess_fee, &escrow, path, &keys);
        assert!(excess_fee.validate_scripts().is_err());
        let mut extra_output = unsigned(&escrow, path);
        extra_output.transaction.outputs[0].value -= 1;
        extra_output
            .transaction
            .outputs
            .push(TransactionOutput::new(
                1,
                pay_to_address_script(escrow.address()),
            ));
        sign(&mut extra_output, &escrow, path, &keys);
        assert!(extra_output.validate_scripts().is_err());
        let mut extra_input = unsigned(&escrow, path);
        extra_input
            .transaction
            .inputs
            .push(TransactionInput::new_with_compute_budget(
                TransactionOutpoint::new(Hash::from_u64_word(124), 0),
                vec![],
                u64::MAX,
                1000,
            ));
        extra_input.entries.push(UtxoEntry::new(
            1,
            ScriptPublicKey::new(0, vec![OpTrue].into()),
            1,
            false,
            None,
        ));
        sign(&mut extra_input, &escrow, path, &keys);
        assert!(extra_input.validate_scripts().is_err());
    }
}

#[test]
fn refund_enforces_lock_time_and_nonfinal_sequence() {
    let escrow = escrow();
    let mut early = unsigned(&escrow, EscrowPath::Refund);
    early.transaction.lock_time = escrow.terms().refund_daa_score - 1;
    sign(&mut early, &escrow, EscrowPath::Refund, &[key(7)]);
    assert!(early.validate_scripts().is_err());
    let mut final_sequence = unsigned(&escrow, EscrowPath::Refund);
    final_sequence.transaction.inputs[0].sequence = u64::MAX;
    sign(&mut final_sequence, &escrow, EscrowPath::Refund, &[key(7)]);
    assert!(final_sequence.validate_scripts().is_err());
    let mut modified_after_signing = unsigned(&escrow, EscrowPath::Release);
    sign(
        &mut modified_after_signing,
        &escrow,
        EscrowPath::Release,
        &[key(7), key(8)],
    );
    modified_after_signing.transaction.outputs[0].value -= 1;
    modified_after_signing.transaction.finalize();
    assert!(modified_after_signing.validate_scripts().is_err());
}

#[test]
fn terms_reject_ambiguous_keys_deadlines_and_unbounded_fees() {
    let mut t = terms();
    t.seller_public_key = t.buyer_public_key.clone();
    assert!(matches!(
        BuyerSellerEscrow::new(t),
        Err(EscrowError::InvalidKeys)
    ));
    let mut t = terms();
    t.buyer_public_key = "00".repeat(32);
    assert!(matches!(
        BuyerSellerEscrow::new(t),
        Err(EscrowError::InvalidKeys)
    ));
    for deadline in [0, LOCK_TIME_THRESHOLD, u64::MAX] {
        let mut t = terms();
        t.refund_daa_score = deadline;
        assert!(matches!(
            BuyerSellerEscrow::new(t),
            Err(EscrowError::InvalidDeadline)
        ));
    }
    for cap in [0, u64::MAX] {
        let mut t = terms();
        t.max_fee_sompi = cap;
        assert!(matches!(
            BuyerSellerEscrow::new(t),
            Err(EscrowError::InvalidFee)
        ));
    }
    let mut json = serde_json::to_value(terms()).unwrap();
    json["arbiter"] = "not-supported".into();
    assert!(serde_json::from_value::<EscrowTerms>(json).is_err());
}

#[test]
fn unsigned_plan_rejects_wrong_network_utxo_and_fee() {
    let escrow = escrow();
    assert!(
        escrow
            .unsigned_spend(
                "kaspa-mainnet",
                outpoint(),
                locked(&escrow),
                EscrowPath::Release,
                100,
                1000
            )
            .is_err()
    );
    for fee in [0, 129, u64::MAX] {
        assert!(
            escrow
                .unsigned_spend(
                    TN10_NETWORK,
                    outpoint(),
                    locked(&escrow),
                    EscrowPath::Release,
                    fee,
                    1000
                )
                .is_err()
        );
    }
    let mut wrong = locked(&escrow);
    wrong.script_public_key = ScriptPublicKey::new(0, vec![OpTrue].into());
    assert!(
        escrow
            .unsigned_spend(
                TN10_NETWORK,
                outpoint(),
                wrong,
                EscrowPath::Release,
                100,
                1000
            )
            .is_err()
    );
    let mut wrong = locked(&escrow);
    wrong.covenant_id = Some(Hash::from_u64_word(1));
    assert!(
        escrow
            .unsigned_spend(
                TN10_NETWORK,
                outpoint(),
                wrong,
                EscrowPath::Release,
                100,
                1000
            )
            .is_err()
    );
    assert!(
        escrow
            .unsigned_spend(
                TN10_NETWORK,
                outpoint(),
                locked(&escrow),
                EscrowPath::Release,
                100,
                0
            )
            .is_err()
    );
}

#[test]
fn preview_accounts_for_every_sompi_and_never_claims_submission() {
    let escrow = escrow();
    let (native, preview) = escrow
        .unsigned_spend(
            TN10_NETWORK,
            outpoint(),
            locked(&escrow),
            EscrowPath::Release,
            100,
            1000,
        )
        .unwrap();
    assert_eq!(
        preview.input_sompi,
        preview.recipient_sompi + preview.fee_sompi
    );
    assert_eq!(preview.change_sompi, 0);
    assert_eq!(native.transaction.outputs.len(), 1);
    assert_eq!(preview.required_signers.len(), 2);
    assert_eq!(preview.fee_sompi, native.fee);
    assert!(!preview.broadcast);
    assert!(!preview.fee_policy_verified);
    assert!(!preview.node_acceptance_verified);
    assert_eq!(preview.compute_budget, 1000);
    assert_eq!(preview.transaction_version, 1);
    assert_eq!(preview.input_sequence, u64::MAX);
    assert!(preview.estimated_signed_mass > preview.storage_mass);
    assert_eq!(preview.source_hash, escrow.source_hash());
    assert_eq!(
        preview.redeem_script_hex,
        hex::encode(escrow.redeem_script())
    );
    assert_eq!(native.transaction.inputs[0].signature_script.len(), 0);
    assert_eq!(serde_json::to_value(preview).unwrap()["fee_sompi"], 100);
}

fn node_info(daa: u64) -> GetServerInfoResponse {
    GetServerInfoResponse {
        rpc_api_version: 1,
        rpc_api_revision: 0,
        server_version: "offline-fixture-v2.1.0".into(),
        network_id: NetworkId::with_suffix(NetworkType::Testnet, 10),
        has_utxo_index: true,
        is_synced: true,
        virtual_daa_score: daa,
    }
}

fn observation(escrow: &BuyerSellerEscrow, daa: u64) -> EscrowObservation {
    let bucket = RpcFeerateBucket {
        feerate: 1.0,
        estimated_seconds: 1.0,
    };
    EscrowObservation {
        server_info: node_info(daa),
        utxos: vec![RpcUtxosByAddressesEntry {
            address: Some(escrow.address().clone()),
            outpoint: outpoint().into(),
            utxo_entry: locked(escrow).into(),
        }],
        fee_estimate: RpcFeeEstimate {
            priority_bucket: bucket,
            normal_buckets: vec![bucket],
            low_buckets: vec![bucket],
        },
    }
}

#[test]
fn node_gate_rejects_mainnet_wrong_suffix_unsynced_and_unindexed() {
    let mut info = node_info(100);
    info.network_id = NetworkId::new(NetworkType::Mainnet);
    assert!(validate_node(&info).is_err());
    info.network_id = NetworkId::with_suffix(NetworkType::Testnet, 12);
    assert!(validate_node(&info).is_err());
    info = node_info(100);
    info.is_synced = false;
    assert!(validate_node(&info).is_err());
    info = node_info(100);
    info.has_utxo_index = false;
    assert!(validate_node(&info).is_err());
}

#[test]
fn snapshot_requires_unspent_outpoint_and_deadline_strictly_passed() {
    let escrow = escrow();
    for daa in [127, 128] {
        assert!(matches!(
            observation(&escrow, daa).unsigned_spend(
                &escrow,
                outpoint(),
                EscrowPath::Refund,
                100,
                1000
            ),
            Err(EscrowError::RefundNotDue)
        ));
    }
    assert!(
        observation(&escrow, 129)
            .unsigned_spend(&escrow, outpoint(), EscrowPath::Refund, 100, 1000)
            .is_ok()
    );
    let mut observed = observation(&escrow, 129);
    observed.utxos.clear();
    assert!(matches!(
        observed.unsigned_spend(&escrow, outpoint(), EscrowPath::Release, 100, 1000),
        Err(EscrowError::MissingUtxo)
    ));
    let mut observed = observation(&escrow, 129);
    observed.utxos.push(observed.utxos[0].clone());
    assert!(
        observed
            .unsigned_spend(&escrow, outpoint(), EscrowPath::Release, 100, 1000)
            .is_err()
    );
}

#[test]
fn fee_and_deadline_parameters_change_the_committed_lock() {
    let original = escrow();
    let mut changed = terms();
    changed.refund_daa_score += 1;
    assert_ne!(
        original.address(),
        BuyerSellerEscrow::new(changed).unwrap().address()
    );
    let mut changed = terms();
    changed.max_fee_sompi += 1;
    assert_ne!(
        original.address(),
        BuyerSellerEscrow::new(changed).unwrap().address()
    );
    assert_eq!(original.address(), escrow().address());
}
