use kaspa_consensus_core::Hash;
use kaspa_rpc_core::RpcTransaction;
use kaspa_txscript::opcodes::codes::*;
use kaspa_txscript::script_builder::ScriptBuilder;
use kaspascript_toccata_native::*;

fn true_script() -> ScriptPublicKey {
    ScriptPublicKey::new(0, vec![OpTrue].into())
}

fn plan() -> Tn10Plan {
    Tn10Plan {
        network: TN10_NETWORK.into(),
        inputs: vec![InputPlan {
            outpoint: TransactionOutpoint::new(Hash::from_u64_word(1), 0),
            utxo: UtxoEntry::new(10_000, true_script(), 0, false, None),
            compute_budget: 1000,
            sequence: u64::MAX,
            witness: vec![],
        }],
        outputs: vec![TransactionOutput::new(9_000, true_script())],
        genesis_groups: vec![],
        lock_time: 0,
        storage_mass: 640,
        expected_fee: 1000,
    }
}

#[test]
fn rpc_round_trip_preserves_native_v1_fields_and_genesis_binding() {
    let mut p = plan();
    p.genesis_groups.push(GenesisCovenantGroup::new(0, vec![0]));
    let native = p.build().unwrap();
    native.validate_scripts().unwrap();
    let rpc_json = serde_json::to_value(native.rpc_transaction()).unwrap();
    let rpc: RpcTransaction = serde_json::from_value(rpc_json).unwrap();
    let restored = Transaction::try_from(rpc).unwrap();
    assert_eq!(restored, native.transaction);
    assert_eq!(restored.storage_mass(), 640);
    assert_eq!(
        restored.inputs[0].compute_commit.compute_budget(),
        Some(1000)
    );
    assert_eq!(restored.outputs[0].covenant.unwrap().authorizing_input, 0);
}

#[test]
fn rejects_wrong_network_duplicate_inputs_hidden_fee_and_overflow() {
    let mut p = plan();
    p.network = "kaspa-mainnet".into();
    assert!(matches!(p.build(), Err(PlanError::WrongNetwork)));
    let mut p = plan();
    p.inputs.push(p.inputs[0].clone());
    assert!(matches!(p.build(), Err(PlanError::DuplicateInput)));
    let mut p = plan();
    p.expected_fee = 999;
    assert!(matches!(p.build(), Err(PlanError::FeeMismatch)));
    let mut p = plan();
    p.inputs[0].utxo.amount = u64::MAX;
    let mut extra = p.inputs[0].clone();
    extra.outpoint.index = 1;
    p.inputs.push(extra);
    assert!(matches!(p.build(), Err(PlanError::Overflow)));
}

// An offline introspection fixture, not a wallet authorization policy.
fn continuation_script() -> ScriptPublicKey {
    let script = ScriptBuilder::new()
        .add_i64(0)
        .unwrap()
        .add_op(OpAuthOutputCount)
        .unwrap()
        .add_i64(1)
        .unwrap()
        .add_op(OpEqualVerify)
        .unwrap()
        .add_i64(0)
        .unwrap()
        .add_op(OpTxOutputAmount)
        .unwrap()
        .add_i64(0)
        .unwrap()
        .add_op(OpTxInputAmount)
        .unwrap()
        .add_i64(1000)
        .unwrap()
        .add_op(OpSub)
        .unwrap()
        .add_op(OpGreaterThanOrEqual)
        .unwrap()
        .drain();
    ScriptPublicKey::new(0, script.into())
}

#[test]
fn native_genesis_continuation_replay_and_invalid_binding() {
    let mut genesis = plan();
    genesis.outputs[0].script_public_key = continuation_script();
    genesis
        .genesis_groups
        .push(GenesisCovenantGroup::new(0, vec![0]));
    let genesis = genesis.build().unwrap();
    genesis.validate_scripts().unwrap();
    let output = &genesis.transaction.outputs[0];
    let covenant_id = output.covenant.unwrap().covenant_id;
    let spent_outpoint = TransactionOutpoint::new(genesis.transaction.id(), 0);
    let mut next = plan();
    next.inputs[0].outpoint = spent_outpoint;
    next.inputs[0].utxo = UtxoEntry::new(
        output.value,
        output.script_public_key.clone(),
        0,
        false,
        Some(covenant_id),
    );
    next.outputs[0] = TransactionOutput::with_covenant(
        8_000,
        continuation_script(),
        Some(CovenantBinding::new(0, covenant_id)),
    );
    let continuation = next.build().unwrap();
    continuation.validate_scripts().unwrap();
    assert_eq!(
        continuation.transaction.outputs[0]
            .covenant
            .unwrap()
            .covenant_id,
        covenant_id
    );

    let mut invalid = continuation;
    invalid.transaction.outputs[0].covenant = None;
    assert!(
        invalid.validate_scripts().is_err(),
        "covenant cannot disappear in this fixture"
    );
    invalid.transaction.outputs[0].covenant =
        Some(CovenantBinding::new(0, Hash::from_u64_word(99)));
    assert!(
        invalid.validate_scripts().is_err(),
        "invalid genesis/continuation lineage must fail"
    );
}

fn signed_vault_script(key: &secp256k1::Keypair) -> ScriptPublicKey {
    let public_key = key.x_only_public_key().0.serialize();
    let payout = true_script();
    // Introspection returns the big-endian script version followed by bytecode.
    let payout_spk: Vec<u8> = payout
        .version()
        .to_be_bytes()
        .into_iter()
        .chain(payout.script().iter().copied())
        .collect();
    let script = ScriptBuilder::new()
        .add_data(&public_key)
        .unwrap()
        .add_op(OpCheckSigVerify)
        .unwrap()
        .add_op(OpIf)
        .unwrap()
        .add_i64(0)
        .unwrap()
        .add_op(OpAuthOutputCount)
        .unwrap()
        .add_i64(1)
        .unwrap()
        .add_op(OpEqualVerify)
        .unwrap()
        .add_i64(0)
        .unwrap()
        .add_op(OpTxOutputSpk)
        .unwrap()
        .add_i64(0)
        .unwrap()
        .add_op(OpTxInputSpk)
        .unwrap()
        .add_op(OpEqualVerify)
        .unwrap()
        .add_op(OpElse)
        .unwrap()
        .add_i64(0)
        .unwrap()
        .add_op(OpAuthOutputCount)
        .unwrap()
        .add_i64(0)
        .unwrap()
        .add_op(OpEqualVerify)
        .unwrap()
        .add_i64(0)
        .unwrap()
        .add_op(OpTxOutputSpk)
        .unwrap()
        .add_data(&payout_spk)
        .unwrap()
        .add_op(OpEqualVerify)
        .unwrap()
        .add_op(OpEndIf)
        .unwrap()
        .add_i64(0)
        .unwrap()
        .add_op(OpTxOutputAmount)
        .unwrap()
        .add_i64(0)
        .unwrap()
        .add_op(OpTxInputAmount)
        .unwrap()
        .add_i64(1000)
        .unwrap()
        .add_op(OpSub)
        .unwrap()
        .add_op(OpGreaterThanOrEqual)
        .unwrap()
        .drain();
    ScriptPublicKey::new(0, script.into())
}

fn sign_fixture(native: &mut NativeTransaction, continuation: bool, key: &secp256k1::Keypair) {
    use kaspa_consensus_core::hashing::{
        sighash::{SigHashReusedValuesUnsync, calc_schnorr_signature_hash},
        sighash_type::SIG_HASH_ALL,
    };
    let hash = calc_schnorr_signature_hash(
        &PopulatedTransaction::new(&native.transaction, native.entries.clone()),
        0,
        SIG_HASH_ALL,
        &SigHashReusedValuesUnsync::new(),
    );
    let message = secp256k1::Message::from_digest(hash.as_bytes());
    let signature = key.sign_schnorr(message);
    let mut bytes = signature.as_ref().to_vec();
    bytes.push(SIG_HASH_ALL.to_u8());
    native.transaction.inputs[0].signature_script = ScriptBuilder::new()
        .add_i64(i64::from(continuation))
        .unwrap()
        .add_data(&bytes)
        .unwrap()
        .drain();
    native.transaction.finalize();
}

// This ledger models only fixture UTXO consumption. It is not a node or indexer.
fn apply_fixture(
    ledger: &mut std::collections::HashMap<TransactionOutpoint, UtxoEntry>,
    native: &NativeTransaction,
) -> Result<(), String> {
    for (input, expected) in native.transaction.inputs.iter().zip(&native.entries) {
        if ledger.get(&input.previous_outpoint) != Some(expected) {
            return Err("spent or mismatched fixture input".into());
        }
    }
    native
        .validate_scripts()
        .map_err(|error| error.to_string())?;
    for input in &native.transaction.inputs {
        ledger.remove(&input.previous_outpoint);
    }
    for (index, output) in native.transaction.outputs.iter().enumerate() {
        ledger.insert(
            TransactionOutpoint::new(native.transaction.id(), index as u32),
            UtxoEntry::new(
                output.value,
                output.script_public_key.clone(),
                0,
                false,
                output.covenant.map(|binding| binding.covenant_id),
            ),
        );
    }
    Ok(())
}

#[test]
fn signed_offline_vault_lifecycle_rejects_replay_and_invalid_payment() {
    // Public deterministic fixture key, used only for offline tests.
    let key = secp256k1::Keypair::from_seckey_slice(secp256k1::SECP256K1, &[7; 32]).unwrap();
    let mut genesis = plan();
    let mut ledger = std::collections::HashMap::from([(
        genesis.inputs[0].outpoint,
        genesis.inputs[0].utxo.clone(),
    )]);
    genesis.outputs[0].script_public_key = signed_vault_script(&key);
    genesis
        .genesis_groups
        .push(GenesisCovenantGroup::new(0, vec![0]));
    let genesis = genesis.build().unwrap();
    apply_fixture(&mut ledger, &genesis).unwrap();
    let before_continuation = ledger.clone();
    let (outpoint, utxo) = ledger.iter().next().unwrap();
    let mut next = plan();
    next.inputs[0].outpoint = *outpoint;
    next.inputs[0].utxo = utxo.clone();
    next.outputs[0] = TransactionOutput::with_covenant(
        8_000,
        signed_vault_script(&key),
        Some(CovenantBinding::new(0, utxo.covenant_id.unwrap())),
    );
    let mut next = next.build().unwrap();
    sign_fixture(&mut next, true, &key);
    apply_fixture(&mut ledger, &next).unwrap();
    let accepted_state = ledger.clone();
    assert!(apply_fixture(&mut ledger, &next).is_err());
    assert_eq!(
        ledger, accepted_state,
        "replay must not mutate fixture state"
    );
    ledger = before_continuation;
    apply_fixture(&mut ledger, &next).unwrap();
    assert_eq!(
        ledger, accepted_state,
        "reapplying after fixture rollback is deterministic"
    );

    let (outpoint, utxo) = ledger.iter().next().unwrap();
    let mut exit = plan();
    exit.inputs[0].outpoint = *outpoint;
    exit.inputs[0].utxo = utxo.clone();
    exit.outputs[0].value = 7_000;
    let mut exit = exit.build().unwrap();
    sign_fixture(&mut exit, false, &key);
    exit.validate_scripts().unwrap();
    exit.transaction.outputs[0].value = 6_999;
    assert!(
        apply_fixture(&mut ledger, &exit).is_err(),
        "modified signed output must fail"
    );
    assert_eq!(ledger, accepted_state);
    exit.transaction.outputs[0].value = 7_000;
    exit.transaction.finalize();
    apply_fixture(&mut ledger, &exit).unwrap();
    assert_eq!(ledger.len(), 1);
    let payout = ledger.values().next().unwrap();
    assert_eq!(payout.amount, 7_000);
    assert_eq!(payout.covenant_id, None);
}

#[cfg(feature = "silverscript")]
#[test]
fn released_silverscript_bytecode_executes_in_v2_1_native_engine() {
    use silverscript_lang::compiler::{CompileOptions, compile_contract};
    let source = "contract KeepValue() { entry spend() { require(tx.outputs[0].value >= tx.inputs[0].value - 1000); } }";
    let compiled = compile_contract(source, &[], CompileOptions::default()).unwrap();
    assert!(!compiled.bytecode.is_empty());
    let mut p = plan();
    p.inputs[0].witness = ScriptBuilder::new()
        .add_data(&compiled.dispatch_tags["spend"])
        .unwrap()
        .drain();
    p.inputs[0].utxo.script_public_key = ScriptPublicKey::new(0, compiled.bytecode.into());
    let mut native = p.build().unwrap();
    native.validate_scripts().unwrap();
    native.transaction.outputs[0].value = 8_999;
    assert!(
        native.validate_scripts().is_err(),
        "compiled value constraint must reject underpayment"
    );
}

#[test]
fn kaspascript_compiler_output_runs_in_v2_1_native_engine() {
    let source =
        "contract PreserveValue { spend advance() { require output(0).value >= input(0).value; } }";
    let compiled = kaspascript_codegen::compile_file(source, "native-compat.ks").unwrap();
    let spend_script = kaspascript_codegen::backends::toccata::compile_spend_script(
        &compiled.contracts[0].spends[0].instructions,
    )
    .unwrap();
    let mut p = plan();
    p.inputs[0].utxo.script_public_key = ScriptPublicKey::new(0, spend_script.into());
    p.outputs[0].value = 10_000;
    p.expected_fee = 0;
    let mut native = p.build().unwrap();
    native.validate_scripts().unwrap();
    native.transaction.outputs[0].value = 9_999;
    assert!(
        native.validate_scripts().is_err(),
        "KaspaScript constraint must reject value loss"
    );
}

#[test]
fn missing_input_metadata_returns_an_error_without_panicking() {
    let mut native = plan().build().unwrap();
    native.entries.clear();
    assert!(matches!(
        native.validate_scripts(),
        Err(PlanError::InputMetadataMismatch)
    ));
}
