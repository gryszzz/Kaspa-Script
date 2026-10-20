//! Pinned native Toccata construction and offline script validation for TN10.
//! No secret keys, signing, or broadcasting are performed here. The escrow
//! reader can query an existing RPC client without submission authority.

pub mod escrow;

use std::collections::HashSet;

use kaspa_consensus_core::hashing::sighash::SigHashReusedValuesUnsync;
use kaspa_consensus_core::subnets::SUBNETWORK_ID_NATIVE;
pub use kaspa_consensus_core::tx::{
    CovenantBinding, GenesisCovenantGroup, PopulatedTransaction, ScriptPublicKey, Transaction,
    TransactionInput, TransactionOutpoint, TransactionOutput, UtxoEntry,
};
use kaspa_rpc_core::RpcTransaction;
use kaspa_txscript::caches::Cache;
use kaspa_txscript::covenants::CovenantsContext;
use kaspa_txscript::{EngineCtx, TxScriptEngine};
use thiserror::Error;

pub const UPSTREAM_COMMIT: &str = "01b532e8b553523216471682649693af92f0fd16";
pub const TN10_NETWORK: &str = "kaspa-testnet-10";

#[derive(Clone)]
pub struct InputPlan {
    pub outpoint: TransactionOutpoint,
    pub utxo: UtxoEntry,
    pub compute_budget: u16,
    pub sequence: u64,
    /// Caller-supplied witness bytes; no secret material is generated or retained.
    pub witness: Vec<u8>,
}

pub struct Tn10Plan {
    /// The caller must verify this identity against its node before submission.
    pub network: String,
    pub inputs: Vec<InputPlan>,
    /// Includes every recipient and change output explicitly.
    pub outputs: Vec<TransactionOutput>,
    pub genesis_groups: Vec<GenesisCovenantGroup>,
    pub lock_time: u64,
    /// Must come from the wallet/node mass calculation, not a guessed fee model.
    pub storage_mass: u64,
    pub expected_fee: u64,
}

#[derive(Debug, Error)]
pub enum PlanError {
    #[error("only verified kaspa-testnet-10 plans are accepted")]
    WrongNetwork,
    #[error("inputs and outputs must be non-empty")]
    Empty,
    #[error("duplicate input outpoint")]
    DuplicateInput,
    #[error("value sum overflow")]
    Overflow,
    #[error("input metadata count does not match transaction inputs")]
    InputMetadataMismatch,
    #[error("outputs exceed inputs or do not match the explicit fee")]
    FeeMismatch,
    #[error("native covenant validation failed: {0}")]
    Covenant(String),
    #[error("native script validation failed for input {index}: {reason}")]
    Script { index: usize, reason: String },
}

pub struct NativeTransaction {
    pub transaction: Transaction,
    pub entries: Vec<UtxoEntry>,
    pub fee: u64,
}

impl Tn10Plan {
    pub fn build(self) -> Result<NativeTransaction, PlanError> {
        if self.network != TN10_NETWORK {
            return Err(PlanError::WrongNetwork);
        }
        if self.inputs.is_empty() || self.outputs.is_empty() {
            return Err(PlanError::Empty);
        }
        let mut outpoints = HashSet::new();
        if self
            .inputs
            .iter()
            .any(|input| !outpoints.insert(input.outpoint))
        {
            return Err(PlanError::DuplicateInput);
        }
        let total_in = self.inputs.iter().try_fold(0u64, |sum, input| {
            sum.checked_add(input.utxo.amount)
                .ok_or(PlanError::Overflow)
        })?;
        let total_out = self.outputs.iter().try_fold(0u64, |sum, output| {
            sum.checked_add(output.value).ok_or(PlanError::Overflow)
        })?;
        let fee = total_in
            .checked_sub(total_out)
            .ok_or(PlanError::FeeMismatch)?;
        if fee != self.expected_fee {
            return Err(PlanError::FeeMismatch);
        }
        let entries = self.inputs.iter().map(|input| input.utxo.clone()).collect();
        let inputs = self
            .inputs
            .into_iter()
            .map(|input| {
                TransactionInput::new_with_compute_budget(
                    input.outpoint,
                    input.witness,
                    input.sequence,
                    input.compute_budget,
                )
            })
            .collect();
        let mut transaction = Transaction::new_with_mass(
            1,
            inputs,
            self.outputs,
            self.lock_time,
            SUBNETWORK_ID_NATIVE,
            0,
            vec![],
            self.storage_mass,
        );
        transaction
            .populate_genesis_covenants(&self.genesis_groups)
            .map_err(|error| PlanError::Covenant(error.to_string()))?;
        transaction.finalize();
        let native = NativeTransaction {
            transaction,
            entries,
            fee,
        };
        CovenantsContext::from_tx(&PopulatedTransaction::new(
            &native.transaction,
            native.entries.clone(),
        ))
        .map_err(|error| PlanError::Covenant(error.to_string()))?;
        Ok(native)
    }
}

impl NativeTransaction {
    /// Validates scripts with the real v2.1.0 engine. This is not mempool,
    /// fee-policy, DAG acceptance, or live UTXO verification.
    pub fn validate_scripts(&self) -> Result<(), PlanError> {
        if self.entries.len() != self.transaction.inputs.len() {
            return Err(PlanError::InputMetadataMismatch);
        }
        let tx = PopulatedTransaction::new(&self.transaction, self.entries.clone());
        let context = CovenantsContext::from_tx(&tx)
            .map_err(|error| PlanError::Covenant(error.to_string()))?;
        let cache = Cache::new(0);
        let reused = SigHashReusedValuesUnsync::new();
        for (index, input) in self.transaction.inputs.iter().enumerate() {
            TxScriptEngine::from_transaction_input(
                &tx,
                input,
                index,
                &self.entries[index],
                EngineCtx::new(&cache)
                    .with_reused(&reused)
                    .with_covenants_ctx(&context),
                Default::default(),
            )
            .execute()
            .map_err(|error| PlanError::Script {
                index,
                reason: error.to_string(),
            })?;
        }
        Ok(())
    }

    pub fn rpc_transaction(&self) -> RpcTransaction {
        RpcTransaction::from(&self.transaction)
    }
}
