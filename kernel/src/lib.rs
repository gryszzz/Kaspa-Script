//! Kaspa-native programmability kernel.
//!
//! The kernel models Kaspa contracts as UTXO state machines. It packages the
//! pieces a real app needs around bytecode: source evidence, network posture,
//! wallet previews, indexer schema, and fee-policy math. It intentionally does
//! not unlock Toccata bytecode lowering until the compiler backend has pinned
//! opcode ABI tests.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;
use thiserror::Error;

/// Source audit date for the bundled Toccata evidence set.
pub const TOCCATA_AUDIT_DATE: &str = "2026-06-04T03:33:39Z";

/// Kaspa network scope for a kernel artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Network {
    Mainnet,
    Tn10,
    Tn12,
    Simnet,
    Devnet,
    Unknown,
}

impl fmt::Display for Network {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Mainnet => "mainnet",
            Self::Tn10 => "tn10",
            Self::Tn12 => "tn12",
            Self::Simnet => "simnet",
            Self::Devnet => "devnet",
            Self::Unknown => "unknown",
        };
        f.write_str(name)
    }
}

/// Programmability features the kernel can reason about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KernelFeature {
    BaseScript,
    TransactionIntrospection,
    CovenantIds,
    SequencingCommitments,
    ZkVerification,
    FeePolicy,
    WalletPreview,
    IndexerLineage,
}

impl fmt::Display for KernelFeature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::BaseScript => "base-script",
            Self::TransactionIntrospection => "transaction-introspection",
            Self::CovenantIds => "covenant-ids",
            Self::SequencingCommitments => "sequencing-commitments",
            Self::ZkVerification => "zk-verification",
            Self::FeePolicy => "fee-policy",
            Self::WalletPreview => "wallet-preview",
            Self::IndexerLineage => "indexer-lineage",
        };
        f.write_str(name)
    }
}

/// Evidence strength. Higher values can satisfy lower requirements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EvidenceLevel {
    Unknown,
    ResearchSignal,
    DocsSignal,
    BranchCode,
    MergedKip,
    MergedCode,
    TestnetActivation,
    MainnetPreActivation,
    MainnetActivation,
}

impl EvidenceLevel {
    const fn rank(self) -> u8 {
        match self {
            Self::Unknown => 0,
            Self::ResearchSignal => 10,
            Self::DocsSignal => 20,
            Self::BranchCode => 30,
            Self::MergedKip => 40,
            Self::MergedCode => 50,
            Self::TestnetActivation => 60,
            Self::MainnetPreActivation => 70,
            Self::MainnetActivation => 100,
        }
    }

    pub const fn proves_mainnet_activation(self) -> bool {
        matches!(self, Self::MainnetActivation)
    }
}

impl PartialOrd for EvidenceLevel {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EvidenceLevel {
    fn cmp(&self, other: &Self) -> Ordering {
        self.rank().cmp(&other.rank())
    }
}

impl fmt::Display for EvidenceLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Unknown => "unknown",
            Self::ResearchSignal => "research-signal",
            Self::DocsSignal => "docs-signal",
            Self::BranchCode => "branch-code",
            Self::MergedKip => "merged-kip",
            Self::MergedCode => "merged-code",
            Self::TestnetActivation => "testnet-activation",
            Self::MainnetPreActivation => "mainnet-pre-activation",
            Self::MainnetActivation => "mainnet-activation",
        };
        f.write_str(name)
    }
}

/// Primary-source evidence attached to a kernel artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceEvidence {
    pub label: String,
    pub url: String,
    pub audit_date: String,
    pub network: Network,
    pub level: EvidenceLevel,
    pub features: Vec<KernelFeature>,
    pub note: String,
}

impl SourceEvidence {
    pub fn new(
        label: impl Into<String>,
        url: impl Into<String>,
        network: Network,
        level: EvidenceLevel,
        features: Vec<KernelFeature>,
        note: impl Into<String>,
    ) -> Self {
        Self {
            label: label.into(),
            url: url.into(),
            audit_date: TOCCATA_AUDIT_DATE.to_owned(),
            network,
            level,
            features,
            note: note.into(),
        }
    }

    fn covers(&self, feature: KernelFeature, network: Network) -> bool {
        self.features.contains(&feature)
            && (self.network == Network::Unknown || self.network == network)
    }
}

/// Contract state type understood by the kernel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StateType {
    PublicKey,
    Signature,
    Sompi,
    DaaScore,
    BlockHeight,
    CovenantId,
    Hash,
    Bool,
    U64,
    Bytes,
}

/// Contract state field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateField {
    pub name: String,
    pub ty: StateType,
    pub description: String,
}

impl StateField {
    pub fn new(name: impl Into<String>, ty: StateType, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ty,
            description: description.into(),
        }
    }
}

/// Contract transition category.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransitionKind {
    Deposit,
    Spend,
    Timeout,
    Recover,
    Close,
    Custom(String),
}

/// Feature requirement for a transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureRequirement {
    pub feature: KernelFeature,
    pub minimum_evidence: EvidenceLevel,
    pub reason: String,
}

impl FeatureRequirement {
    pub fn new(
        feature: KernelFeature,
        minimum_evidence: EvidenceLevel,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            feature,
            minimum_evidence,
            reason: reason.into(),
        }
    }
}

/// Optional proof requirement for a transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProofRequirement {
    pub verifier: ProofVerifier,
    pub public_inputs: Vec<String>,
    pub payload_hint: String,
}

/// Supported proof verifier labels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProofVerifier {
    None,
    Groth16,
    Risc0Succinct,
    External(String),
}

/// A Kaspa-native contract transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transition {
    pub name: String,
    pub kind: TransitionKind,
    pub consumes: Vec<String>,
    pub creates: Vec<String>,
    pub signers: Vec<String>,
    pub requirements: Vec<FeatureRequirement>,
    pub proof: ProofRequirement,
    pub wallet_warnings: Vec<String>,
}

impl Transition {
    pub fn new(name: impl Into<String>, kind: TransitionKind) -> Self {
        Self {
            name: name.into(),
            kind,
            consumes: Vec::new(),
            creates: Vec::new(),
            signers: Vec::new(),
            requirements: Vec::new(),
            proof: ProofRequirement {
                verifier: ProofVerifier::None,
                public_inputs: Vec::new(),
                payload_hint: "no proof payload".to_owned(),
            },
            wallet_warnings: Vec::new(),
        }
    }

    pub fn consumes(mut self, field: impl Into<String>) -> Self {
        self.consumes.push(field.into());
        self
    }

    pub fn creates(mut self, field: impl Into<String>) -> Self {
        self.creates.push(field.into());
        self
    }

    pub fn signer(mut self, signer: impl Into<String>) -> Self {
        self.signers.push(signer.into());
        self
    }

    pub fn requires(mut self, requirement: FeatureRequirement) -> Self {
        self.requirements.push(requirement);
        self
    }

    pub fn proof(mut self, proof: ProofRequirement) -> Self {
        self.proof = proof;
        self
    }

    pub fn wallet_warning(mut self, warning: impl Into<String>) -> Self {
        self.wallet_warnings.push(warning.into());
        self
    }
}

/// Contract blueprint packaged by the kernel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractBlueprint {
    pub name: String,
    pub network: Network,
    pub state: Vec<StateField>,
    pub transitions: Vec<Transition>,
    pub evidence: Vec<SourceEvidence>,
}

impl ContractBlueprint {
    pub fn builder(name: impl Into<String>) -> ContractBuilder {
        ContractBuilder {
            blueprint: Self {
                name: name.into(),
                network: Network::Unknown,
                state: Vec::new(),
                transitions: Vec::new(),
                evidence: Vec::new(),
            },
        }
    }

    pub fn with_network(mut self, network: Network) -> Self {
        self.network = network;
        self
    }

    pub fn transition(&self, name: &str) -> Option<&Transition> {
        self.transitions
            .iter()
            .find(|transition| transition.name == name)
    }

    pub fn preview_transition(&self, name: &str) -> Result<WalletPreview, KernelError> {
        let transition = self
            .transition(name)
            .ok_or_else(|| KernelError::UnknownTransition(name.to_owned()))?;

        let mut warnings = transition.wallet_warnings.clone();
        warnings.push(
            "This is a covenant state transition preview, not an ordinary payment preview."
                .to_owned(),
        );
        if self.network == Network::Mainnet
            && !self
                .evidence
                .iter()
                .any(|evidence| evidence.level.proves_mainnet_activation())
        {
            warnings
                .push("Mainnet activation is not verified for this contract blueprint.".to_owned());
        }

        Ok(WalletPreview {
            contract: self.name.clone(),
            transition: transition.name.clone(),
            network: self.network,
            classification: PreviewClassification::CovenantStateTransition,
            consumes: transition.consumes.clone(),
            creates: transition.creates.clone(),
            signers: transition.signers.clone(),
            proof: transition.proof.clone(),
            warnings,
        })
    }

    pub fn readiness_report(&self) -> ReadinessReport {
        let mut blockers = Vec::new();
        let mut feature_reports = Vec::new();

        if self.network == Network::Mainnet
            && !self
                .evidence
                .iter()
                .any(|evidence| evidence.level.proves_mainnet_activation())
        {
            blockers.push(
                "mainnet activation is not verified; pre-activation releases are not enough"
                    .to_owned(),
            );
        }

        for transition in &self.transitions {
            for requirement in &transition.requirements {
                let best = self.best_evidence(requirement.feature);
                let satisfied = best
                    .as_ref()
                    .is_some_and(|evidence| evidence.level >= requirement.minimum_evidence);
                if !satisfied {
                    blockers.push(format!(
                        "transition `{}` requires {} at {}, best evidence is {}",
                        transition.name,
                        requirement.feature,
                        requirement.minimum_evidence,
                        best.as_ref()
                            .map(|evidence| evidence.level.to_string())
                            .unwrap_or_else(|| EvidenceLevel::Unknown.to_string())
                    ));
                }
                feature_reports.push(FeatureReadiness {
                    transition: transition.name.clone(),
                    feature: requirement.feature,
                    required: requirement.minimum_evidence,
                    best: best.as_ref().map(|evidence| evidence.level),
                    satisfied,
                    source_label: best.map(|evidence| evidence.label),
                });
            }
        }

        ReadinessReport {
            contract: self.name.clone(),
            network: self.network,
            ready: blockers.is_empty(),
            blockers,
            features: feature_reports,
        }
    }

    pub fn indexer_schema(&self) -> IndexerSchema {
        covenant_lineage_schema(self.name.clone(), self.network)
    }

    pub fn package(&self) -> Result<KernelPackage, KernelError> {
        let wallet_previews = self
            .transitions
            .iter()
            .map(|transition| self.preview_transition(&transition.name))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(KernelPackage {
            blueprint: self.clone(),
            readiness: self.readiness_report(),
            wallet_previews,
            indexer_schema: self.indexer_schema(),
            fee_policy: ToccataFeePolicy::default(),
        })
    }

    fn best_evidence(&self, feature: KernelFeature) -> Option<SourceEvidence> {
        self.evidence
            .iter()
            .filter(|evidence| evidence.covers(feature, self.network))
            .max_by_key(|evidence| evidence.level)
            .cloned()
    }
}

/// Builder for contract blueprints.
#[derive(Debug, Clone)]
pub struct ContractBuilder {
    blueprint: ContractBlueprint,
}

impl ContractBuilder {
    pub fn network(mut self, network: Network) -> Self {
        self.blueprint.network = network;
        self
    }

    pub fn state_field(mut self, field: StateField) -> Self {
        self.blueprint.state.push(field);
        self
    }

    pub fn transition(mut self, transition: Transition) -> Self {
        self.blueprint.transitions.push(transition);
        self
    }

    pub fn evidence(mut self, evidence: SourceEvidence) -> Self {
        self.blueprint.evidence.push(evidence);
        self
    }

    pub fn build(self) -> Result<ContractBlueprint, KernelError> {
        if self.blueprint.name.trim().is_empty() {
            return Err(KernelError::EmptyContractName);
        }
        if self.blueprint.transitions.is_empty() {
            return Err(KernelError::NoTransitions {
                contract: self.blueprint.name,
            });
        }
        Ok(self.blueprint)
    }
}

/// Wallet preview classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreviewClassification {
    OrdinaryPayment,
    CovenantStateTransition,
    ProofBearingTransition,
}

/// Wallet-facing transition preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletPreview {
    pub contract: String,
    pub transition: String,
    pub network: Network,
    pub classification: PreviewClassification,
    pub consumes: Vec<String>,
    pub creates: Vec<String>,
    pub signers: Vec<String>,
    pub proof: ProofRequirement,
    pub warnings: Vec<String>,
}

/// Indexer schema emitted with a contract package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexerSchema {
    pub contract: String,
    pub network: Network,
    pub tables: Vec<TableSpec>,
}

/// Table schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableSpec {
    pub name: String,
    pub columns: Vec<ColumnSpec>,
}

/// Column schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnSpec {
    pub name: String,
    pub ty: String,
    pub required: bool,
}

/// Readiness report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadinessReport {
    pub contract: String,
    pub network: Network,
    pub ready: bool,
    pub blockers: Vec<String>,
    pub features: Vec<FeatureReadiness>,
}

/// Per-feature readiness line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureReadiness {
    pub transition: String,
    pub feature: KernelFeature,
    pub required: EvidenceLevel,
    pub best: Option<EvidenceLevel>,
    pub satisfied: bool,
    pub source_label: Option<String>,
}

/// Complete kernel package for an app builder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelPackage {
    pub blueprint: ContractBlueprint,
    pub readiness: ReadinessReport,
    pub wallet_previews: Vec<WalletPreview>,
    pub indexer_schema: IndexerSchema,
    pub fee_policy: ToccataFeePolicy,
}

/// Compiler artifact summary embedded in CLI kernel packages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledArtifactSummary {
    pub backend: String,
    pub target: String,
    pub compiler_version: String,
    pub bytecode_bytes: usize,
    pub finality_depth: Option<u64>,
    pub kip_requirements: Vec<u16>,
    pub contracts: Vec<String>,
    pub spends: Vec<String>,
}

/// Fee estimate with explicit assumptions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeeEstimate {
    pub policy: String,
    pub source: String,
    pub compute_grams: u64,
    pub transaction_bytes: u64,
    pub minimum_standard_fee_sompi: u64,
    pub assumption: String,
}

/// Combined compiler plus kernel package emitted by the CLI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledKernelPackage {
    pub artifact: CompiledArtifactSummary,
    pub bytecode_hex: String,
    pub bytecode_asm: String,
    pub kernel: KernelPackage,
    pub fee_estimate: FeeEstimate,
}

/// Toccata pre-activation RPC minimum-standard-fee policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToccataFeePolicy {
    pub sompi_per_unit: u64,
}

impl Default for ToccataFeePolicy {
    fn default() -> Self {
        Self {
            sompi_per_unit: 100,
        }
    }
}

impl ToccataFeePolicy {
    pub fn minimum_standard_fee(
        self,
        compute_grams: u64,
        transaction_bytes: u64,
    ) -> Result<u64, KernelError> {
        let normalized_bytes = transaction_bytes
            .checked_mul(2)
            .ok_or(KernelError::ArithmeticOverflow)?;
        let units = compute_grams.max(normalized_bytes);
        units
            .checked_mul(self.sompi_per_unit)
            .ok_or(KernelError::ArithmeticOverflow)
    }

    pub fn estimate(
        self,
        compute_grams: u64,
        transaction_bytes: u64,
        assumption: impl Into<String>,
    ) -> Result<FeeEstimate, KernelError> {
        Ok(FeeEstimate {
            policy: "toccata-rpc-minimum-standard-fee".to_owned(),
            source: "https://github.com/kaspanet/rusty-kaspa/releases/tag/v1.3.0-toc.5".to_owned(),
            compute_grams,
            transaction_bytes,
            minimum_standard_fee_sompi: self
                .minimum_standard_fee(compute_grams, transaction_bytes)?,
            assumption: assumption.into(),
        })
    }
}

/// Kernel errors.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum KernelError {
    #[error("contract name cannot be empty")]
    EmptyContractName,
    #[error("contract `{contract}` must define at least one transition")]
    NoTransitions { contract: String },
    #[error("unknown transition `{0}`")]
    UnknownTransition(String),
    #[error("arithmetic overflow")]
    ArithmeticOverflow,
}

/// Starts a Kaspa contract blueprint.
pub fn define_kaspa_contract(name: impl Into<String>) -> ContractBuilder {
    ContractBlueprint::builder(name)
}

/// Combines a compiler artifact summary and kernel blueprint into one package.
pub fn package_compiled_contract(
    artifact: CompiledArtifactSummary,
    bytecode_hex: impl Into<String>,
    bytecode_asm: impl Into<String>,
    blueprint: ContractBlueprint,
    compute_grams: u64,
    transaction_bytes: u64,
    fee_assumption: impl Into<String>,
) -> Result<CompiledKernelPackage, KernelError> {
    let fee_policy = ToccataFeePolicy::default();
    Ok(CompiledKernelPackage {
        artifact,
        bytecode_hex: bytecode_hex.into(),
        bytecode_asm: bytecode_asm.into(),
        kernel: blueprint.package()?,
        fee_estimate: fee_policy.estimate(compute_grams, transaction_bytes, fee_assumption)?,
    })
}

/// Current bundled Toccata evidence used by examples and tests.
pub fn current_toccata_evidence() -> Vec<SourceEvidence> {
    vec![
        SourceEvidence::new(
            "rusty-kaspa v1.3.0-toc.5",
            "https://github.com/kaspanet/rusty-kaspa/releases/tag/v1.3.0-toc.5",
            Network::Mainnet,
            EvidenceLevel::MainnetPreActivation,
            vec![KernelFeature::FeePolicy],
            "mainnet sanity-testing pre-release; explicitly not mainnet activation",
        ),
        SourceEvidence::new(
            "rusty-kaspa PR #1000",
            "https://github.com/kaspanet/rusty-kaspa/pull/1000",
            Network::Unknown,
            EvidenceLevel::MergedCode,
            vec![
                KernelFeature::TransactionIntrospection,
                KernelFeature::CovenantIds,
                KernelFeature::SequencingCommitments,
                KernelFeature::ZkVerification,
            ],
            "Toccata implementation merged into master",
        ),
        SourceEvidence::new(
            "rusty-kaspa tn10-toc3",
            "https://github.com/kaspanet/rusty-kaspa/releases/tag/tn10-toc3",
            Network::Tn10,
            EvidenceLevel::TestnetActivation,
            vec![
                KernelFeature::ZkVerification,
                KernelFeature::SequencingCommitments,
            ],
            "TN10 Toccata ZK hardening activation evidence",
        ),
        SourceEvidence::new(
            "KIP-17 merged file",
            "https://github.com/kaspanet/kips/blob/master/kip-0017.md",
            Network::Tn10,
            EvidenceLevel::TestnetActivation,
            vec![KernelFeature::TransactionIntrospection],
            "merged KIP status indicates implemented and activated in TN10",
        ),
        SourceEvidence::new(
            "KIP-20 merged file",
            "https://github.com/kaspanet/kips/blob/master/kip-0020.md",
            Network::Tn10,
            EvidenceLevel::TestnetActivation,
            vec![KernelFeature::CovenantIds],
            "merged KIP status indicates implemented and activated in TN10",
        ),
        SourceEvidence::new(
            "KIP-21 merged file",
            "https://github.com/kaspanet/kips/blob/master/kip-0021.md",
            Network::Tn10,
            EvidenceLevel::TestnetActivation,
            vec![KernelFeature::SequencingCommitments],
            "merged KIP status indicates implemented and activated in TN10",
        ),
        SourceEvidence::new(
            "KaspaScript kernel artifacts",
            "kernel/src/lib.rs",
            Network::Unknown,
            EvidenceLevel::BranchCode,
            vec![KernelFeature::WalletPreview, KernelFeature::IndexerLineage],
            "local kernel emits wallet preview and covenant lineage schema artifacts",
        ),
    ]
}

/// Flagship starter contract: a UTXO-native covenant vault blueprint.
pub fn dagsafe_vault_blueprint() -> ContractBlueprint {
    let mut builder = define_kaspa_contract("DAGSafeVault")
        .network(Network::Tn10)
        .state_field(StateField::new(
            "owner",
            StateType::PublicKey,
            "key authorized to release funds after the unlock DAA score",
        ))
        .state_field(StateField::new(
            "recovery_key",
            StateType::PublicKey,
            "emergency key for mediated recovery flow",
        ))
        .state_field(StateField::new(
            "unlock_daa",
            StateType::DaaScore,
            "minimum DAA score required for normal release",
        ))
        .state_field(StateField::new(
            "covenant_id",
            StateType::CovenantId,
            "lineage identifier carried by successor outputs",
        ))
        .state_field(StateField::new(
            "policy_hash",
            StateType::Hash,
            "hash of the app-level vault policy shown to the wallet",
        ));

    for evidence in current_toccata_evidence() {
        builder = builder.evidence(evidence);
    }

    builder
        .transition(
            Transition::new("deposit", TransitionKind::Deposit)
                .creates("successor covenant output with same covenant_id")
                .requires(FeatureRequirement::new(
                    KernelFeature::CovenantIds,
                    EvidenceLevel::TestnetActivation,
                    "deposits must preserve covenant lineage on the target network",
                ))
                .requires(FeatureRequirement::new(
                    KernelFeature::IndexerLineage,
                    EvidenceLevel::Unknown,
                    "local indexer schema is emitted by this kernel package",
                ))
                .wallet_warning("Deposit creates stateful covenant funds that should be tracked by covenant_id."),
        )
        .transition(
            Transition::new("release_after_unlock", TransitionKind::Spend)
                .consumes("current vault covenant output")
                .creates("owner-controlled output")
                .signer("owner")
                .requires(FeatureRequirement::new(
                    KernelFeature::TransactionIntrospection,
                    EvidenceLevel::TestnetActivation,
                    "release checks current DAA/order context and successor output shape",
                ))
                .requires(FeatureRequirement::new(
                    KernelFeature::CovenantIds,
                    EvidenceLevel::TestnetActivation,
                    "release consumes a covenant-bound UTXO",
                ))
                .wallet_warning("Wallet must show consumed vault state, unlock_daa, and resulting owner output."),
        )
        .transition(
            Transition::new("emergency_recover", TransitionKind::Recover)
                .consumes("current vault covenant output")
                .creates("recovery covenant output")
                .signer("recovery_key")
                .requires(FeatureRequirement::new(
                    KernelFeature::CovenantIds,
                    EvidenceLevel::TestnetActivation,
                    "recovery must keep lineage queryable after a mediated state change",
                ))
                .requires(FeatureRequirement::new(
                    KernelFeature::SequencingCommitments,
                    EvidenceLevel::TestnetActivation,
                    "recovery policy should be ordered against accepted transaction context",
                ))
                .wallet_warning("Recovery changes vault control; require explicit user confirmation."),
        )
        .build()
        .expect("DAGSafeVault blueprint is valid")
}

fn covenant_lineage_schema(contract: String, network: Network) -> IndexerSchema {
    IndexerSchema {
        contract,
        network,
        tables: vec![
            TableSpec {
                name: "covenant_lineage".to_owned(),
                columns: vec![
                    column("covenant_id", "bytes32", true),
                    column("network", "text", true),
                    column("genesis_tx_id", "bytes32", true),
                    column("genesis_output_index", "u32", true),
                    column("current_tip_tx_id", "bytes32", true),
                    column("current_tip_output_index", "u32", true),
                    column("last_seen_daa_score", "u64", true),
                    column("status", "text", true),
                ],
            },
            TableSpec {
                name: "covenant_transition".to_owned(),
                columns: vec![
                    column("covenant_id", "bytes32", true),
                    column("spending_tx_id", "bytes32", true),
                    column("consumed_outpoint", "text", true),
                    column("successor_outpoint", "text", true),
                    column("authorizing_input_index", "u32", true),
                    column("accepting_block_hash", "bytes32", true),
                    column("accepting_daa_score", "u64", true),
                    column("transition_status", "text", true),
                ],
            },
            TableSpec {
                name: "wallet_preview_audit".to_owned(),
                columns: vec![
                    column("preview_id", "text", true),
                    column("contract", "text", true),
                    column("transition", "text", true),
                    column("network", "text", true),
                    column("consumes_json", "json", true),
                    column("creates_json", "json", true),
                    column("warnings_json", "json", true),
                ],
            },
        ],
    }
}

fn column(name: &str, ty: &str, required: bool) -> ColumnSpec {
    ColumnSpec {
        name: name.to_owned(),
        ty: ty.to_owned(),
        required,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toccata_fee_policy_matches_release_formula() {
        let fee = ToccataFeePolicy::default()
            .minimum_standard_fee(1_000, 400)
            .expect("fee");
        assert_eq!(fee, 100_000);

        let byte_dominated = ToccataFeePolicy::default()
            .minimum_standard_fee(500, 400)
            .expect("fee");
        assert_eq!(byte_dominated, 80_000);
    }

    #[test]
    fn dagsafe_vault_packages_wallet_and_indexer_artifacts() {
        let vault = dagsafe_vault_blueprint();
        let package = vault.package().expect("package");

        assert!(package.readiness.ready, "{:?}", package.readiness.blockers);
        assert_eq!(package.wallet_previews.len(), 3);
        assert!(package
            .indexer_schema
            .tables
            .iter()
            .any(|table| table.name == "covenant_lineage"));
    }

    #[test]
    fn wallet_preview_labels_state_transition_not_ordinary_payment() {
        let preview = dagsafe_vault_blueprint()
            .preview_transition("release_after_unlock")
            .expect("preview");

        assert_eq!(
            preview.classification,
            PreviewClassification::CovenantStateTransition
        );
        assert!(preview
            .warnings
            .iter()
            .any(|warning| warning.contains("not an ordinary payment")));
    }

    #[test]
    fn mainnet_blueprint_is_blocked_without_activation_evidence() {
        let vault = dagsafe_vault_blueprint().with_network(Network::Mainnet);
        let report = vault.readiness_report();

        assert!(!report.ready);
        assert!(report
            .blockers
            .iter()
            .any(|blocker| blocker.contains("mainnet activation is not verified")));
    }
}
