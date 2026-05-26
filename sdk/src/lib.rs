//! KaspaScript SDK surface.

#[cfg(any(feature = "tn12-integration", feature = "testnet-integration"))]
pub mod testnet;
#[cfg(any(feature = "tn12-integration", feature = "testnet-integration"))]
pub mod tn12;

use kaspascript_codegen::{compile_file, CompiledArtifact};
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

/// Compiles KaspaScript source.
pub fn compile(src: &str, file: &str) -> Result<CompiledArtifact, CompileError> {
    compile_file(src, file)
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
}
