use kaspascript_kernel::{
    dagsafe_vault_blueprint, EvidenceLevel, KernelFeature, Network, PreviewClassification,
    ToccataFeePolicy,
};

#[test]
fn dagsafe_vault_is_a_kernel_package_not_just_script() {
    let package = dagsafe_vault_blueprint().package().expect("package");

    assert_eq!(package.blueprint.name, "DAGSafeVault");
    assert_eq!(package.blueprint.network, Network::Tn10);
    assert!(package.readiness.ready);
    assert!(package
        .wallet_previews
        .iter()
        .all(|preview| preview.classification == PreviewClassification::CovenantStateTransition));
    assert!(package
        .indexer_schema
        .tables
        .iter()
        .any(|table| table.name == "covenant_transition"));
}

#[test]
fn covenant_id_requirements_are_tn10_gated() {
    let package = dagsafe_vault_blueprint().package().expect("package");

    let covenant_lines = package
        .readiness
        .features
        .iter()
        .filter(|line| line.feature == KernelFeature::CovenantIds)
        .collect::<Vec<_>>();

    assert!(!covenant_lines.is_empty());
    assert!(covenant_lines
        .iter()
        .all(|line| line.best == Some(EvidenceLevel::TestnetActivation)));
}

#[test]
fn toccata_fee_policy_uses_compute_or_two_times_tx_bytes() {
    let policy = ToccataFeePolicy::default();

    assert_eq!(policy.minimum_standard_fee(20, 11).expect("fee"), 2_200);
    assert_eq!(policy.minimum_standard_fee(25, 11).expect("fee"), 2_500);
}
