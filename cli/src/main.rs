use std::env;
use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use kaspascript_codegen::{
    bytecode_asm, bytecode_hex, compile_file_for_target, verify_artifact, CompiledArtifact, Target,
};
use kaspascript_ir::lower_file;
use kaspascript_kernel::{
    current_source_snapshots, current_toccata_evidence, CompiledKernelPackage, ToccataFeePolicy,
};
use kaspascript_sdk::{
    build_kernel_package as build_sdk_kernel_package, KernelPackageBuildOptions,
};
use serde_json::{json, Value};

const CLI_BRIEF: &str = "KaspaScript is a source-grounded Kaspa contract compiler and programmability kernel for target-gated txscript bytecode, Toccata readiness, wallet previews, indexer schemas, and fee-aware package metadata.";

fn main() -> Result<()> {
    let args = env::args().collect::<Vec<_>>();
    if args.len() < 2 {
        bail!("{}", usage());
    }

    match args[1].as_str() {
        "--help" | "-h" | "help" => {
            println!("{}", help());
            Ok(())
        }
        "--brief" | "brief" => {
            println!("{CLI_BRIEF}");
            Ok(())
        }
        "--version" | "-V" | "version" => {
            println!("kaspascript {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "compile" => compile_command(&args[2..]),
        "verify" if args.len() == 3 => verify_command(&args[2]),
        "inspect" => inspect_command(&args[2..]),
        "kernel" => kernel_command(&args[2..]),
        "toccata" => toccata_command(&args[2..]),
        "doctor" => kernel_check_command(&args[2..]),
        "wallet" => wallet_command(&args[2..]),
        "tx" => tx_command(&args[2..]),
        "proof" => proof_command(&args[2..]),
        other => bail!("unknown command `{other}`"),
    }
}

fn usage() -> &'static str {
    "usage: kaspascript compile <file.ks> [--target verified-tn12|tn10-toccata|toccata-preview|future-mainnet] [--output <file>]\n       kaspascript verify <artifact.json>\n       kaspascript inspect <file.ks|artifact.json> [--json]\n       kaspascript kernel package <file.ks> [--target verified-tn12|tn10-toccata|toccata-preview|future-mainnet] [--output <file>] [--compute-grams <n>] [--tx-bytes <n>]\n       kaspascript kernel check <file.ks> [--target <target>] [--compute-grams <n>] [--tx-bytes <n>] [--json]\n       kaspascript kernel preview <file.ks> [--target <target>] [--transition <name>] [--json]\n       kaspascript toccata status [--json]\n       kaspascript toccata targets [--json]\n       kaspascript toccata fee --compute-grams <n> --tx-bytes <n> [--json]\n       kaspascript doctor <file.ks> [--target <target>] [--json]\n       kaspascript wallet balance --target tn12\n       kaspascript tx lock <file.ks> --target tn12 --amount 1.0 [--dry-run|--broadcast]\n       kaspascript tx spend <artifact.json> --spend <name> --target tn12 [--dry-run|--broadcast]\n       kaspascript proof verify <proof.json>"
}

fn help() -> String {
    format!(
        "{CLI_BRIEF}\n\n{}\n\nKaspa-native workflow:\n  1. kaspascript toccata status\n  2. kaspascript compile contract.ks --target verified-tn12\n  3. kaspascript kernel check contract.ks --target verified-tn12\n  4. kaspascript kernel preview contract.ks --transition <spend>\n  5. kaspascript kernel package contract.ks --target verified-tn12 --compute-grams 1000 --tx-bytes 400\n\nTargets:\n  verified-tn12     source-grounded TN12 txscript subset\n  tn10-toccata      TN10 Toccata readiness posture\n  toccata-preview   preview-only analysis for gated Toccata features\n  future-mainnet    blocked until mainnet activation evidence is verified",
        usage()
    )
}

fn compile_command(args: &[String]) -> Result<()> {
    let Some(path) = args.first() else {
        bail!("{}", usage());
    };
    let options = CompileOptions::parse(&args[1..])?;
    let source = fs::read_to_string(path).with_context(|| format!("failed to read {path}"))?;
    let artifact = compile_file_for_target(&source, path, options.target)
        .map_err(|error| anyhow::anyhow!("error: {error}"))?;
    verify_artifact(&artifact).map_err(|error| anyhow::anyhow!("error: {error}"))?;
    let json = serde_json::to_string_pretty(&artifact)?;
    let output = options
        .output
        .clone()
        .unwrap_or_else(|| artifact_path(path));
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

fn inspect_command(args: &[String]) -> Result<()> {
    let Some(path) = args.first() else {
        bail!("{}", usage());
    };
    let format = parse_report_format(&args[1..])?;
    if path.ends_with(".artifact") || path.ends_with(".artifact.json") {
        let artifact = read_artifact(path)?;
        match format {
            OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&artifact)?),
            OutputFormat::Human => {
                println!("artifact target: {}", artifact.target);
                println!("backend: {}", artifact.backend);
                print_application_model(&artifact.application);
            }
        }
        return Ok(());
    }

    let source = fs::read_to_string(path).with_context(|| format!("failed to read {path}"))?;
    let ir = lower_file(&source, path).map_err(|error| anyhow::anyhow!("error: {error}"))?;
    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&ir.application)?),
        OutputFormat::Human => println!("{ir}"),
    }
    Ok(())
}

fn kernel_command(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("package") => kernel_package_command(&args[1..]),
        Some("check") => kernel_check_command(&args[1..]),
        Some("preview") => kernel_preview_command(&args[1..]),
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

fn kernel_check_command(args: &[String]) -> Result<()> {
    let Some(path) = args.first() else {
        bail!("{}", usage());
    };
    let options = KernelReportOptions::parse(&args[1..])?;
    let package = build_kernel_package_from_path(path, &options.package_options())?;

    match options.format {
        OutputFormat::Json => {
            let report = kernel_check_report(path, &package);
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        OutputFormat::Human => print_kernel_check_human(path, &package),
    }

    Ok(())
}

fn kernel_preview_command(args: &[String]) -> Result<()> {
    let Some(path) = args.first() else {
        bail!("{}", usage());
    };
    let options = KernelReportOptions::parse(&args[1..])?;
    let package = build_kernel_package_from_path(path, &options.package_options())?;
    let previews = package
        .kernel
        .wallet_previews
        .iter()
        .filter(|preview| {
            options
                .transition
                .as_ref()
                .map_or(true, |transition| preview.transition == *transition)
        })
        .collect::<Vec<_>>();

    if previews.is_empty() {
        let transition = options.transition.as_deref().unwrap_or("<any transition>");
        bail!("no wallet preview matched transition `{transition}`");
    }

    match options.format {
        OutputFormat::Json => {
            let report = kernel_preview_report(&package, &previews);
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        OutputFormat::Human => print_kernel_preview_human(&package, &previews),
    }

    Ok(())
}

fn build_kernel_package_from_path(
    path: &str,
    options: &KernelPackageOptions,
) -> Result<CompiledKernelPackage> {
    let source = fs::read_to_string(path).with_context(|| format!("failed to read {path}"))?;
    build_kernel_package(path, &source, options)
}

fn build_kernel_package(
    path: &str,
    source: &str,
    options: &KernelPackageOptions,
) -> Result<CompiledKernelPackage> {
    build_sdk_kernel_package(source, path, options.sdk_options())
        .map_err(|error| anyhow::anyhow!("error: {error}"))
}

fn toccata_command(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("status") | None => toccata_status_command(args.get(1..).unwrap_or(&[])),
        Some("targets") => toccata_targets_command(&args[1..]),
        Some("fee") => toccata_fee_command(&args[1..]),
        _ => bail!("{}", usage()),
    }
}

fn toccata_status_command(args: &[String]) -> Result<()> {
    let format = parse_report_format(args)?;
    let report = toccata_status_report();
    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
        OutputFormat::Human => print_toccata_status_human(&report),
    }
    Ok(())
}

fn toccata_targets_command(args: &[String]) -> Result<()> {
    let format = parse_report_format(args)?;
    let targets = target_matrix();
    match format {
        OutputFormat::Json => {
            let report = toccata_targets_report(targets);
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        OutputFormat::Human => print_target_matrix_human(&targets),
    }
    Ok(())
}

fn toccata_fee_command(args: &[String]) -> Result<()> {
    let options = ToccataFeeOptions::parse(args)?;
    let estimate = ToccataFeePolicy::default()
        .estimate(
            options.compute_grams,
            options.tx_bytes,
            "caller-provided Toccata fee estimate inputs",
        )
        .map_err(|error| anyhow::anyhow!("error: {error}"))?;

    match options.format {
        OutputFormat::Json => {
            let report = toccata_fee_report(&estimate);
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        OutputFormat::Human => {
            println!("policy: {}", estimate.policy);
            println!("source: {}", estimate.source);
            println!("compute_grams: {}", estimate.compute_grams);
            println!("transaction_bytes: {}", estimate.transaction_bytes);
            println!(
                "minimum_standard_fee_sompi: {}",
                estimate.minimum_standard_fee_sompi
            );
            println!("formula: max(compute_grams, tx_bytes * 2) * 100 sompi");
        }
    }

    Ok(())
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
struct CompileOptions {
    output: Option<std::path::PathBuf>,
    target: Target,
}

impl CompileOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut options = Self {
            output: None,
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
                "--target" => {
                    index += 1;
                    let value = args
                        .get(index)
                        .ok_or_else(|| anyhow::anyhow!("--target needs a value"))?;
                    options.target = parse_target_option(value)?;
                }
                other => bail!("unknown option `{other}`"),
            }
            index += 1;
        }
        Ok(options)
    }
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
                    options.target = parse_target_option(value)?;
                }
                other => bail!("unknown option `{other}`"),
            }
            index += 1;
        }
        Ok(options)
    }

    fn sdk_options(&self) -> KernelPackageBuildOptions {
        let mut options =
            KernelPackageBuildOptions::new(self.target).with_compute_grams(self.compute_grams);
        if let Some(tx_bytes) = self.tx_bytes {
            options = options.with_tx_bytes(tx_bytes);
        }
        options
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    Human,
    Json,
}

impl OutputFormat {
    fn parse_flag(flag: &str, current: &mut Self) -> bool {
        match flag {
            "--json" | "--agent" => {
                *current = Self::Json;
                true
            }
            "--human" => {
                *current = Self::Human;
                true
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct KernelReportOptions {
    compute_grams: u64,
    tx_bytes: Option<u64>,
    target: Target,
    format: OutputFormat,
    transition: Option<String>,
}

impl KernelReportOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut options = Self {
            compute_grams: 0,
            tx_bytes: None,
            target: Target::VerifiedTn12,
            format: OutputFormat::Human,
            transition: None,
        };
        let mut index = 0;
        while index < args.len() {
            if OutputFormat::parse_flag(args[index].as_str(), &mut options.format) {
                index += 1;
                continue;
            }

            match args[index].as_str() {
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
                    options.target = parse_target_option(value)?;
                }
                "--transition" => {
                    index += 1;
                    options.transition = Some(
                        args.get(index)
                            .ok_or_else(|| anyhow::anyhow!("--transition needs a value"))?
                            .clone(),
                    );
                }
                other => bail!("unknown option `{other}`"),
            }
            index += 1;
        }
        Ok(options)
    }

    fn package_options(&self) -> KernelPackageOptions {
        KernelPackageOptions {
            output: None,
            compute_grams: self.compute_grams,
            tx_bytes: self.tx_bytes,
            target: self.target,
        }
    }
}

fn parse_target_option(value: &str) -> Result<Target> {
    Target::parse(value).ok_or_else(|| {
        anyhow::anyhow!(
            "--target must be one of verified-tn12, tn10-toccata, toccata-preview, future-mainnet"
        )
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ToccataFeeOptions {
    compute_grams: u64,
    tx_bytes: u64,
    format: OutputFormat,
}

impl ToccataFeeOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut compute_grams = None;
        let mut tx_bytes = None;
        let mut format = OutputFormat::Human;
        let mut index = 0;
        while index < args.len() {
            if OutputFormat::parse_flag(args[index].as_str(), &mut format) {
                index += 1;
                continue;
            }

            match args[index].as_str() {
                "--compute-grams" => {
                    index += 1;
                    let value = args
                        .get(index)
                        .ok_or_else(|| anyhow::anyhow!("--compute-grams needs a value"))?;
                    compute_grams = Some(parse_u64_option("--compute-grams", value)?);
                }
                "--tx-bytes" => {
                    index += 1;
                    let value = args
                        .get(index)
                        .ok_or_else(|| anyhow::anyhow!("--tx-bytes needs a value"))?;
                    tx_bytes = Some(parse_u64_option("--tx-bytes", value)?);
                }
                other => bail!("unknown option `{other}`"),
            }
            index += 1;
        }

        Ok(Self {
            compute_grams: compute_grams
                .ok_or_else(|| anyhow::anyhow!("--compute-grams is required"))?,
            tx_bytes: tx_bytes.ok_or_else(|| anyhow::anyhow!("--tx-bytes is required"))?,
            format,
        })
    }
}

fn parse_report_format(args: &[String]) -> Result<OutputFormat> {
    let mut format = OutputFormat::Human;
    for arg in args {
        if !OutputFormat::parse_flag(arg, &mut format) {
            bail!("unknown option `{arg}`");
        }
    }
    Ok(format)
}

fn parse_u64_option(option: &str, value: &str) -> Result<u64> {
    value
        .parse::<u64>()
        .with_context(|| format!("{option} must be a non-negative integer"))
}

fn kernel_check_report(path: &str, package: &CompiledKernelPackage) -> Value {
    json!({
        "schema_version": "kaspascript.cli.kernel.check.v0",
        "contract": package.kernel.readiness.contract,
        "target": package.package_target,
        "artifact": &package.artifact,
        "readiness": &package.kernel.readiness,
        "capabilities": &package.kernel.capabilities,
        "fee_estimate": &package.fee_estimate,
        "next_commands": [
            format!("kaspascript kernel preview {path} --target {}", package.package_target),
            format!("kaspascript kernel package {path} --target {} --compute-grams {} --tx-bytes {}", package.package_target, package.fee_estimate.compute_grams, package.fee_estimate.transaction_bytes)
        ]
    })
}

fn kernel_preview_report(
    package: &CompiledKernelPackage,
    previews: &[&kaspascript_kernel::WalletPreview],
) -> Value {
    json!({
        "schema_version": "kaspascript.cli.kernel.preview.v0",
        "contract": package.kernel.readiness.contract,
        "target": package.package_target,
        "previews": previews,
    })
}

fn toccata_targets_report(targets: Vec<Value>) -> Value {
    json!({
        "schema_version": "kaspascript.cli.toccata.targets.v0",
        "targets": targets,
    })
}

fn toccata_fee_report(estimate: &kaspascript_kernel::FeeEstimate) -> Value {
    json!({
        "schema_version": "kaspascript.cli.toccata.fee.v0",
        "fee_estimate": estimate,
        "formula": "max(compute_grams, tx_bytes * 2) * 100 sompi",
    })
}

fn toccata_status_report() -> Value {
    json!({
        "schema_version": "kaspascript.cli.toccata.status.v0",
        "upgrade": toccata_upgrade_profile(),
        "source_snapshots": current_source_snapshots(),
        "evidence": current_toccata_evidence(),
        "targets": target_matrix(),
        "recommended_commands": [
            "kaspascript toccata targets",
            "kaspascript kernel check <contract.ks> --target verified-tn12",
            "kaspascript kernel preview <contract.ks> --target verified-tn12",
            "kaspascript kernel package <contract.ks> --target verified-tn12 --compute-grams 1000 --tx-bytes 400"
        ],
    })
}

fn toccata_upgrade_profile() -> Value {
    json!({
        "name": "Toccata",
        "rusty_kaspa_release": {
            "repo": "https://github.com/kaspanet/rusty-kaspa",
            "tag": "v2.0.1",
            "name": "Mainnet Toccata Release - v2.0.1",
            "published_at": "2026-06-15T19:14:22Z",
            "commit": "cfafeb4c093fa37a303f1b9f19c58f986b870ce3",
            "upgrade_note": "drop-in update for v2.0.0 nodes and Toccata upgrade version for pre-Toccata 1.x nodes",
        },
        "baseline_toccata_release": {
            "tag": "v2.0.0",
            "published_at": "2026-06-05T12:09:13Z",
            "commit": "90dbf074275d60c1fe74a3491883196f110970c0",
            "role": "baseline release that announced the mainnet activation DAA score",
        },
        "node_upgrade_guide": {
            "url": "https://github.com/kaspanet/rusty-kaspa/blob/v2.0.1/docs/toccata-guide.md",
            "database_upgrade": "one-way; downgrade requires resync",
            "recommended_rehearsal_network": "testnet-10",
        },
        "release_assets": [
            {
                "name": "kaspa-wasm32-sdk-v2.0.1.zip",
                "url": "https://github.com/kaspanet/rusty-kaspa/releases/download/v2.0.1/kaspa-wasm32-sdk-v2.0.1.zip",
            },
            {
                "name": "rusty-kaspa-v2.0.1-linux-amd64.zip",
                "url": "https://github.com/kaspanet/rusty-kaspa/releases/download/v2.0.1/rusty-kaspa-v2.0.1-linux-amd64.zip",
            },
            {
                "name": "rusty-kaspa-v2.0.1-osx.zip",
                "url": "https://github.com/kaspanet/rusty-kaspa/releases/download/v2.0.1/rusty-kaspa-v2.0.1-osx.zip",
            },
            {
                "name": "rusty-kaspa-v2.0.1-win64.zip",
                "url": "https://github.com/kaspanet/rusty-kaspa/releases/download/v2.0.1/rusty-kaspa-v2.0.1-win64.zip",
            },
        ],
        "maintenance_changes": [
            "seq-commit state and lane-proof RPC support",
            "wallet/core and Wasm SMT sync progress notifications",
            "SMT database inspection tooling",
            "transaction-generation tooling for user-lane workflows",
            "covenant binding handling refinements across client and wallet components",
            "Wasm client transaction v0 deserialization fix",
        ],
        "mainnet_activation": {
            "daa_score": 474_165_565u64,
            "estimated_utc": "2026-06-30T16:15:00Z",
            "status": "scheduled-not-independently-verified",
            "kaspa_script_readiness": "blocked-for-production-mainnet",
        },
        "p2p_protocol": {
            "required_version": 10,
            "restriction_window": "24h-before-activation",
        },
        "node_requirements": {
            "minimum": {
                "cpu_cores": "8",
                "ram_gb": 16,
                "storage": "640 GB SSD",
                "network": "10 MB/s or about 80 Mbit/s",
            },
            "preferred": {
                "cpu_cores": "12-16",
                "ram_gb": 32,
                "storage": "1 TB SSD",
                "network": "higher sustained bandwidth",
            },
            "rationale": "Toccata doubles the transient mass limit to support ZK-STARK proof workloads.",
        },
        "fee_policy": {
            "formula": "100 sompi * max(compute grams, 2 * transaction bytes)",
            "minimum_fee_rate_change": {
                "pre_toccata_sompi_per_gram": 1,
                "toccata_sompi_per_gram": 100,
            },
            "normalized_transient_mass": "2 * transaction bytes",
            "pre_activation_rpc": "direct RPC transaction submission applies the higher minimum standard fee",
            "pre_activation_p2p": "P2P relay keeps the pre-activation policy until activation",
            "post_activation": "RPC and P2P relay reject transactions below the higher minimum standard fee",
            "consensus": "standard fee policy is not consensus; zero-fee transactions remain consensus-valid",
        },
        "transaction_format": {
            "transaction_version": 1,
            "new_fields": [
                "TransactionOutput.covenant",
                "TransactionInput.compute_commit",
            ],
            "pool_stratum_requirement": "preserve the new fields from GetBlockTemplate through SubmitBlock",
            "mass_field": {
                "rust_protobuf": "storage_mass",
                "json_js": "storageMass",
                "deprecated_alias": "mass",
            },
        },
        "kips": [
            {
                "kip": 16,
                "title": "New Transaction Opcodes for Verifiable Computation",
                "status": "Proposed; implemented and activated in TN10",
                "kaspa_script_integration": "gate OpZkPrecompile 0xa6 until proof ABI fixtures and pricing tests are pinned",
                "source": "https://github.com/kaspanet/kips/blob/master/kip-0016.md",
            },
            {
                "kip": 17,
                "title": "Covenants and Improved Scripting Capabilities",
                "status": "Implemented and activated in TN10",
                "kaspa_script_integration": "model full transaction introspection, byte-string ops, keyed hashes, and post-activation script limits behind Toccata targets",
                "source": "https://github.com/kaspanet/kips/blob/master/kip-0017.md",
            },
            {
                "kip": 20,
                "title": "Covenant IDs",
                "status": "Proposed; implemented and activated in TN10",
                "kaspa_script_integration": "emit covenant lineage metadata now; lower covenant binding opcodes only after transaction builder support lands",
                "source": "https://github.com/kaspanet/kips/blob/master/kip-0020.md",
            },
            {
                "kip": 21,
                "title": "Partitioned Sequencing Commitment with O(activity) Proving",
                "status": "Implemented and activated in TN10",
                "kaspa_script_integration": "treat OpChainblockSeqCommit as a future proof-bearing transition primitive with lane witness requirements",
                "source": "https://github.com/kaspanet/kips/blob/master/kip-0021.md",
            },
        ],
        "integrator_actions": [
            "Upgrade nodes to rusty-kaspa v2.0.1 before the scheduled activation window.",
            "Run full wallet, explorer, miner, pool, and indexer rehearsal on Testnet-10.",
            "Add compatibility coverage for seq-commit lane-proof RPCs and covenant binding refinements.",
            "Update gRPC/protobuf integrations for transaction version 1 covenant and compute_commit fields.",
            "Use node fee estimation when available; otherwise apply the Toccata minimum standard fee formula.",
            "Prefer storage_mass/storageMass in new JSON and protobuf integrations; treat mass as deprecated compatibility only.",
            "Keep KaspaScript future-mainnet packages blocked until activation evidence is independently verified.",
        ],
    })
}

fn target_matrix() -> Vec<Value> {
    vec![
        json!({
            "target": "verified-tn12",
            "readiness": "verified",
            "network": "tn12",
            "use": "deterministic txscript packages for the source-grounded V1 subset",
            "allows_gated_warnings": false,
            "production_mainnet": false,
            "recommended_for": ["golden tests", "wallet preview integration", "indexer schema integration"],
        }),
        json!({
            "target": "tn10-toccata",
            "readiness": "verified",
            "network": "tn10",
            "use": "Toccata testnet posture for upgrade compatibility checks",
            "allows_gated_warnings": true,
            "production_mainnet": false,
            "recommended_for": ["Toccata app design", "covenant readiness analysis", "testnet package review"],
        }),
        json!({
            "target": "toccata-preview",
            "readiness": "preview",
            "network": "unknown",
            "use": "analysis surface for recognized but not fully lowered Toccata features",
            "allows_gated_warnings": true,
            "production_mainnet": false,
            "recommended_for": ["architecture planning", "backend ABI TODO discovery", "agent review"],
        }),
        json!({
            "target": "future-mainnet",
            "readiness": "blocked",
            "network": "mainnet",
            "use": "future gate that remains blocked until mainnet activation and lowering evidence are verified",
            "allows_gated_warnings": false,
            "production_mainnet": false,
            "recommended_for": ["release readiness checks only"],
        }),
    ]
}

fn print_toccata_status_human(report: &Value) {
    let upgrade = &report["upgrade"];
    println!("upgrade: {}", upgrade["name"].as_str().unwrap_or("Toccata"));
    println!(
        "rusty_kaspa_release: {} ({})",
        upgrade["rusty_kaspa_release"]["tag"]
            .as_str()
            .unwrap_or("unknown"),
        upgrade["rusty_kaspa_release"]["published_at"]
            .as_str()
            .unwrap_or("unknown")
    );
    println!(
        "mainnet_activation: DAA {} estimated {}",
        upgrade["mainnet_activation"]["daa_score"]
            .as_u64()
            .unwrap_or_default(),
        upgrade["mainnet_activation"]["estimated_utc"]
            .as_str()
            .unwrap_or("unknown")
    );
    println!(
        "kaspa_script_readiness: {}",
        upgrade["mainnet_activation"]["kaspa_script_readiness"]
            .as_str()
            .unwrap_or("unknown")
    );
    println!(
        "guide: {}",
        upgrade["node_upgrade_guide"]["url"]
            .as_str()
            .unwrap_or("unknown")
    );
    println!(
        "fee_policy: {}",
        upgrade["fee_policy"]["formula"]
            .as_str()
            .unwrap_or("unknown")
    );
    println!("targets:");
    if let Some(targets) = report["targets"].as_array() {
        for target in targets {
            println!(
                "- {}: {} ({})",
                target["target"].as_str().unwrap_or("unknown"),
                target["readiness"].as_str().unwrap_or("unknown"),
                target["use"].as_str().unwrap_or("no description")
            );
        }
    }
}

fn print_target_matrix_human(targets: &[Value]) {
    for target in targets {
        println!("target: {}", target["target"].as_str().unwrap_or("unknown"));
        println!(
            "  readiness: {}",
            target["readiness"].as_str().unwrap_or("unknown")
        );
        println!(
            "  network: {}",
            target["network"].as_str().unwrap_or("unknown")
        );
        println!("  use: {}", target["use"].as_str().unwrap_or("unknown"));
        println!(
            "  allows_gated_warnings: {}",
            target["allows_gated_warnings"]
                .as_bool()
                .unwrap_or_default()
        );
        println!(
            "  production_mainnet: {}",
            target["production_mainnet"].as_bool().unwrap_or_default()
        );
    }
}

fn print_kernel_check_human(path: &str, package: &CompiledKernelPackage) {
    println!("contract: {}", package.kernel.readiness.contract);
    println!("source: {path}");
    println!("target: {}", package.package_target);
    println!(
        "readiness: {}",
        readiness_label(package.kernel.readiness.level)
    );
    println!("ready: {}", package.kernel.readiness.ready);
    println!("bytecode_bytes: {}", package.artifact.bytecode_bytes);
    println!(
        "minimum_standard_fee_sompi: {}",
        package.fee_estimate.minimum_standard_fee_sompi
    );

    if !package.kernel.readiness.blockers.is_empty() {
        println!("blockers:");
        for blocker in &package.kernel.readiness.blockers {
            println!("- {blocker}");
        }
    }

    println!("features:");
    for feature in &package.kernel.readiness.features {
        let best = feature
            .best
            .map(|level| level.to_string())
            .unwrap_or_else(|| "unknown".to_owned());
        let source = feature.source_label.as_deref().unwrap_or("none");
        println!(
            "- {}.{} requires {} at {}, best {}, level {}, source {}",
            package.kernel.readiness.contract,
            feature.transition,
            feature.feature,
            feature.required,
            best,
            readiness_label(feature.level),
            source
        );
    }

    println!("next:");
    println!(
        "- kaspascript kernel preview {path} --target {}",
        package.package_target
    );
    println!(
        "- kaspascript kernel package {path} --target {} --compute-grams {} --tx-bytes {}",
        package.package_target,
        package.fee_estimate.compute_grams,
        package.fee_estimate.transaction_bytes
    );
}

fn print_kernel_preview_human(
    package: &CompiledKernelPackage,
    previews: &[&kaspascript_kernel::WalletPreview],
) {
    println!("contract: {}", package.kernel.readiness.contract);
    println!("target: {}", package.package_target);
    for preview in previews {
        println!("transition: {}", preview.transition);
        println!("  classification: {:?}", preview.classification);
        println!("  network: {}", preview.network);
        println!("  consumes: {}", join_strings(&preview.consumes));
        println!("  creates: {}", join_strings(&preview.creates));
        println!("  signers: {}", join_strings(&preview.signers));
        if let Some(semantics) = &preview.semantics {
            println!(
                "  transaction inputs: {:?}",
                semantics.transaction_shape.referenced_inputs
            );
            println!(
                "  transaction outputs: {:?}",
                semantics.transaction_shape.referenced_outputs
            );
            println!(
                "  additional inputs/outputs permitted: {}/{}",
                semantics.transaction_shape.additional_inputs_permitted,
                semantics.transaction_shape.additional_outputs_permitted
            );
            println!(
                "  exact inputs/outputs: {:?}/{:?}",
                semantics.transaction_shape.exact_input_count,
                semantics.transaction_shape.exact_output_count
            );
            println!(
                "  continuation: {:?} ({})",
                semantics.continuation.kind, semantics.continuation.note
            );
            if !semantics.continuation.named_successor_outputs.is_empty() {
                println!(
                    "  named continuation outputs: {}",
                    semantics
                        .continuation
                        .named_successor_outputs
                        .iter()
                        .map(|output| format!("{}=output({})", output.name, output.output_index))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            println!("  constraints:");
            for constraint in &semantics.constraints {
                println!("  - {:?}: {}", constraint.kind, constraint.expression);
            }
        }
        if !preview.warnings.is_empty() {
            println!("  warnings:");
            for warning in &preview.warnings {
                println!("  - {warning}");
            }
        }
    }
}

fn print_application_model(application: &kaspascript_model::ApplicationModel) {
    println!("application schema: {}", application.schema_version);
    println!("execution model: {:?}", application.execution_model);
    for contract in &application.contracts {
        println!("contract: {}", contract.name);
        if let Some(depth) = contract.finality_depth {
            println!("  finality depth: {depth}");
        }
        println!("  state:");
        for state in &contract.state {
            println!("  - {}: {:?}", state.name, state.ty);
        }
        for transition in &contract.transitions {
            println!("  transition: {}", transition.name);
            if transition.signing_requirements.is_empty() {
                println!("    signing: no recognized signature requirement");
            } else {
                for signing in &transition.signing_requirements {
                    println!(
                        "    signing: {:?}, threshold {}, keys [{}], signatures [{}]",
                        signing.scheme,
                        signing.threshold,
                        signing.authorized_keys.join(", "),
                        signing.signature_arguments.join(", ")
                    );
                }
            }
            println!(
                "    referenced inputs/outputs: {:?}/{:?}",
                transition.transaction_shape.referenced_inputs,
                transition.transaction_shape.referenced_outputs
            );
            println!(
                "    additional inputs/outputs permitted: {}/{}",
                transition.transaction_shape.additional_inputs_permitted,
                transition.transaction_shape.additional_outputs_permitted
            );
            println!(
                "    exact inputs/outputs: {:?}/{:?}",
                transition.transaction_shape.exact_input_count,
                transition.transaction_shape.exact_output_count
            );
            for constraint in &transition.constraints {
                println!(
                    "    require [{:?}]: {}",
                    constraint.kind, constraint.expression
                );
            }
            println!(
                "    continuation: {:?} ({})",
                transition.continuation.kind, transition.continuation.note
            );
            if !transition.continuation.named_successor_outputs.is_empty() {
                println!(
                    "    named continuation outputs: {}",
                    transition
                        .continuation
                        .named_successor_outputs
                        .iter()
                        .map(|output| format!("{}=output({})", output.name, output.output_index))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            println!(
                "    monetary: fees/change external-explicit; compiler injects outputs/recipients: {}/{}",
                transition.monetary_policy.compiler_injects_outputs,
                transition.monetary_policy.compiler_injects_recipients
            );
        }
    }
    println!("compiler guarantees:");
    for assurance in &application.assurances.compiler_guarantees {
        println!("- {}: {}", assurance.id, assurance.statement);
    }
    println!("external obligations:");
    for assurance in &application.assurances.external_obligations {
        println!(
            "- {:?} {}: {}",
            assurance.actor, assurance.id, assurance.statement
        );
    }
}

fn readiness_label(level: kaspascript_kernel::ReadinessLevel) -> &'static str {
    match level {
        kaspascript_kernel::ReadinessLevel::Verified => "verified",
        kaspascript_kernel::ReadinessLevel::Preview => "preview",
        kaspascript_kernel::ReadinessLevel::Blocked => "blocked",
    }
}

fn join_strings(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_owned()
    } else {
        values.join(", ")
    }
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

    const KERNEL_GOLDENS: &[(&str, &str, Target, &str)] = &[
        (
            "tests/contracts/escrow.ks",
            include_str!("../../tests/contracts/escrow.ks"),
            Target::VerifiedTn12,
            include_str!("../../tests/golden/escrow.kernel.json"),
        ),
        (
            "tests/contracts/vault.ks",
            include_str!("../../tests/contracts/vault.ks"),
            Target::VerifiedTn12,
            include_str!("../../tests/golden/vault.kernel.json"),
        ),
        (
            "tests/contracts/escrow.ks",
            include_str!("../../tests/contracts/escrow.ks"),
            Target::Tn10Toccata,
            include_str!("../../tests/golden/escrow.tn10.kernel.json"),
        ),
        (
            "tests/contracts/vault.ks",
            include_str!("../../tests/contracts/vault.ks"),
            Target::Tn10Toccata,
            include_str!("../../tests/golden/vault.tn10.kernel.json"),
        ),
    ];

    const CLI_REPORT_SCHEMAS: &[(&str, &str, &str)] = &[
        (
            "kaspascript.cli.toccata.status.v0",
            include_str!("../../docs/schemas/kaspascript.cli.toccata.status.v0.schema.json"),
            include_str!("../../tests/golden/cli/toccata.status.json"),
        ),
        (
            "kaspascript.cli.toccata.targets.v0",
            include_str!("../../docs/schemas/kaspascript.cli.toccata.targets.v0.schema.json"),
            include_str!("../../tests/golden/cli/toccata.targets.json"),
        ),
        (
            "kaspascript.cli.toccata.fee.v0",
            include_str!("../../docs/schemas/kaspascript.cli.toccata.fee.v0.schema.json"),
            include_str!("../../tests/golden/cli/toccata.fee.json"),
        ),
        (
            "kaspascript.cli.kernel.check.v0",
            include_str!("../../docs/schemas/kaspascript.cli.kernel.check.v0.schema.json"),
            include_str!("../../tests/golden/cli/kernel.check.escrow.verified-tn12.json"),
        ),
        (
            "kaspascript.cli.kernel.preview.v0",
            include_str!("../../docs/schemas/kaspascript.cli.kernel.preview.v0.schema.json"),
            include_str!("../../tests/golden/cli/kernel.preview.escrow.release.verified-tn12.json"),
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
            Value::String("v2.0.1".to_owned())
        );
        assert_eq!(
            package["source_snapshots"][1]["tag"],
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
    fn compile_command_accepts_target_and_output() {
        let dir =
            std::env::temp_dir().join(format!("kaspascript-compile-cli-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("temp dir");
        let source_path = dir.join("escrow.ks");
        let output_path = dir.join("escrow.tn10.artifact.json");
        fs::write(
            &source_path,
            include_str!("../../tests/contracts/escrow.ks"),
        )
        .expect("write source");

        let args = vec![
            source_path.display().to_string(),
            "--target".to_owned(),
            "tn10-toccata".to_owned(),
            "--output".to_owned(),
            output_path.display().to_string(),
        ];

        compile_command(&args).expect("compile command");

        let json = fs::read_to_string(&output_path).expect("read artifact");
        let artifact: Value = serde_json::from_str(&json).expect("artifact json");
        assert_eq!(artifact["target"], Value::String("tn10-toccata".to_owned()));
        assert!(artifact["bytecode"]
            .as_array()
            .is_some_and(|bytes| !bytes.is_empty()));

        let _ = fs::remove_file(source_path);
        let _ = fs::remove_file(output_path);
        let _ = fs::remove_dir(dir);
    }

    #[test]
    fn kernel_report_options_parse_agent_flags() {
        let args = vec![
            "--target".to_owned(),
            "toccata-preview".to_owned(),
            "--transition".to_owned(),
            "release".to_owned(),
            "--json".to_owned(),
            "--compute-grams".to_owned(),
            "55".to_owned(),
            "--tx-bytes".to_owned(),
            "123".to_owned(),
        ];
        let options = KernelReportOptions::parse(&args).expect("report options");

        assert_eq!(options.target, Target::ToccataPreview);
        assert_eq!(options.transition.as_deref(), Some("release"));
        assert_eq!(options.format, OutputFormat::Json);
        assert_eq!(options.compute_grams, 55);
        assert_eq!(options.tx_bytes, Some(123));
    }

    #[test]
    fn toccata_status_marks_future_mainnet_blocked() {
        let report = toccata_status_report();

        assert_eq!(
            report["upgrade"]["mainnet_activation"]["daa_score"],
            Value::from(474_165_565u64)
        );
        assert_eq!(
            report["upgrade"]["mainnet_activation"]["kaspa_script_readiness"],
            Value::String("blocked-for-production-mainnet".to_owned())
        );
        assert_eq!(
            report["upgrade"]["rusty_kaspa_release"]["tag"],
            Value::String("v2.0.1".to_owned())
        );
        assert_eq!(
            report["upgrade"]["baseline_toccata_release"]["tag"],
            Value::String("v2.0.0".to_owned())
        );
        assert_eq!(
            report["upgrade"]["node_upgrade_guide"]["url"],
            Value::String(
                "https://github.com/kaspanet/rusty-kaspa/blob/v2.0.1/docs/toccata-guide.md"
                    .to_owned()
            )
        );
        assert_eq!(
            report["upgrade"]["fee_policy"]["formula"],
            Value::String("100 sompi * max(compute grams, 2 * transaction bytes)".to_owned())
        );
        assert_eq!(
            report["upgrade"]["transaction_format"]["mass_field"]["json_js"],
            Value::String("storageMass".to_owned())
        );
        assert_eq!(report["upgrade"]["kips"].as_array().expect("kips").len(), 4);
        let targets = report["targets"].as_array().expect("targets");
        let future = targets
            .iter()
            .find(|target| target["target"] == Value::String("future-mainnet".to_owned()))
            .expect("future target");
        assert_eq!(future["readiness"], Value::String("blocked".to_owned()));
        assert_eq!(future["production_mainnet"], Value::Bool(false));
    }

    #[test]
    fn toccata_fee_options_require_inputs() {
        let args = vec![
            "--compute-grams".to_owned(),
            "1000".to_owned(),
            "--tx-bytes".to_owned(),
            "400".to_owned(),
            "--json".to_owned(),
        ];
        let options = ToccataFeeOptions::parse(&args).expect("fee options");

        assert_eq!(options.compute_grams, 1000);
        assert_eq!(options.tx_bytes, 400);
        assert_eq!(options.format, OutputFormat::Json);
        assert!(ToccataFeeOptions::parse(&["--tx-bytes".to_owned(), "400".to_owned()]).is_err());
    }

    #[test]
    fn cli_report_schema_files_are_valid_json() {
        for (schema_version, schema, golden) in CLI_REPORT_SCHEMAS {
            let schema_json: Value = serde_json::from_str(schema).expect("schema json");
            let golden_json: Value = serde_json::from_str(golden).expect("golden json");

            assert_eq!(
                schema_json["properties"]["schema_version"]["const"],
                Value::String((*schema_version).to_owned()),
                "{schema_version}"
            );
            assert_eq!(
                golden_json["schema_version"],
                Value::String((*schema_version).to_owned()),
                "{schema_version}"
            );
            assert_eq!(
                schema_json["$schema"],
                Value::String("https://json-schema.org/draft/2020-12/schema".to_owned()),
                "{schema_version}"
            );
        }
    }

    #[test]
    fn kernel_package_schema_tracks_the_canonical_application_model() {
        let schema: Value = serde_json::from_str(include_str!(
            "../../docs/schemas/kaspascript.kernel.package.v0.schema.json"
        ))
        .expect("kernel package schema");
        let package: Value =
            serde_json::from_str(include_str!("../../tests/golden/escrow.kernel.json"))
                .expect("kernel package golden");

        assert_eq!(
            schema["properties"]["schema_version"]["const"],
            Value::String("kaspascript.kernel.package.v0".to_owned())
        );
        assert_eq!(
            package["kernel"]["application"]["schema_version"],
            Value::String("kaspascript.application.v0".to_owned())
        );
        assert_eq!(
            package["artifact"]["application_schema_version"],
            Value::String("kaspascript.application.v0".to_owned())
        );
    }

    #[test]
    fn cli_report_golden_snapshots_match() {
        assert_report_snapshot(
            "toccata.status",
            toccata_status_report(),
            include_str!("../../tests/golden/cli/toccata.status.json"),
        );

        assert_report_snapshot(
            "toccata.targets",
            toccata_targets_report(target_matrix()),
            include_str!("../../tests/golden/cli/toccata.targets.json"),
        );

        let fee = ToccataFeePolicy::default()
            .estimate(1000, 400, "caller-provided Toccata fee estimate inputs")
            .expect("fee estimate");
        assert_report_snapshot(
            "toccata.fee",
            toccata_fee_report(&fee),
            include_str!("../../tests/golden/cli/toccata.fee.json"),
        );

        let package = escrow_kernel_package();
        assert_report_snapshot(
            "kernel.check.escrow.verified-tn12",
            kernel_check_report("tests/contracts/escrow.ks", &package),
            include_str!("../../tests/golden/cli/kernel.check.escrow.verified-tn12.json"),
        );

        let previews = package
            .kernel
            .wallet_previews
            .iter()
            .filter(|preview| preview.transition == "release")
            .collect::<Vec<_>>();
        assert_report_snapshot(
            "kernel.preview.escrow.release.verified-tn12",
            kernel_preview_report(&package, &previews),
            include_str!("../../tests/golden/cli/kernel.preview.escrow.release.verified-tn12.json"),
        );
    }

    #[test]
    fn kernel_package_golden_snapshots_match() {
        for (source_path, source, target, golden) in KERNEL_GOLDENS {
            let options = KernelPackageOptions {
                output: None,
                compute_grams: 1000,
                tx_bytes: Some(400),
                target: *target,
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

    fn escrow_kernel_package() -> CompiledKernelPackage {
        let options = KernelPackageOptions {
            output: None,
            compute_grams: 1000,
            tx_bytes: Some(400),
            target: Target::VerifiedTn12,
        };
        build_kernel_package(
            "tests/contracts/escrow.ks",
            include_str!("../../tests/contracts/escrow.ks"),
            &options,
        )
        .expect("escrow package")
    }

    fn assert_report_snapshot(name: &str, actual: Value, golden: &str) {
        let expected: Value = serde_json::from_str(golden).expect("golden json");
        assert_eq!(actual, expected, "{name}");
    }
}
