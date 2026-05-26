use std::env;
use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use kaspascript_codegen::{verify_artifact, CompiledArtifact};
use kaspascript_ir::lower_file;
use kaspascript_sdk::compile;

fn main() -> Result<()> {
    let args = env::args().collect::<Vec<_>>();
    if args.len() != 3 {
        bail!("usage: kaspascript <compile|verify|inspect> <file>");
    }

    match args[1].as_str() {
        "compile" => compile_command(&args[2]),
        "verify" => verify_command(&args[2]),
        "inspect" => inspect_command(&args[2]),
        other => bail!("unknown command `{other}`"),
    }
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
    println!("compiler: {}", artifact.compiler_version);
    println!("bytecode_bytes: {}", artifact.bytecode.len());
    println!("finality_depth: {:?}", artifact.finality_depth);
    println!("kip_requirements: {:?}", artifact.kip_requirements);
    Ok(())
}

fn inspect_command(path: &str) -> Result<()> {
    if path.ends_with(".artifact") {
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
    Path::new(path).with_extension("artifact")
}
