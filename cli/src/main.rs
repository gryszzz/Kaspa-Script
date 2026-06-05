use std::env;
use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use kaspascript_codegen::{
    bytecode_asm, bytecode_hex, compile_file_for_target, verify_artifact, CompiledArtifact, Target,
};
use kaspascript_ir::lower_file;
use kaspascript_kernel::{
    current_toccata_evidence, define_kaspa_contract, package_compiled_contract,
    CompiledArtifactSummary, CompiledKernelPackage, ContractBlueprint, EvidenceLevel,
    FeatureRequirement, KernelFeature, Network, SourceEvidence, StateField, StateType, Transition,
    TransitionKind,
};
use kaspascript_lexer::TypeName;
use kaspascript_sdk::compile;

fn main() -> Result<()> {
    let args = env::args().collect::<Vec<_>>();
    if args.len() < 2 {
        bail!("{}", usage());
    }

    match args[1].as_str() {
        "compile" if args.len() == 3 => compile_command(&args[2]),
        "verify" if args.len() == 3 => verify_command(&args[2]),
        "inspect" if args.len() == 3 => inspect_command(&args[2]),
        "kernel" => kernel_command(&args[2..]),
        "wallet" => wallet_command(&args[2..]),
        "tx" => tx_command(&args[2..]),
        "proof" => proof_command(&args[2..]),
        other => bail!("unknown command `{other}`"),
    }
}

fn usage() -> &'static str {
    "usage: kaspascript <compile|verify|inspect> <file>\n       kaspascript kernel package <file.ks> [--target verified-tn12|tn10-toccata|toccata-preview|future-mainnet] [--output <file>] [--compute-grams <n>] [--tx-bytes <n>]\n       kaspascript wallet balance --target tn12\n       kaspascript tx lock <file.ks> --target tn12 --amount 1.0 [--dry-run|--broadcast]\n       kaspascript tx spend <artifact.json> --spend <name> --target tn12 [--dry-run|--broadcast]\n       kaspascript proof verify <proof.json>"
}

fn compile_command(path: &str) -> Result<()> {
    let source = fs::read_to_string(path).with_context(|| format!("failed to read {path}"))?;
    let artifact = compile(&source, path).map_err(|error| anyhow::anyhow!("error: {error}"))?;
    let json = serde_json::to_string_pretty(&artifact)?;
    let output = artifact_path(path);
    fs::write(&output, json).with_context(|| format!("failed to write {}", output.display()))?;
    println!("{}", output.display());
    Ok(())
}

fn verify_command(path: &str) -> Result<()> {
    let artifact = read_artifact(path)?;
    verify_artifact(&artifact).map_err(|error| anyhow::anyhow!("error: {error}"))?;
    println!("backend: {}", artifact.backend);
    println!("target: {}", artifact.target);
    println!("compiler: {}", artifact.compiler_version);
    println!("bytecode_bytes: {}", artifact.bytecode.len());
    println!("bytecode_hex: {}", bytecode_hex(&artifact.bytecode));
    println!("bytecode_asm: {}", bytecode_asm(&artifact.bytecode)?);
    println!("finality_depth: {:?}", artifact.finality_depth);
    println!("kip_requirements: {:?}", artifact.kip_requirements);
    for warning in &artifact.warnings {
        println!(
            "warning: {} [{:?}] from {}: {}",
            warning.id, warning.category, warning.citation.path, warning.message
        );
    }
    Ok(())
}

fn inspect_command(path: &str) -> Result<()> {
    if path.ends_with(".artifact") || path.ends_with(".artifact.json") {
        let artifact = read_artifact(path)?;
        println!("{}", serde_json::to_string_pretty(&artifact)?);
        return Ok(());
    }

    let source = fs::read_to_string(path).with_context(|| format!("failed to read {path}"))?;
    let ir = lower_file(&source, path).map_err(|error| anyhow::anyhow!("error: {error}"))?;
    println!("{ir}");
    Ok(())
}

fn kernel_command(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("package") => kernel_package_command(&args[1..]),
        _ => bail!("{}", usage()),
    }
}

fn kernel_package_command(args: &[String]) -> Result<()> {
    let Some(path) = args.first() else {
        bail!("{}", usage());
    };
    let options = KernelPackageOptions::parse(&args[1..])?;
    let source = fs::read_to_string(path).with_context(|| format!("failed to read {path}"))?;
    let package = build_kernel_package(path, &source, &options)?;

    let json = serde_json::to_string_pretty(&package)?;
    let output = options
        .output
        .clone()
        .unwrap_or_else(|| kernel_package_path(path));
    fs::write(&output, json).with_context(|| format!("failed to write {}", output.display()))?;
    println!("{}", output.display());
    Ok(())
}

fn build_kernel_package(
    path: &str,
    source: &str,
    options: &KernelPackageOptions,
) -> Result<CompiledKernelPackage> {
    let artifact = compile_file_for_target(source, path, options.target)
        .map_err(|error| anyhow::anyhow!("error: {error}"))?;
    verify_artifact(&artifact).map_err(|error| anyhow::anyhow!("error: {error}"))?;

    let bytecode_hex = bytecode_hex(&artifact.bytecode);
    let bytecode_asm = bytecode_asm(&artifact.bytecode)?;
    let summary = artifact_summary(&artifact);
    let blueprint = kernel_blueprint_from_artifact(path, &artifact)?;
    let transaction_bytes = options
        .tx_bytes
        .unwrap_or_else(|| u64::try_from(artifact.bytecode.len()).unwrap_or(u64::MAX));
    let fee_assumption = if options.tx_bytes.is_some() || options.compute_grams != 0 {
        "caller-provided fee estimate inputs"
    } else {
        "lower-bound estimate using compiled bytecode length as transaction_bytes and compute_grams=0"
    };
    let package = package_compiled_contract(
        summary,
        bytecode_hex,
        bytecode_asm,
        blueprint,
        options.compute_grams,
        transaction_bytes,
        fee_assumption,
    )
    .map_err(|error| anyhow::anyhow!("error: {error}"))?;

    Ok(package)
}

fn read_artifact(path: &str) -> Result<CompiledArtifact> {
    let json = fs::read_to_string(path).with_context(|| format!("failed to read {path}"))?;
    serde_json::from_str(&json).with_context(|| format!("failed to parse {path}"))
}

fn artifact_path(path: &str) -> std::path::PathBuf {
    Path::new(path).with_extension("artifact.json")
}

fn kernel_package_path(path: &str) -> std::path::PathBuf {
    Path::new(path).with_extension("kernel.json")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct KernelPackageOptions {
    output: Option<std::path::PathBuf>,
    compute_grams: u64,
    tx_bytes: Option<u64>,
    target: Target,
}

impl KernelPackageOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut options = Self {
            output: None,
            compute_grams: 0,
            tx_bytes: None,
            target: Target::VerifiedTn12,
        };
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--output" | "-o" => {
                    index += 1;
                    let value = args
                        .get(index)
                        .ok_or_else(|| anyhow::anyhow!("--output needs a value"))?;
                    options.output = Some(value.into());
                }
                "--compute-grams" => {
                    index += 1;
                    let value = args
                        .get(index)
                        .ok_or_else(|| anyhow::anyhow!("--compute-grams needs a value"))?;
                    options.compute_grams = parse_u64_option("--compute-grams", value)?;
                }
                "--tx-bytes" => {
                    index += 1;
                    let value = args
                        .get(index)
                        .ok_or_else(|| anyhow::anyhow!("--tx-bytes needs a value"))?;
                    options.tx_bytes = Some(parse_u64_option("--tx-bytes", value)?);
                }
                "--target" => {
                    index += 1;
                    let value = args
                        .get(index)
                        .ok_or_else(|| anyhow::anyhow!("--target needs a value"))?;
                    options.target = Target::parse(value).ok_or_else(|| {
                        anyhow::anyhow!(
                            "--target must be one of verified-tn12, tn10-toccata, toccata-preview, future-mainnet"
                        )
                    })?;
                }
                other => bail!("unknown option `{other}`"),
            }
            index += 1;
        }
        Ok(options)
    }
}

fn parse_u64_option(option: &str, value: &str) -> Result<u64> {
    value
        .parse::<u64>()
        .with_context(|| format!("{option} must be a non-negative integer"))
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
    }
}

fn kernel_blueprint_from_artifact(
    source_path: &str,
    artifact: &CompiledArtifact,
) -> Result<ContractBlueprint> {
    let network = network_from_target(&artifact.target);
    let contract_name = if artifact.contracts.len() == 1 {
        artifact.contracts[0].name.clone()
    } else {
        contract_name_from_path_without_cfg(source_path)
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
            let mut transition = Transition::new(&spend.name, TransitionKind::Spend)
                .consumes(format!("{} compiled locking state", contract.name))
                .creates("transaction outputs selected by the spend path")
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

    builder
        .build()
        .map_err(|error| anyhow::anyhow!("error: {error}"))
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

fn contract_name_from_path_without_cfg(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("KaspaScriptPackage")
        .to_owned()
}

#[cfg(any(feature = "tn12-integration", feature = "testnet-integration"))]
fn wallet_command(args: &[String]) -> Result<()> {
    if args.first().map(String::as_str) != Some("balance") {
        bail!("{}", usage());
    }
    let options = CliOptions::parse(&args[1..])?;
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async move {
        let config = options.config()?;
        let rpc = kaspascript_sdk::testnet::TestnetRpcClient::connect(&config).await?;
        let wallet = kaspascript_sdk::testnet::TestWallet::from_env()?;
        let balance = wallet.list_balance(&rpc).await?;
        println!("target: {}", config.target);
        println!("address: {}", wallet.address_string());
        println!("balance_sompi: {balance}");
        rpc.disconnect().await?;
        Ok(())
    })
}

#[cfg(not(any(feature = "tn12-integration", feature = "testnet-integration")))]
fn wallet_command(_args: &[String]) -> Result<()> {
    bail!("wallet commands require --features testnet-integration")
}

#[cfg(any(feature = "tn12-integration", feature = "testnet-integration"))]
fn tx_command(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("lock") => tx_lock_command(&args[1..]),
        Some("spend") => tx_spend_command(&args[1..]),
        _ => bail!("{}", usage()),
    }
}

#[cfg(not(any(feature = "tn12-integration", feature = "testnet-integration")))]
fn tx_command(_args: &[String]) -> Result<()> {
    bail!("tx commands require --features testnet-integration")
}

#[cfg(any(feature = "tn12-integration", feature = "testnet-integration"))]
fn tx_lock_command(args: &[String]) -> Result<()> {
    let Some(path) = args.first() else {
        bail!("{}", usage());
    };
    let options = CliOptions::parse(&args[1..])?;
    let amount = options
        .amount_sompi
        .ok_or_else(|| anyhow::anyhow!("--amount is required for tx lock"))?;
    let source = fs::read_to_string(path).with_context(|| format!("failed to read {path}"))?;
    let contract_name = contract_name_from_path(path);
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async move {
        let config = options.config()?;
        let rpc = kaspascript_sdk::testnet::TestnetRpcClient::connect(&config).await?;
        let wallet = kaspascript_sdk::testnet::TestWallet::from_env()?;
        let harness = kaspascript_sdk::testnet::TestnetContractHarness::new(&rpc, &wallet);
        let proof = harness
            .deploy_and_execute(&contract_name, path, &source, amount, &config)
            .await?;
        println!("target: {}", config.target);
        println!("contract: {}", proof.contract_name);
        println!("result: {:?}", proof.result);
        println!("fee_sompi: {}", proof.fee);
        println!("mass: {}", proof.mass);
        if let Some(lock_txid) = &proof.lock_txid {
            println!("lock_txid: {lock_txid}");
        } else {
            println!("lock_txid: <dry-run>");
        }
        if let Some(spend_txid) = &proof.spend_txid {
            println!("spend_txid: {spend_txid}");
        } else {
            println!("spend_txid: <dry-run>");
        }
        let proof_path = format!(
            "tests/proofs/{}/{}.proof.json",
            config.target, contract_name
        );
        proof.write_json(&proof_path)?;
        println!("proof: {proof_path}");
        rpc.disconnect().await?;
        Ok(())
    })
}

#[cfg(any(feature = "tn12-integration", feature = "testnet-integration"))]
fn tx_spend_command(args: &[String]) -> Result<()> {
    let Some(path) = args.first() else {
        bail!("{}", usage());
    };
    let options = CliOptions::parse(&args[1..])?;
    let artifact = read_artifact(path)?;
    let spend = options
        .spend
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("--spend is required"))?;
    verify_artifact(&artifact).map_err(|error| anyhow::anyhow!("error: {error}"))?;
    println!("artifact: {path}");
    println!("spend: {spend}");
    println!("target: {}", options.target);
    println!("dry_run: {}", !options.broadcast);
    bail!("standalone tx spend requires a lock proof/lock output record; run `kaspascript tx lock <file.ks> ...` for the current end-to-end flow")
}

#[cfg(any(feature = "tn12-integration", feature = "testnet-integration"))]
fn proof_command(args: &[String]) -> Result<()> {
    if args.first().map(String::as_str) != Some("verify") || args.len() != 2 {
        bail!("{}", usage());
    }
    let json =
        fs::read_to_string(&args[1]).with_context(|| format!("failed to read {}", args[1]))?;
    let proof: kaspascript_sdk::testnet::TestnetProof =
        serde_json::from_str(&json).with_context(|| format!("failed to parse {}", args[1]))?;
    if proof.result == kaspascript_sdk::testnet::ProofResult::Pass
        && (proof.lock_txid.is_none() || proof.spend_txid.is_none())
    {
        bail!("pass proof is missing lock_txid or spend_txid");
    }
    println!("target: {}", proof.target);
    println!("contract: {}", proof.contract_name);
    println!("result: {:?}", proof.result);
    println!("source_hash: {}", proof.source_hash);
    println!("artifact_hash: {}", proof.artifact_hash);
    println!("locking_script_hash: {}", proof.locking_script_hash);
    Ok(())
}

#[cfg(not(any(feature = "tn12-integration", feature = "testnet-integration")))]
fn proof_command(_args: &[String]) -> Result<()> {
    bail!("proof commands require --features testnet-integration")
}

#[cfg(any(feature = "tn12-integration", feature = "testnet-integration"))]
#[derive(Debug)]
struct CliOptions {
    target: kaspascript_sdk::testnet::TestnetTarget,
    amount_sompi: Option<u64>,
    spend: Option<String>,
    broadcast: bool,
    rpc_url: Option<String>,
}

#[cfg(any(feature = "tn12-integration", feature = "testnet-integration"))]
impl CliOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut options = Self {
            target: kaspascript_sdk::testnet::TestnetTarget::Tn12,
            amount_sompi: None,
            spend: None,
            broadcast: false,
            rpc_url: None,
        };
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--target" => {
                    index += 1;
                    let value = args
                        .get(index)
                        .ok_or_else(|| anyhow::anyhow!("--target needs a value"))?;
                    options.target = kaspascript_sdk::testnet::TestnetTarget::parse(value)?;
                }
                "--amount" => {
                    index += 1;
                    let value = args
                        .get(index)
                        .ok_or_else(|| anyhow::anyhow!("--amount needs a value"))?;
                    options.amount_sompi = Some(parse_kaspa_amount(value)?);
                }
                "--spend" => {
                    index += 1;
                    options.spend = Some(
                        args.get(index)
                            .ok_or_else(|| anyhow::anyhow!("--spend needs a value"))?
                            .clone(),
                    );
                }
                "--rpc-url" => {
                    index += 1;
                    options.rpc_url = Some(
                        args.get(index)
                            .ok_or_else(|| anyhow::anyhow!("--rpc-url needs a value"))?
                            .clone(),
                    );
                }
                "--broadcast" => options.broadcast = true,
                "--dry-run" => options.broadcast = false,
                other => bail!("unknown option `{other}`"),
            }
            index += 1;
        }
        Ok(options)
    }

    fn config(&self) -> Result<kaspascript_sdk::testnet::TestnetConfig> {
        let mut config = if let Some(rpc_url) = &self.rpc_url {
            kaspascript_sdk::testnet::TestnetConfig::new(self.target, rpc_url.clone())
        } else {
            let mut config = kaspascript_sdk::testnet::TestnetConfig::from_env()?;
            config.target = self.target;
            config
        };
        config.broadcast = self.broadcast;
        Ok(config)
    }
}

#[cfg(any(feature = "tn12-integration", feature = "testnet-integration"))]
fn parse_kaspa_amount(value: &str) -> Result<u64> {
    let (whole, frac) = value.split_once('.').unwrap_or((value, ""));
    let whole = whole.parse::<u64>()?;
    let mut frac_string = frac.to_owned();
    if frac_string.len() > 8 {
        bail!("amount has more than 8 decimal places");
    }
    while frac_string.len() < 8 {
        frac_string.push('0');
    }
    let frac = if frac_string.is_empty() {
        0
    } else {
        frac_string.parse::<u64>()?
    };
    whole
        .checked_mul(100_000_000)
        .and_then(|value| value.checked_add(frac))
        .ok_or_else(|| anyhow::anyhow!("amount overflow"))
}

#[cfg(any(feature = "tn12-integration", feature = "testnet-integration"))]
fn contract_name_from_path(path: &str) -> String {
    Path::new(path)
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
    use kaspascript_kernel::ReadinessLevel;
    use serde_json::Value;

    const KERNEL_GOLDENS: &[(&str, &str, &str)] = &[
        (
            "tests/contracts/escrow.ks",
            include_str!("../../tests/contracts/escrow.ks"),
            include_str!("../../tests/golden/escrow.kernel.json"),
        ),
        (
            "tests/contracts/vault.ks",
            include_str!("../../tests/contracts/vault.ks"),
            include_str!("../../tests/golden/vault.kernel.json"),
        ),
    ];

    #[test]
    fn kernel_package_command_writes_combined_artifact() {
        let dir =
            std::env::temp_dir().join(format!("kaspascript-kernel-cli-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("temp dir");
        let source_path = dir.join("escrow.ks");
        let output_path = dir.join("escrow.kernel.json");
        fs::write(
            &source_path,
            include_str!("../../tests/contracts/escrow.ks"),
        )
        .expect("write source");

        let args = vec![
            source_path.display().to_string(),
            "--output".to_owned(),
            output_path.display().to_string(),
            "--target".to_owned(),
            "verified-tn12".to_owned(),
            "--compute-grams".to_owned(),
            "1000".to_owned(),
            "--tx-bytes".to_owned(),
            "400".to_owned(),
        ];

        kernel_package_command(&args).expect("kernel package command");

        let json = fs::read_to_string(&output_path).expect("read package");
        let package: Value = serde_json::from_str(&json).expect("json");
        assert_eq!(
            package["schema_version"],
            Value::String("kaspascript.kernel.package.v0".to_owned())
        );
        assert_eq!(
            package["package_target"],
            Value::String("verified-tn12".to_owned())
        );
        assert_eq!(
            package["artifact"]["contracts"][0],
            Value::String("Escrow".to_owned())
        );
        assert_eq!(
            package["fee_estimate"]["minimum_standard_fee_sompi"],
            Value::from(100_000)
        );
        assert!(package["bytecode_hex"]
            .as_str()
            .is_some_and(|hex| !hex.is_empty()));
        assert!(package["kernel"]["wallet_previews"]
            .as_array()
            .is_some_and(|previews| !previews.is_empty()));
        assert!(package["kernel"]["readiness"]["ready"]
            .as_bool()
            .expect("readiness bool"));
        assert_eq!(
            package["kernel"]["readiness"]["level"],
            Value::String("verified".to_owned())
        );
        assert_eq!(
            package["kernel"]["capabilities"]["execution_model"],
            Value::String("kaspa-utxo-state-machine".to_owned())
        );
        assert!(package["kernel"]["capabilities"]["transition_profiles"]
            .as_array()
            .is_some_and(|profiles| profiles.len() == 2));
        assert_eq!(
            package["source_snapshots"][0]["tag"],
            Value::String("v2.0.0".to_owned())
        );

        let _ = fs::remove_file(source_path);
        let _ = fs::remove_file(output_path);
        let _ = fs::remove_dir(dir);
    }

    #[test]
    fn kernel_options_parse_fee_inputs() {
        let args = vec![
            "--target".to_owned(),
            "tn10-toccata".to_owned(),
            "--compute-grams".to_owned(),
            "25".to_owned(),
            "--tx-bytes".to_owned(),
            "11".to_owned(),
        ];
        let options = KernelPackageOptions::parse(&args).expect("options");

        assert_eq!(options.compute_grams, 25);
        assert_eq!(options.tx_bytes, Some(11));
        assert_eq!(options.target, Target::Tn10Toccata);
    }

    #[test]
    fn kernel_package_golden_snapshots_match() {
        for (source_path, source, golden) in KERNEL_GOLDENS {
            let options = KernelPackageOptions {
                output: None,
                compute_grams: 1000,
                tx_bytes: Some(400),
                target: Target::VerifiedTn12,
            };

            let package =
                build_kernel_package(source_path, source, &options).expect("kernel package");
            let actual = serde_json::to_string_pretty(&package).expect("json");

            assert_eq!(actual.trim_end(), golden.trim_end(), "{source_path}");
        }
    }

    #[test]
    fn kernel_package_targets_drive_readiness_levels() {
        let source_path = "tests/contracts/escrow.ks";
        let source = include_str!("../../tests/contracts/escrow.ks");
        let options = KernelPackageOptions {
            output: None,
            compute_grams: 1000,
            tx_bytes: Some(400),
            target: Target::ToccataPreview,
        };
        let preview_package =
            build_kernel_package(source_path, source, &options).expect("preview package");

        assert_eq!(preview_package.package_target, "toccata-preview");
        assert_eq!(
            preview_package.kernel.readiness.level,
            ReadinessLevel::Preview
        );

        let options = KernelPackageOptions {
            output: None,
            compute_grams: 1000,
            tx_bytes: Some(400),
            target: Target::FutureMainnet,
        };
        let blocked_package =
            build_kernel_package(source_path, source, &options).expect("future package");

        assert_eq!(blocked_package.package_target, "future-mainnet");
        assert_eq!(
            blocked_package.kernel.readiness.level,
            ReadinessLevel::Blocked
        );
        assert!(!blocked_package.kernel.readiness.ready);
    }
}
