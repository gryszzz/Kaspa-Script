//! KaspaScript SDK surface.

#[cfg(any(feature = "tn12-integration", feature = "testnet-integration"))]
pub mod testnet;
#[cfg(any(feature = "tn12-integration", feature = "testnet-integration"))]
pub mod tn12;
pub mod toccata;

use kaspascript_codegen::{
    bytecode_asm, bytecode_hex, compile_file, compile_file_for_target, verify_artifact,
    CompiledArtifact, Target,
};
use kaspascript_kernel::{
    current_toccata_evidence, define_kaspa_contract, package_compiled_contract,
    CompiledArtifactSummary, CompiledKernelPackage, CompiledPackageInput, ContractBlueprint,
    EvidenceLevel, FeatureRequirement, KernelError, KernelFeature, Network, SourceEvidence,
    StateField, StateType, Transition, TransitionKind,
};
use kaspascript_lexer::TypeName;
use kaspascript_model::ApplicationModel;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// SDK compile error.
pub type CompileError = kaspascript_codegen::CodegenError;

/// Transaction builder maturity status.
pub const TRANSACTION_BUILDER_STATUS: &str = "preview";

/// Spend argument for transaction construction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpendArg {
    Bytes(Vec<u8>),
    Integer(u64),
    Bool(bool),
}

/// UTXO input used by the transaction builder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Utxo {
    pub outpoint: String,
    pub value: u64,
    pub confirmations: u64,
    pub script_pubkey: Vec<u8>,
}

/// Transaction output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxOutput {
    pub value: u64,
    pub script_pubkey: Vec<u8>,
}

/// Deterministic preview transaction model used by the SDK.
///
/// This is not yet a `rusty-kaspa` transaction. It exists to test finality
/// policy and artifact wiring until real Kaspa transaction construction is
/// integrated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transaction {
    pub spend_fn: String,
    pub inputs: Vec<Utxo>,
    pub outputs: Vec<TxOutput>,
    pub args: Vec<SpendArg>,
}

/// Transaction builder error.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TxBuildError {
    #[error("finality depth requires {required} confirmations, got {actual}")]
    InsufficientFinality { required: u64, actual: u64 },
    #[error("spend value is zero")]
    EmptySpend,
}

/// SDK error for compiling and packaging a KaspaScript kernel package.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PackageError {
    #[error("code generation error: {0}")]
    Codegen(#[from] kaspascript_codegen::CodegenError),
    #[error("kernel package error: {0}")]
    Kernel(#[from] KernelError),
}

/// Options for building a compiled kernel package from source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KernelPackageBuildOptions {
    pub target: Target,
    pub compute_grams: u64,
    pub tx_bytes: Option<u64>,
}

impl Default for KernelPackageBuildOptions {
    fn default() -> Self {
        Self {
            target: Target::VerifiedTn12,
            compute_grams: 0,
            tx_bytes: None,
        }
    }
}

impl KernelPackageBuildOptions {
    /// Creates package options for a specific compiler target.
    pub const fn new(target: Target) -> Self {
        Self {
            target,
            compute_grams: 0,
            tx_bytes: None,
        }
    }

    /// Sets the Toccata compute grams used for the fee estimate.
    pub const fn with_compute_grams(mut self, compute_grams: u64) -> Self {
        self.compute_grams = compute_grams;
        self
    }

    /// Sets the transaction byte estimate used for the fee estimate.
    pub const fn with_tx_bytes(mut self, tx_bytes: u64) -> Self {
        self.tx_bytes = Some(tx_bytes);
        self
    }
}

/// Compiles KaspaScript source.
pub fn compile(src: &str, file: &str) -> Result<CompiledArtifact, CompileError> {
    compile_file(src, file)
}

/// Compiles source and returns its canonical KaspaScript application model.
pub fn compile_application(src: &str, file: &str) -> Result<ApplicationModel, CompileError> {
    compile(src, file).map(|artifact| artifact.application)
}

/// Compiles source, verifies bytecode, and returns a complete kernel package.
pub fn build_kernel_package(
    src: &str,
    file: &str,
    options: KernelPackageBuildOptions,
) -> Result<CompiledKernelPackage, PackageError> {
    let artifact = compile_file_for_target(src, file, options.target)?;
    build_kernel_package_from_artifact(file, &artifact, options)
}

/// Verifies an existing artifact and wraps it in a complete kernel package.
pub fn build_kernel_package_from_artifact(
    file: &str,
    artifact: &CompiledArtifact,
    options: KernelPackageBuildOptions,
) -> Result<CompiledKernelPackage, PackageError> {
    verify_artifact(artifact)?;

    let transaction_bytes = options
        .tx_bytes
        .unwrap_or_else(|| u64::try_from(artifact.bytecode.len()).unwrap_or(u64::MAX));
    let fee_assumption = if options.tx_bytes.is_some() || options.compute_grams != 0 {
        "caller-provided fee estimate inputs"
    } else {
        "lower-bound estimate using compiled bytecode length as transaction_bytes and compute_grams=0"
    };

    Ok(package_compiled_contract(CompiledPackageInput {
        artifact: artifact_summary(artifact),
        bytecode_hex: bytecode_hex(&artifact.bytecode),
        bytecode_asm: bytecode_asm(&artifact.bytecode)?,
        application: artifact.application.clone(),
        blueprint: kernel_blueprint_from_artifact(file, artifact)?,
        compute_grams: options.compute_grams,
        transaction_bytes,
        fee_assumption: fee_assumption.to_owned(),
    })?)
}

/// Builds a preview spend transaction with finality enforcement and no hidden fees.
pub fn build_spend_tx(
    artifact: &CompiledArtifact,
    spend_fn: &str,
    args: Vec<SpendArg>,
    utxos: Vec<Utxo>,
) -> Result<Transaction, TxBuildError> {
    let total_value = utxos.iter().map(|utxo| utxo.value).sum::<u64>();
    if total_value == 0 {
        return Err(TxBuildError::EmptySpend);
    }

    if let Some(required) = artifact.finality_depth {
        for utxo in &utxos {
            if utxo.confirmations < required {
                return Err(TxBuildError::InsufficientFinality {
                    required,
                    actual: utxo.confirmations,
                });
            }
        }
    }

    Ok(Transaction {
        spend_fn: spend_fn.to_owned(),
        inputs: utxos,
        outputs: vec![TxOutput {
            value: total_value,
            script_pubkey: artifact.bytecode.clone(),
        }],
        args,
    })
}

fn artifact_summary(artifact: &CompiledArtifact) -> CompiledArtifactSummary {
    CompiledArtifactSummary {
        backend: artifact.backend.clone(),
        target: artifact.target.clone(),
        compiler_version: artifact.compiler_version.clone(),
        bytecode_bytes: artifact.bytecode.len(),
        finality_depth: artifact.finality_depth,
        kip_requirements: artifact.kip_requirements.clone(),
        contracts: artifact
            .contracts
            .iter()
            .map(|contract| contract.name.clone())
            .collect(),
        spends: artifact
            .contracts
            .iter()
            .flat_map(|contract| {
                contract
                    .spends
                    .iter()
                    .map(move |spend| format!("{}.{}", contract.name, spend.name))
            })
            .collect(),
        application_schema_version: artifact.application.schema_version.clone(),
    }
}

fn kernel_blueprint_from_artifact(
    source_path: &str,
    artifact: &CompiledArtifact,
) -> Result<ContractBlueprint, KernelError> {
    let network = network_from_target(&artifact.target);
    let contract_name = if artifact.contracts.len() == 1 {
        artifact.contracts[0].name.clone()
    } else {
        contract_name_from_path(source_path)
    };

    let mut builder = define_kaspa_contract(contract_name)
        .network(network)
        .evidence(local_artifact_evidence(source_path, network, artifact));

    for evidence in current_toccata_evidence() {
        builder = builder.evidence(evidence);
    }

    for contract in &artifact.contracts {
        for param in &contract.params {
            let field_name = if artifact.contracts.len() == 1 {
                param.name.clone()
            } else {
                format!("{}.{}", contract.name, param.name)
            };
            builder = builder.state_field(StateField::new(
                field_name,
                state_type_from_type_name(param.ty),
                format!("compiled parameter from contract {}", contract.name),
            ));
        }

        for spend in &contract.spends {
            let semantics = artifact
                .application
                .transition(&contract.name, &spend.name)
                .cloned();
            let mut transition = Transition::new(&spend.name, TransitionKind::Spend)
                .requires(FeatureRequirement::new(
                    KernelFeature::BaseScript,
                    EvidenceLevel::BranchCode,
                    "compiled artifact emitted verified Kaspa txscript bytecode",
                ))
                .requires(FeatureRequirement::new(
                    KernelFeature::WalletPreview,
                    EvidenceLevel::BranchCode,
                    "kernel package emits wallet preview metadata",
                ))
                .requires(FeatureRequirement::new(
                    KernelFeature::IndexerLineage,
                    EvidenceLevel::BranchCode,
                    "kernel package emits indexer schema metadata",
                ))
                .wallet_warning(format!(
                    "Review `{}` as a Kaspa contract spend path before signing.",
                    spend.name
                ));

            if let Some(semantics) = semantics {
                if semantics.transaction_shape.referenced_inputs.is_empty() {
                    transition =
                        transition.consumes(format!("{} compiled locking state", contract.name));
                } else {
                    for input in &semantics.transaction_shape.referenced_inputs {
                        transition =
                            transition.consumes(format!("input({input}) referenced by source"));
                    }
                }

                if semantics.output_bindings.is_empty() {
                    transition =
                        transition.creates("no output field is constrained by this spend path");
                } else {
                    for binding in &semantics.output_bindings {
                        transition = transition.creates(format!(
                            "output({}).{} {} {}",
                            binding.output_index, binding.field, binding.relation, binding.expected
                        ));
                    }
                }

                if semantics.transaction_shape.additional_outputs_permitted {
                    transition = transition.wallet_warning(
                        "Source does not constrain the exact output count; review every additional output and recipient.",
                    );
                }
                transition = transition.semantics(semantics);
            } else {
                transition = transition
                    .consumes(format!("{} compiled locking state", contract.name))
                    .creates("application model did not resolve this spend path");
            }

            if artifact.kip_requirements.contains(&10) {
                transition = transition.requires(FeatureRequirement::new(
                    KernelFeature::TransactionIntrospection,
                    EvidenceLevel::BranchCode,
                    "artifact declares KIP-10 transaction introspection requirements",
                ));
            }

            for param in &spend.params {
                if param.ty == TypeName::Signature {
                    transition = transition.signer(&param.name);
                }
            }

            builder = builder.transition(transition);
        }
    }

    builder.build()
}

fn local_artifact_evidence(
    source_path: &str,
    network: Network,
    artifact: &CompiledArtifact,
) -> SourceEvidence {
    let mut features = vec![
        KernelFeature::BaseScript,
        KernelFeature::WalletPreview,
        KernelFeature::IndexerLineage,
    ];
    if artifact.kip_requirements.contains(&10) {
        features.push(KernelFeature::TransactionIntrospection);
    }

    SourceEvidence::new(
        "KaspaScript compiled artifact",
        source_path,
        network,
        EvidenceLevel::BranchCode,
        features,
        "local compiler artifact verified before kernel package emission",
    )
}

fn network_from_target(target: &str) -> Network {
    match target {
        "verified-tn12" => Network::Tn12,
        "tn10-toccata" => Network::Tn10,
        "future-mainnet" => Network::Mainnet,
        "toccata-preview" => Network::Unknown,
        _ => Network::Unknown,
    }
}

fn state_type_from_type_name(ty: TypeName) -> StateType {
    match ty {
        TypeName::PublicKey => StateType::PublicKey,
        TypeName::Signature => StateType::Signature,
        TypeName::Hash => StateType::Hash,
        TypeName::BlockHeight => StateType::BlockHeight,
        TypeName::Amount => StateType::Sompi,
        TypeName::Bool => StateType::Bool,
        TypeName::Bytes => StateType::Bytes,
        TypeName::CovenantID => StateType::CovenantId,
        TypeName::ZKProof | TypeName::UTXO | TypeName::Output | TypeName::Input => StateType::Bytes,
    }
}

fn contract_name_from_path(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| {
            stem.split('_')
                .filter(|part| !part.is_empty())
                .map(|part| {
                    let mut chars = part.chars();
                    match chars.next() {
                        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                        None => String::new(),
                    }
                })
                .collect::<String>()
        })
        .unwrap_or_else(|| "Contract".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enforces_finality_depth() {
        let artifact = CompiledArtifact {
            bytecode: vec![1],
            source_hash: [0; 32],
            compiler_version: "test".to_owned(),
            backend: "kaspa-txscript".to_owned(),
            target: "verified-tn12".to_owned(),
            finality_depth: Some(10),
            kip_requirements: vec![10],
            warnings: Vec::new(),
            application: ApplicationModel::empty(),
            contracts: Vec::new(),
        };
        let result = build_spend_tx(
            &artifact,
            "withdraw",
            Vec::new(),
            vec![Utxo {
                outpoint: "a:0".to_owned(),
                value: 1_000,
                confirmations: 9,
                script_pubkey: Vec::new(),
            }],
        );
        assert!(matches!(
            result,
            Err(TxBuildError::InsufficientFinality { .. })
        ));
    }

    #[test]
    fn does_not_inject_hidden_fee() {
        let artifact = CompiledArtifact {
            bytecode: vec![1],
            source_hash: [0; 32],
            compiler_version: "test".to_owned(),
            backend: "kaspa-txscript".to_owned(),
            target: "verified-tn12".to_owned(),
            finality_depth: None,
            kip_requirements: Vec::new(),
            warnings: Vec::new(),
            application: ApplicationModel::empty(),
            contracts: Vec::new(),
        };
        let tx = build_spend_tx(
            &artifact,
            "withdraw",
            Vec::new(),
            vec![Utxo {
                outpoint: "a:0".to_owned(),
                value: 100_000,
                confirmations: 0,
                script_pubkey: Vec::new(),
            }],
        )
        .expect("transaction builds");

        assert_eq!(TRANSACTION_BUILDER_STATUS, "preview");
        assert_eq!(tx.outputs.len(), 1);
        assert_eq!(tx.outputs[0].value, 100_000);
        assert_eq!(tx.outputs[0].script_pubkey, vec![1]);
    }

    #[test]
    fn exposes_the_same_application_model_as_the_compiler_artifact() {
        let source = include_str!("../../tests/contracts/escrow.ks");
        let artifact = compile(source, "escrow.ks").expect("compile");
        let application = compile_application(source, "escrow.ks").expect("application");

        assert_eq!(application, artifact.application);
        assert_eq!(application.contracts[0].name, "Escrow");
        assert_eq!(application.contracts[0].transitions.len(), 2);
    }

    #[test]
    fn builds_kernel_package_without_invoking_cli() {
        let source = include_str!("../../tests/contracts/escrow.ks");
        let package = build_kernel_package(
            source,
            "tests/contracts/escrow.ks",
            KernelPackageBuildOptions::new(Target::VerifiedTn12)
                .with_compute_grams(1000)
                .with_tx_bytes(400),
        )
        .expect("kernel package");
        let actual = serde_json::to_string_pretty(&package).expect("json");

        assert_eq!(
            actual.trim_end(),
            include_str!("../../tests/golden/escrow.kernel.json").trim_end()
        );
    }

    #[test]
    fn package_default_fee_uses_bytecode_length_as_lower_bound() {
        let source = include_str!("../../tests/contracts/escrow.ks");
        let artifact = compile(source, "tests/contracts/escrow.ks").expect("compile");
        let package = build_kernel_package_from_artifact(
            "tests/contracts/escrow.ks",
            &artifact,
            KernelPackageBuildOptions::default(),
        )
        .expect("kernel package");

        assert_eq!(
            package.fee_estimate.transaction_bytes,
            artifact.bytecode.len() as u64
        );
        assert_eq!(
            package.fee_estimate.assumption,
            "lower-bound estimate using compiled bytecode length as transaction_bytes and compute_grams=0"
        );
    }
}
