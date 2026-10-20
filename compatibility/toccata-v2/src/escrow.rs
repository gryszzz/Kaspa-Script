//! Two-party TN10 escrow compiled from the checked-in KaspaScript source.
//! Keys and signing remain with the participants. This module cannot submit.

use kaspa_addresses::{Address, Prefix, Version};
use kaspa_consensus_core::{
    config::params::Params,
    constants::LOCK_TIME_THRESHOLD,
    hashing::sighash_type::SIG_HASH_ALL,
    mass::MassCalculator,
    network::{NetworkId, NetworkType},
};
use kaspa_rpc_core::{
    GetServerInfoResponse, RpcFeeEstimate, RpcUtxosByAddressesEntry, api::rpc::RpcApi,
};
use kaspa_txscript::{
    extract_script_pub_key_address, opcodes::codes::*, pay_to_address_script,
    pay_to_script_hash_script, script_builder::ScriptBuilder,
};
use kaspascript_codegen::{
    Target, backends::toccata::compile_spend_script, compile_file_for_target,
};
use kaspascript_ir::{Instruction, InstructionKind, Value};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    InputPlan, NativeTransaction, PlanError, PopulatedTransaction, ScriptPublicKey, TN10_NETWORK,
    Tn10Plan, TransactionOutpoint, TransactionOutput, UtxoEntry,
};

pub const ESCROW_SOURCE: &str = include_str!("../../../contracts/buyer_seller_escrow.ks");

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EscrowTerms {
    /// Hex-encoded x-only Schnorr public keys, never private keys.
    pub buyer_public_key: String,
    pub seller_public_key: String,
    /// Absolute DAA score, below the native timestamp threshold.
    pub refund_daa_score: u64,
    pub max_fee_sompi: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EscrowPath {
    Release,
    Refund,
}

#[derive(Debug, Error)]
pub enum EscrowError {
    #[error("buyer and seller must be different valid x-only Schnorr public keys")]
    InvalidKeys,
    #[error("refund deadline must be a nonzero DAA score below the timestamp threshold")]
    InvalidDeadline,
    #[error("fee must be nonzero, within the agreed cap, and below the locked amount")]
    InvalidFee,
    #[error("UTXO must match this escrow's P2SH script and have no covenant or coinbase flag")]
    WrongUtxo,
    #[error("script construction failed: {0}")]
    Script(String),
    #[error("exactly the required number of 65-byte SIGHASH_ALL signatures is required")]
    InvalidSignatures,
    #[error("only a synced, indexed TN10 node is accepted")]
    WrongNode,
    #[error("node RPC failed: {0}")]
    Rpc(String),
    #[error("selected escrow outpoint is missing or spent in this node snapshot")]
    MissingUtxo,
    #[error("refund deadline has not passed at the observed virtual DAA score")]
    RefundNotDue,
    #[error("native mass calculation failed")]
    Mass,
    #[error(transparent)]
    Plan(#[from] PlanError),
}

pub struct BuyerSellerEscrow {
    terms: EscrowTerms,
    buyer: [u8; 32],
    seller: [u8; 32],
    redeem_script: Vec<u8>,
    script_public_key: ScriptPublicKey,
    address: Address,
    source_hash: String,
}

#[derive(Debug, Serialize)]
pub struct SigningPreview {
    pub network: &'static str,
    pub transaction_version: u16,
    pub path: EscrowPath,
    pub input_outpoint: String,
    pub input_sompi: u64,
    pub recipient: String,
    pub recipient_sompi: u64,
    pub change_sompi: u64,
    pub fee_sompi: u64,
    pub max_fee_sompi: u64,
    pub refund_daa_score: u64,
    pub transaction_lock_time: u64,
    pub input_sequence: u64,
    pub compute_budget: u16,
    pub required_signers: Vec<String>,
    pub source_hash: String,
    pub redeem_script_hex: String,
    pub storage_mass: u64,
    /// Includes correctly sized placeholder witnesses, not actual signatures.
    pub estimated_signed_mass: u64,
    pub broadcast: bool,
    pub fee_policy_verified: bool,
    pub node_acceptance_verified: bool,
}

impl BuyerSellerEscrow {
    pub fn new(terms: EscrowTerms) -> Result<Self, EscrowError> {
        let buyer = parse_key(&terms.buyer_public_key)?;
        let seller = parse_key(&terms.seller_public_key)?;
        if buyer == seller {
            return Err(EscrowError::InvalidKeys);
        }
        if terms.refund_daa_score == 0 || terms.refund_daa_score >= LOCK_TIME_THRESHOLD {
            return Err(EscrowError::InvalidDeadline);
        }
        if terms.max_fee_sompi == 0 || terms.max_fee_sompi > i64::MAX as u64 {
            return Err(EscrowError::InvalidFee);
        }
        let artifact = compile_file_for_target(
            ESCROW_SOURCE,
            "contracts/buyer_seller_escrow.ks",
            Target::Tn10Toccata,
        )
        .map_err(|error| EscrowError::Script(error.to_string()))?;
        let mut escrow = Self {
            terms,
            buyer,
            seller,
            redeem_script: vec![],
            script_public_key: ScriptPublicKey::new(0, vec![].into()),
            address: Address::new(Prefix::Testnet, Version::PubKey, &buyer),
            source_hash: hex::encode(artifact.source_hash),
        };
        // Both paths must share one address; instantiating a single spend would
        // omit the other path and could remove the buyer's recovery option.
        let contract = &artifact.contracts[0];
        let mut branches = Vec::new();
        for name in ["release", "refund"] {
            let spend = contract
                .spends
                .iter()
                .find(|spend| spend.name == name)
                .ok_or_else(|| EscrowError::Script("escrow source is missing a spend".into()))?;
            let instructions = escrow.bind(&spend.instructions)?;
            branches.push(
                compile_spend_script(&instructions)
                    .map_err(|error| EscrowError::Script(error.to_string()))?,
            );
        }
        escrow.redeem_script = ScriptBuilder::new()
            .add_op(OpIf)
            .and_then(|b| b.add_ops(&branches[0]))
            .and_then(|b| b.add_op(OpElse))
            .and_then(|b| b.add_ops(&branches[1]))
            .and_then(|b| b.add_op(OpEndIf))
            .map_err(|error| EscrowError::Script(error.to_string()))?
            .drain();
        escrow.script_public_key = pay_to_script_hash_script(&escrow.redeem_script);
        escrow.address = extract_script_pub_key_address(&escrow.script_public_key, Prefix::Testnet)
            .map_err(|error| EscrowError::Script(error.to_string()))?;
        Ok(escrow)
    }

    pub fn terms(&self) -> &EscrowTerms {
        &self.terms
    }

    pub fn address(&self) -> &Address {
        &self.address
    }

    pub fn redeem_script(&self) -> &[u8] {
        &self.redeem_script
    }

    pub fn script_public_key(&self) -> &ScriptPublicKey {
        &self.script_public_key
    }

    pub fn source_hash(&self) -> &str {
        &self.source_hash
    }

    fn payout(&self, path: EscrowPath) -> Address {
        let key = match path {
            EscrowPath::Release => &self.seller,
            EscrowPath::Refund => &self.buyer,
        };
        Address::new(Prefix::Testnet, Version::PubKey, key)
    }

    // This binder is deliberately scoped to the embedded escrow template, not
    // a general-purpose ABI implementation for arbitrary spend arguments.
    fn bind(&self, instructions: &[Instruction]) -> Result<Vec<Instruction>, EscrowError> {
        let mut result = Vec::new();
        for (index, instruction) in instructions.iter().enumerate() {
            let InstructionKind::Push(Value::Symbol(name)) = &instruction.kind else {
                result.push(instruction.clone());
                continue;
            };
            let value = match name.as_str() {
                "buyer_sig" | "seller_sig" => continue,
                "max_fee" => Value::Integer(self.terms.max_fee_sompi),
                "refund_daa_score" => Value::Integer(self.terms.refund_daa_score),
                "buyer" | "seller" => {
                    let key = if name == "buyer" {
                        self.buyer
                    } else {
                        self.seller
                    };
                    let as_spk = index > 0
                        && matches!(
                            instructions[index - 1].kind,
                            InstructionKind::OutputScript(_)
                        );
                    let bytes = if as_spk {
                        let spk = pay_to_address_script(&Address::new(
                            Prefix::Testnet,
                            Version::PubKey,
                            &key,
                        ));
                        spk.version()
                            .to_be_bytes()
                            .into_iter()
                            .chain(spk.script().iter().copied())
                            .collect()
                    } else {
                        key.to_vec()
                    };
                    Value::Bytes(bytes)
                }
                _ => {
                    return Err(EscrowError::Script(
                        "unexpected escrow source parameter".into(),
                    ));
                }
            };
            result.push(Instruction::new(
                instruction.span,
                InstructionKind::Push(value),
            ));
        }
        Ok(result)
    }

    /// Pure construction from caller-supplied metadata. Network identity and
    /// UTXO liveness are not proven here. Use observe() for a read-only snapshot.
    pub fn unsigned_spend(
        &self,
        network: &str,
        outpoint: TransactionOutpoint,
        utxo: UtxoEntry,
        path: EscrowPath,
        fee_sompi: u64,
        compute_budget: u16,
    ) -> Result<(NativeTransaction, SigningPreview), EscrowError> {
        if network != TN10_NETWORK {
            return Err(PlanError::WrongNetwork.into());
        }
        if utxo.script_public_key != self.script_public_key
            || utxo.covenant_id.is_some()
            || utxo.is_coinbase
            || utxo.amount > i64::MAX as u64
        {
            return Err(EscrowError::WrongUtxo);
        }
        if fee_sompi == 0
            || fee_sompi > self.terms.max_fee_sompi
            || utxo.amount <= self.terms.max_fee_sompi
        {
            return Err(EscrowError::InvalidFee);
        }
        if compute_budget == 0 {
            return Err(EscrowError::Mass);
        }
        let recipient = self.payout(path);
        let recipient_sompi = utxo.amount - fee_sompi;
        let input_sompi = utxo.amount;
        let lock_time = if path == EscrowPath::Refund {
            self.terms.refund_daa_score
        } else {
            0
        };
        let mut native = Tn10Plan {
            network: network.into(),
            inputs: vec![InputPlan {
                outpoint,
                utxo,
                compute_budget,
                sequence: if path == EscrowPath::Refund {
                    0
                } else {
                    u64::MAX
                },
                witness: vec![],
            }],
            outputs: vec![TransactionOutput::new(
                recipient_sompi,
                pay_to_address_script(&recipient),
            )],
            genesis_groups: vec![],
            lock_time,
            storage_mass: 0,
            expected_fee: fee_sompi,
        }
        .build()?;
        let params = Params::from(NetworkId::with_suffix(NetworkType::Testnet, 10));
        let calculator = MassCalculator::new_with_consensus_params(&params);
        let storage = calculator
            .calc_contextual_masses(&PopulatedTransaction::new(
                &native.transaction,
                native.entries.clone(),
            ))
            .ok_or(EscrowError::Mass)?
            .storage_mass;
        native.transaction.set_storage_mass(storage);
        native.transaction.finalize();
        let mut sized = native.transaction.clone();
        let mut placeholder = vec![0; 65];
        placeholder[64] = SIG_HASH_ALL.to_u8();
        sized.inputs[0].signature_script =
            self.signature_script(path, &vec![placeholder; signer_count(path)])?;
        let masses = calculator.calc_non_contextual_masses(&sized);
        let estimated_signed_mass = masses
            .normalized_max(&params.block_mass_limits.cofactors())
            .max((storage as f64 * params.block_mass_limits.cofactors().storage).ceil() as u64);
        let mut required_signers = vec![hex::encode(self.buyer)];
        if path == EscrowPath::Release {
            required_signers.push(hex::encode(self.seller));
        }
        let preview = SigningPreview {
            network: TN10_NETWORK,
            transaction_version: native.transaction.version,
            path,
            input_outpoint: format!("{}:{}", outpoint.transaction_id, outpoint.index),
            input_sompi,
            recipient: recipient.to_string(),
            recipient_sompi,
            change_sompi: 0,
            fee_sompi,
            max_fee_sompi: self.terms.max_fee_sompi,
            refund_daa_score: self.terms.refund_daa_score,
            transaction_lock_time: lock_time,
            input_sequence: native.transaction.inputs[0].sequence,
            compute_budget,
            required_signers,
            source_hash: self.source_hash.clone(),
            redeem_script_hex: hex::encode(&self.redeem_script),
            storage_mass: storage,
            estimated_signed_mass,
            broadcast: false,
            fee_policy_verified: false,
            node_acceptance_verified: false,
        };
        Ok((native, preview))
    }

    /// Encodes externally supplied public signatures; never generates them.
    pub fn signature_script(
        &self,
        path: EscrowPath,
        signatures: &[Vec<u8>],
    ) -> Result<Vec<u8>, EscrowError> {
        if signatures.len() != signer_count(path)
            || signatures
                .iter()
                .any(|sig| sig.len() != 65 || sig[64] != SIG_HASH_ALL.to_u8())
        {
            return Err(EscrowError::InvalidSignatures);
        }
        let mut builder = ScriptBuilder::new();
        for sig in signatures {
            builder
                .add_data(sig)
                .map_err(|error| EscrowError::Script(error.to_string()))?;
        }
        builder
            .add_i64(i64::from(path == EscrowPath::Release))
            .and_then(|b| b.add_data(&self.redeem_script))
            .map_err(|error| EscrowError::Script(error.to_string()))?;
        Ok(builder.drain())
    }
}

fn signer_count(path: EscrowPath) -> usize {
    match path {
        EscrowPath::Release => 2,
        EscrowPath::Refund => 1,
    }
}

fn parse_key(value: &str) -> Result<[u8; 32], EscrowError> {
    let bytes: [u8; 32] = hex::decode(value)
        .map_err(|_| EscrowError::InvalidKeys)?
        .try_into()
        .map_err(|_| EscrowError::InvalidKeys)?;
    secp256k1::XOnlyPublicKey::from_slice(&bytes).map_err(|_| EscrowError::InvalidKeys)?;
    Ok(bytes)
}

pub fn validate_node(info: &GetServerInfoResponse) -> Result<(), EscrowError> {
    if info.network_id != NetworkId::with_suffix(NetworkType::Testnet, 10)
        || !info.is_synced
        || !info.has_utxo_index
    {
        return Err(EscrowError::WrongNode);
    }
    Ok(())
}

/// Point-in-time observations from the supplied node, not acceptance receipts.
pub struct EscrowObservation {
    pub server_info: GetServerInfoResponse,
    pub utxos: Vec<RpcUtxosByAddressesEntry>,
    pub fee_estimate: RpcFeeEstimate,
}

/// Uses an existing native RPC client. No signing or submission API is called.
pub async fn observe(
    rpc: &dyn RpcApi,
    escrow: &BuyerSellerEscrow,
) -> Result<EscrowObservation, EscrowError> {
    let info = rpc
        .get_server_info()
        .await
        .map_err(|e| EscrowError::Rpc(e.to_string()))?;
    validate_node(&info)?;
    let utxos = rpc
        .get_utxos_by_addresses(vec![escrow.address.clone()])
        .await
        .map_err(|e| EscrowError::Rpc(e.to_string()))?;
    let fee_estimate = rpc
        .get_fee_estimate()
        .await
        .map_err(|e| EscrowError::Rpc(e.to_string()))?;
    // Recheck identity after reads as reconnects/provider changes are untrusted.
    let after = rpc
        .get_server_info()
        .await
        .map_err(|e| EscrowError::Rpc(e.to_string()))?;
    validate_node(&after)?;
    Ok(EscrowObservation {
        server_info: after,
        utxos,
        fee_estimate,
    })
}

impl EscrowObservation {
    pub fn unsigned_spend(
        &self,
        escrow: &BuyerSellerEscrow,
        outpoint: TransactionOutpoint,
        path: EscrowPath,
        fee_sompi: u64,
        compute_budget: u16,
    ) -> Result<(NativeTransaction, SigningPreview), EscrowError> {
        validate_node(&self.server_info)?;
        if path == EscrowPath::Refund
            && self.server_info.virtual_daa_score <= escrow.terms.refund_daa_score
        {
            return Err(EscrowError::RefundNotDue);
        }
        let mut matches = self
            .utxos
            .iter()
            .filter(|entry| TransactionOutpoint::from(entry.outpoint) == outpoint);
        let entry = matches.next().ok_or(EscrowError::MissingUtxo)?;
        if matches.next().is_some() {
            return Err(EscrowError::WrongUtxo);
        }
        escrow.unsigned_spend(
            TN10_NETWORK,
            outpoint,
            entry.utxo_entry.clone().into(),
            path,
            fee_sompi,
            compute_budget,
        )
    }
}
