use std::env;
use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use kaspascript_codegen::{bytecode_asm, bytecode_hex, verify_artifact, CompiledArtifact};
use kaspascript_ir::lower_file;
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
        "wallet" => wallet_command(&args[2..]),
        "tx" => tx_command(&args[2..]),
        "proof" => proof_command(&args[2..]),
        other => bail!("unknown command `{other}`"),
    }
}

fn usage() -> &'static str {
    "usage: kaspascript <compile|verify|inspect> <file>\n       kaspascript wallet balance --target tn12\n       kaspascript tx lock <file.ks> --target tn12 --amount 1.0 [--dry-run|--broadcast]\n       kaspascript tx spend <artifact.json> --spend <name> --target tn12 [--dry-run|--broadcast]\n       kaspascript proof verify <proof.json>"
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

fn read_artifact(path: &str) -> Result<CompiledArtifact> {
    let json = fs::read_to_string(path).with_context(|| format!("failed to read {path}"))?;
    serde_json::from_str(&json).with_context(|| format!("failed to parse {path}"))
}

fn artifact_path(path: &str) -> std::path::PathBuf {
    Path::new(path).with_extension("artifact.json")
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
