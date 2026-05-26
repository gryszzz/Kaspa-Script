#![cfg(any(feature = "tn12-integration", feature = "testnet-integration"))]

use kaspascript_sdk::tn12::{
    ContractDeploymentPlan, ProofResult, TestWallet, Tn12Config, Tn12ContractHarness, Tn12Error,
    Tn12RpcClient, TN12_NETWORK_ID,
};

const CONTRACTS: &[(&str, &str, &str)] = &[
    (
        "escrow",
        "escrow.ks",
        include_str!("../../tests/contracts/escrow.ks"),
    ),
    (
        "timelock",
        "timelock.ks",
        include_str!("../../tests/contracts/timelock.ks"),
    ),
    (
        "multisig",
        "multisig.ks",
        include_str!("../../tests/contracts/multisig.ks"),
    ),
    (
        "atomic_swap",
        "atomic_swap.ks",
        include_str!("../../tests/contracts/atomic_swap.ks"),
    ),
    (
        "vault",
        "vault.ks",
        include_str!("../../tests/contracts/vault.ks"),
    ),
];

#[test]
fn offline_contract_deployment_plans_are_deterministic() {
    for (name, file, source) in CONTRACTS {
        let first = ContractDeploymentPlan::from_source(name, file, source).expect(file);
        let second = ContractDeploymentPlan::from_source(name, file, source).expect(file);

        assert_eq!(first.source_hash, second.source_hash, "{file}");
        assert_eq!(first.artifact_hash, second.artifact_hash, "{file}");
        assert_eq!(first.script_hash, second.script_hash, "{file}");
        assert_eq!(first.script_hex, second.script_hex, "{file}");
        assert!(!first.script_hex.is_empty(), "{file}");
    }
}

#[test]
fn offline_wallet_generation_never_requires_a_live_node() {
    let wallet = TestWallet::generate_ephemeral().expect("wallet");

    assert!(wallet.address_string().starts_with("kaspatest:"));
    assert_eq!(wallet.public_key().len(), 32);
    assert_eq!(wallet.sign_spend_digest([7; 32]).len(), 64);
}

#[tokio::test]
#[ignore = "requires KASPA_TN12_RPC_URL and KASPA_TN12_PRIVATE_KEY"]
async fn tn12_rpc_wallet_preflight() {
    let config = Tn12Config::from_env().expect("TN12 env");
    let rpc = Tn12RpcClient::connect(&config).await.expect("TN12 RPC");
    let info = rpc.network_info().await.expect("network info");

    assert_eq!(info.network, TN12_NETWORK_ID);
    assert!(
        info.has_utxo_index,
        "TN12 integration tests need a node with the UTXO index enabled"
    );

    let wallet = TestWallet::from_env().expect("test wallet");
    println!("TN12 wallet address: {}", wallet.address_string());
    println!(
        "TN12 wallet key fingerprint: {}",
        wallet.private_key_fingerprint()
    );
    let balance = wallet.list_balance(&rpc).await.expect("wallet balance");
    println!("TN12 wallet balance: {balance} sompi");
    let _utxos = rpc
        .fetch_utxos(wallet.address())
        .await
        .expect("wallet UTXOs");
    let _fees = rpc
        .estimate_fees_if_supported()
        .await
        .expect("fee estimate call");

    rpc.disconnect().await.expect("disconnect");
}

#[tokio::test]
#[ignore = "requires live testnet env; dry-run by default unless KASPA_BROADCAST=true"]
async fn tn12_contract_suite_builds_real_transaction_previews() {
    let config = Tn12Config::from_env().expect("TN12 env");
    let rpc = Tn12RpcClient::connect(&config).await.expect("TN12 RPC");
    let wallet = TestWallet::from_env().expect("test wallet");
    let harness = Tn12ContractHarness::new(&rpc, &wallet);

    for (name, file, source) in CONTRACTS {
        let proof = match harness
            .deploy_and_execute(name, file, source, 1_000_000, &config)
            .await
        {
            Err(Tn12Error::Unsupported(_)) => harness
                .gated_proof(name, file, source)
                .await
                .expect("gated proof"),
            Ok(proof) => proof,
            Err(error) => panic!("{file}: unexpected TN12 harness error: {error}"),
        };

        if config.broadcast {
            assert_eq!(proof.result, ProofResult::Pass, "{file}");
            assert!(proof.lock_txid.is_some(), "{file}");
            assert!(proof.spend_txid.is_some(), "{file}");
        } else {
            assert_eq!(proof.result, ProofResult::Gated, "{file}");
            assert!(proof.lock_txid.is_none(), "{file}");
            assert!(proof.spend_txid.is_none(), "{file}");
        }
        assert_eq!(proof.network, TN12_NETWORK_ID, "{file}");
        proof
            .write_json(format!("tests/proofs/tn12/{name}.proof.json"))
            .expect("proof write");
    }

    rpc.disconnect().await.expect("disconnect");
}
