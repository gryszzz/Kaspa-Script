//! Source-grounded Kaspa testnet transaction integration.
//!
//! The transaction structures, signing hash, P2PK/P2SH scripts, script engine,
//! RPC types, and mass calculation are all delegated to Rusty Kaspa crates.

use std::collections::{BTreeMap, HashSet};
use std::env;
use std::fmt;
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use kaspa_addresses::{Address, Prefix, Version};
use kaspa_consensus_client::{
    TransactionOutpoint as ClientTransactionOutpoint, UtxoEntry as ClientUtxoEntry,
    UtxoEntryReference,
};
use kaspa_consensus_core::config::params::Params;
use kaspa_consensus_core::constants::{MAX_TX_IN_SEQUENCE_NUM, TX_VERSION};
use kaspa_consensus_core::hashing::sighash::SigHashReusedValues;
use kaspa_consensus_core::hashing::sighash_type::SIG_HASH_ALL;
use kaspa_consensus_core::network::{NetworkId, NetworkType};
use kaspa_consensus_core::sign::sign_input;
use kaspa_consensus_core::subnets::SUBNETWORK_ID_NATIVE;
use kaspa_consensus_core::tx::{
    PopulatedTransaction, ScriptPublicKey, SignableTransaction, Transaction, TransactionInput,
    TransactionOutpoint, TransactionOutput, UtxoEntry,
};
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_rpc_core::prelude::{
    RpcFeeEstimate, RpcMempoolEntry, RpcTransaction, RpcTransactionId, RpcUtxosByAddressesEntry,
};
use kaspa_txscript::caches::Cache;
use kaspa_txscript::script_builder::ScriptBuilder;
use kaspa_txscript::{
    extract_script_pub_key_address, get_sig_op_count, pay_to_address_script,
    pay_to_script_hash_script, SigCacheKey, TxScriptEngine,
};
use kaspa_wallet_core::tx::mass::{MassCalculator, MAXIMUM_STANDARD_TRANSACTION_MASS};
use kaspa_wallet_core::utxo::NetworkParams;
use kaspa_wrpc_client::{KaspaRpcClient, WrpcEncoding};
use kaspascript_codegen::backends::toccata::compile_instruction_sequence;
use kaspascript_codegen::{
    bytecode_hex, compile_file, verify_artifact, ArtifactContract, ArtifactParam, ArtifactSpend,
    CompiledArtifact,
};
use kaspascript_ir::{Instruction, InstructionKind, Value};
use kaspascript_lexer::TypeName;
use secp256k1::{Keypair, Secp256k1, SecretKey, XOnlyPublicKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Environment variable containing `tn10`, `tn11`, or `tn12`.
pub const KASPA_TARGET_ENV: &str = "KASPA_TARGET";
/// Environment variable containing a testnet wRPC URL.
pub const KASPA_RPC_URL_ENV: &str = "KASPA_RPC_URL";
/// Environment variable containing a 32-byte testnet private key in hex.
pub const KASPA_TESTNET_PRIVATE_KEY_ENV: &str = "KASPA_TESTNET_PRIVATE_KEY";
/// Optional environment variable containing the expected derived testnet address.
pub const KASPA_TESTNET_ADDRESS_ENV: &str = "KASPA_TESTNET_ADDRESS";
/// Backwards-compatible TN12 URL variable.
pub const TN12_RPC_URL_ENV: &str = "KASPA_TN12_RPC_URL";
/// Backwards-compatible TN12 private-key variable.
pub const TN12_PRIVATE_KEY_ENV: &str = "KASPA_TN12_PRIVATE_KEY";
/// Backwards-compatible TN12 faucet/address variable.
pub const TN12_FAUCET_ADDRESS_ENV: &str = "KASPA_TN12_FAUCET_ADDRESS";
/// TN12 network ID string.
pub const TN12_NETWORK_ID: &str = "testnet-12";

const DEFAULT_POLL_ATTEMPTS: usize = 120;
const DEFAULT_POLL_INTERVAL_MS: u64 = 1_000;
const SOMPI_PER_KASPA: u64 = 100_000_000;

/// Supported live testnet targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TestnetTarget {
    /// Kaspa testnet 10. Rusty Kaspa exposes suffix-specific consensus params for this target.
    Tn10,
    /// Kaspa testnet 11. Network ID is source-supported; suffix-specific params are gated.
    Tn11,
    /// Kaspa testnet 12. Network ID is source-supported; suffix-specific params are gated.
    Tn12,
}

impl TestnetTarget {
    /// Parses a target label.
    pub fn parse(value: &str) -> Result<Self, TestnetError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "tn10" | "testnet-10" => Ok(Self::Tn10),
            "tn11" | "testnet-11" => Ok(Self::Tn11),
            "tn12" | "testnet-12" => Ok(Self::Tn12),
            "mainnet" | "kaspa" => Err(TestnetError::MainnetRejected),
            other => Err(TestnetError::InvalidTarget(other.to_owned())),
        }
    }

    /// Returns the Rusty Kaspa network suffix.
    pub const fn suffix(self) -> u32 {
        match self {
            Self::Tn10 => 10,
            Self::Tn11 => 11,
            Self::Tn12 => 12,
        }
    }

    /// Returns the Rusty Kaspa network id.
    pub fn network_id(self) -> NetworkId {
        NetworkId::with_suffix(NetworkType::Testnet, self.suffix())
    }

    /// Returns the node-reported network string.
    pub fn network_name(self) -> String {
        self.network_id().to_string()
    }

    /// Returns target warnings grounded in the pinned Rusty Kaspa source.
    pub fn warnings(self) -> Vec<String> {
        match self {
            Self::Tn10 => Vec::new(),
            Self::Tn11 | Self::Tn12 => vec![
                "rusty-kaspa NetworkId accepts arbitrary testnet suffixes, but consensus Params::from(NetworkId) in consensus/core/src/config/params.rs is suffix-specific only for testnet-10; this builder uses generic NetworkType::Testnet params for fee/mass and records this as target-gated".to_owned(),
            ],
        }
    }
}

impl fmt::Display for TestnetTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Tn10 => "tn10",
            Self::Tn11 => "tn11",
            Self::Tn12 => "tn12",
        })
    }
}

/// Testnet integration error.
#[derive(Debug, Error)]
pub enum TestnetError {
    /// A required environment variable is missing.
    #[error("missing environment variable `{0}`")]
    MissingEnv(&'static str),
    /// An environment variable was present but invalid.
    #[error("invalid environment variable `{var}`: {message}")]
    InvalidEnv { var: &'static str, message: String },
    /// The requested network is not a supported safe testnet target.
    #[error("invalid testnet target `{0}`")]
    InvalidTarget(String),
    /// Mainnet is rejected by default.
    #[error("refusing mainnet transaction construction without an explicit unsafe path")]
    MainnetRejected,
    /// The connected node reports a different network than requested.
    #[error("network mismatch: expected `{expected}`, got `{actual}`")]
    NetworkMismatch { expected: String, actual: String },
    /// Broadcast was requested without explicit permission.
    #[error("broadcast disabled; pass --broadcast for live submission")]
    BroadcastDisabled,
    /// The connected node cannot serve indexed UTXO queries.
    #[error("connected node does not expose the UTXO index")]
    UtxoIndexDisabled,
    /// No spendable UTXO matched the request.
    #[error("insufficient funds: need at least {needed} sompi, available {available} sompi")]
    InsufficientFunds { needed: u64, available: u64 },
    /// The live RPC call failed.
    #[error("Kaspa RPC error: {0}")]
    Rpc(String),
    /// The KaspaScript compiler rejected the contract.
    #[error("compile error: {0}")]
    Compile(String),
    /// The artifact is malformed for transaction construction.
    #[error("artifact error: {0}")]
    Artifact(String),
    /// Parameter binding failed.
    #[error("parameter error: {0}")]
    Param(String),
    /// Transaction construction failed.
    #[error("transaction error: {0}")]
    Tx(String),
    /// Script validation failed.
    #[error("script validation error: {0}")]
    Script(String),
    /// JSON serialization or deserialization failed.
    #[error("JSON error: {0}")]
    Json(String),
    /// Filesystem IO failed.
    #[error("IO error: {0}")]
    Io(String),
    /// A requested operation is intentionally unsupported for source-grounded safety.
    #[error("unsupported operation: {0}")]
    Unsupported(String),
    /// Raw data supplied to the harness is invalid.
    #[error("invalid data: {0}")]
    InvalidData(String),
}

/// Runtime configuration for live testnet operations.
#[derive(Clone)]
pub struct TestnetConfig {
    /// Testnet target.
    pub target: TestnetTarget,
    /// wRPC URL.
    pub rpc_url: String,
    /// Number of status-poll attempts used by live tests.
    pub poll_attempts: usize,
    /// Delay between status-poll attempts.
    pub poll_interval: Duration,
    /// Whether this process may broadcast transactions.
    pub broadcast: bool,
}

impl fmt::Debug for TestnetConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TestnetConfig")
            .field("target", &self.target)
            .field("rpc_url", &self.rpc_url)
            .field("poll_attempts", &self.poll_attempts)
            .field("poll_interval", &self.poll_interval)
            .field("broadcast", &self.broadcast)
            .finish()
    }
}

impl TestnetConfig {
    /// Loads testnet configuration from environment variables.
    pub fn from_env() -> Result<Self, TestnetError> {
        let target = env::var(KASPA_TARGET_ENV)
            .ok()
            .as_deref()
            .map(TestnetTarget::parse)
            .transpose()?
            .unwrap_or(TestnetTarget::Tn12);
        let rpc_url = read_required_env_with_fallback(KASPA_RPC_URL_ENV, TN12_RPC_URL_ENV)?;
        let poll_attempts =
            read_optional_usize("KASPA_TESTNET_POLL_ATTEMPTS")?.unwrap_or(DEFAULT_POLL_ATTEMPTS);
        let poll_interval_ms = read_optional_u64("KASPA_TESTNET_POLL_INTERVAL_MS")?
            .unwrap_or(DEFAULT_POLL_INTERVAL_MS);
        let broadcast = read_optional_bool("KASPA_BROADCAST")?.unwrap_or(false);

        Ok(Self {
            target,
            rpc_url,
            poll_attempts,
            poll_interval: Duration::from_millis(poll_interval_ms),
            broadcast,
        })
    }

    /// Creates a config directly.
    pub fn new(target: TestnetTarget, rpc_url: impl Into<String>) -> Self {
        Self {
            target,
            rpc_url: rpc_url.into(),
            poll_attempts: DEFAULT_POLL_ATTEMPTS,
            poll_interval: Duration::from_millis(DEFAULT_POLL_INTERVAL_MS),
            broadcast: false,
        }
    }

    /// Returns a copy with broadcast enabled or disabled.
    pub fn with_broadcast(mut self, broadcast: bool) -> Self {
        self.broadcast = broadcast;
        self
    }
}

/// Live network metadata returned by a testnet node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NetworkInfo {
    /// Network identifier reported by the node.
    pub network: String,
    /// Node implementation/version string.
    pub node_version: String,
    /// RPC API version.
    pub rpc_api_version: u16,
    /// RPC API revision.
    pub rpc_api_revision: u16,
    /// Whether the node reports it is synced.
    pub is_synced: bool,
    /// Whether the node has the UTXO index enabled.
    pub has_utxo_index: bool,
    /// Current virtual DAA score.
    pub virtual_daa_score: u64,
    /// Block count reported by BlockDAG info.
    pub block_count: u64,
    /// Header count reported by BlockDAG info.
    pub header_count: u64,
}

/// Simplified UTXO view used by testnet wallet selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestnetUtxo {
    /// Transaction outpoint in `txid:index` form.
    pub outpoint: String,
    /// Amount in sompi.
    pub amount: u64,
    /// Block DAA score where the UTXO was accepted.
    pub block_daa_score: u64,
    /// Estimated confirmation count from the node virtual DAA score.
    pub confirmations: u64,
    /// Whether the UTXO is coinbase.
    pub is_coinbase: bool,
    /// Script public key bytes as lowercase hex.
    pub script_public_key_hex: String,
}

/// Indexed output observed after live broadcast.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedOutput {
    /// Transaction id.
    pub transaction_id: String,
    /// Output index.
    pub output_index: u32,
    /// Amount in sompi.
    pub amount: u64,
    /// Block DAA score where the output was accepted.
    pub block_daa_score: u64,
    /// Current observed DAA-score depth.
    pub observed_depth: u64,
}

/// Fee estimate snapshot normalized for proof files and logs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeeEstimateSnapshot {
    /// Priority bucket feerate in sompi/gram.
    pub priority_feerate: f64,
    /// Normal priority feerate buckets in sompi/gram.
    pub normal_feerates: Vec<f64>,
    /// Low priority feerate buckets in sompi/gram.
    pub low_feerates: Vec<f64>,
}

impl From<RpcFeeEstimate> for FeeEstimateSnapshot {
    fn from(value: RpcFeeEstimate) -> Self {
        Self {
            priority_feerate: value.priority_bucket.feerate,
            normal_feerates: value
                .normal_buckets
                .into_iter()
                .map(|bucket| bucket.feerate)
                .collect(),
            low_feerates: value
                .low_buckets
                .into_iter()
                .map(|bucket| bucket.feerate)
                .collect(),
        }
    }
}

/// Transaction status as observed by the RPC adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionStatus {
    /// The transaction is currently visible in the node mempool.
    Mempool {
        /// Fee reported by the mempool entry.
        fee: u64,
        /// Whether the transaction is in the orphan pool.
        is_orphan: bool,
    },
    /// The node did not return a status.
    Unknown {
        /// RPC message returned by the node.
        message: String,
    },
}

impl From<RpcMempoolEntry> for TransactionStatus {
    fn from(entry: RpcMempoolEntry) -> Self {
        Self::Mempool {
            fee: entry.fee,
            is_orphan: entry.is_orphan,
        }
    }
}

/// Live wRPC adapter.
pub struct TestnetRpcClient {
    target: TestnetTarget,
    client: KaspaRpcClient,
}

impl fmt::Debug for TestnetRpcClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TestnetRpcClient")
            .field("target", &self.target)
            .finish_non_exhaustive()
    }
}

impl TestnetRpcClient {
    /// Creates a testnet RPC client without connecting.
    pub fn new(config: &TestnetConfig) -> Result<Self, TestnetError> {
        let client = KaspaRpcClient::new(
            WrpcEncoding::Borsh,
            Some(config.rpc_url.as_str()),
            None,
            Some(config.target.network_id()),
            None,
        )
        .map_err(|error| TestnetError::Rpc(error.to_string()))?;

        Ok(Self {
            target: config.target,
            client,
        })
    }

    /// Connects to the configured node and hard-fails unless it is the requested target.
    pub async fn connect(config: &TestnetConfig) -> Result<Self, TestnetError> {
        let adapter = Self::new(config)?;
        adapter
            .client
            .connect(None)
            .await
            .map_err(|error| TestnetError::Rpc(error.to_string()))?;
        adapter.assert_target().await?;
        Ok(adapter)
    }

    /// Disconnects the underlying wRPC client.
    pub async fn disconnect(&self) -> Result<(), TestnetError> {
        self.client
            .disconnect()
            .await
            .map_err(|error| TestnetError::Rpc(error.to_string()))
    }

    /// Fetches network and node metadata.
    pub async fn network_info(&self) -> Result<NetworkInfo, TestnetError> {
        let server = self
            .client
            .get_server_info()
            .await
            .map_err(|error| TestnetError::Rpc(error.to_string()))?;
        let dag = self
            .client
            .get_block_dag_info()
            .await
            .map_err(|error| TestnetError::Rpc(error.to_string()))?;

        Ok(NetworkInfo {
            network: server.network_id.to_string(),
            node_version: server.server_version,
            rpc_api_version: server.rpc_api_version,
            rpc_api_revision: server.rpc_api_revision,
            is_synced: server.is_synced,
            has_utxo_index: server.has_utxo_index,
            virtual_daa_score: dag.virtual_daa_score,
            block_count: dag.block_count,
            header_count: dag.header_count,
        })
    }

    /// Hard-fails if the connected node is not the configured target.
    pub async fn assert_target(&self) -> Result<NetworkInfo, TestnetError> {
        let info = self.network_info().await?;
        let expected = self.target.network_name();
        if info.network == expected {
            Ok(info)
        } else {
            Err(TestnetError::NetworkMismatch {
                expected,
                actual: info.network,
            })
        }
    }

    /// Backwards-compatible TN12 assertion.
    pub async fn assert_tn12(&self) -> Result<NetworkInfo, TestnetError> {
        let info = self.network_info().await?;
        if info.network == TN12_NETWORK_ID {
            Ok(info)
        } else {
            Err(TestnetError::NetworkMismatch {
                expected: TN12_NETWORK_ID.to_owned(),
                actual: info.network,
            })
        }
    }

    /// Fetches all indexed UTXOs for an address.
    pub async fn fetch_utxos(&self, address: &Address) -> Result<Vec<TestnetUtxo>, TestnetError> {
        let info = self.assert_target().await?;
        if !info.has_utxo_index {
            return Err(TestnetError::UtxoIndexDisabled);
        }
        let entries = self.fetch_rpc_utxos(address).await?;
        Ok(entries
            .into_iter()
            .map(|entry| normalize_utxo(entry, info.virtual_daa_score))
            .collect())
    }

    /// Fetches typed Rusty Kaspa RPC UTXOs for transaction construction.
    pub async fn fetch_rpc_utxos(
        &self,
        address: &Address,
    ) -> Result<Vec<RpcUtxosByAddressesEntry>, TestnetError> {
        let info = self.assert_target().await?;
        if !info.has_utxo_index {
            return Err(TestnetError::UtxoIndexDisabled);
        }
        self.client
            .get_utxos_by_addresses(vec![address.clone()])
            .await
            .map_err(|error| TestnetError::Rpc(error.to_string()))
    }

    /// Fetches the indexed balance for an address.
    pub async fn balance(&self, address: &Address) -> Result<u64, TestnetError> {
        let info = self.assert_target().await?;
        if !info.has_utxo_index {
            return Err(TestnetError::UtxoIndexDisabled);
        }

        self.client
            .get_balance_by_address(address.clone())
            .await
            .map_err(|error| TestnetError::Rpc(error.to_string()))
    }

    /// Fetches the node fee estimate, returning `None` when the node does not expose it.
    pub async fn estimate_fees_if_supported(
        &self,
    ) -> Result<Option<FeeEstimateSnapshot>, TestnetError> {
        match self.client.get_fee_estimate().await {
            Ok(estimate) => Ok(Some(estimate.into())),
            Err(error) => {
                let message = error.to_string();
                let lower = message.to_ascii_lowercase();
                if lower.contains("not found")
                    || lower.contains("not supported")
                    || lower.contains("unimplemented")
                {
                    Ok(None)
                } else {
                    Err(TestnetError::Rpc(message))
                }
            }
        }
    }

    /// Submits a typed Rusty Kaspa RPC transaction to the configured testnet.
    pub async fn submit_transaction(
        &self,
        transaction: RpcTransaction,
        allow_orphan: bool,
        config: &TestnetConfig,
    ) -> Result<RpcTransactionId, TestnetError> {
        self.assert_target().await?;
        if !config.broadcast {
            return Err(TestnetError::BroadcastDisabled);
        }
        self.client
            .submit_transaction(transaction, allow_orphan)
            .await
            .map_err(|error| TestnetError::Rpc(error.to_string()))
    }

    /// Polls the mempool status for a transaction ID.
    pub async fn poll_transaction_status(
        &self,
        transaction_id: RpcTransactionId,
    ) -> Result<TransactionStatus, TestnetError> {
        self.assert_target().await?;
        match self
            .client
            .get_mempool_entry(transaction_id, true, false)
            .await
        {
            Ok(entry) => Ok(entry.into()),
            Err(error) => Ok(TransactionStatus::Unknown {
                message: error.to_string(),
            }),
        }
    }

    /// Waits for a transaction to become visible in the node mempool.
    pub async fn wait_for_mempool(
        &self,
        transaction_id: RpcTransactionId,
        config: &TestnetConfig,
    ) -> Result<TransactionStatus, TestnetError> {
        for _ in 0..config.poll_attempts {
            let status = self.poll_transaction_status(transaction_id).await?;
            if matches!(status, TransactionStatus::Mempool { .. }) {
                return Ok(status);
            }
            tokio::time::sleep(config.poll_interval).await;
        }

        Ok(TransactionStatus::Unknown {
            message: "transaction was not observed in mempool before timeout".to_owned(),
        })
    }

    /// Waits until an output is visible in the indexed UTXO set, and optionally
    /// until its DAA-score depth reaches the requested finality window.
    pub async fn wait_for_indexed_output(
        &self,
        address: &Address,
        transaction_id: RpcTransactionId,
        output_index: u32,
        finality_depth: Option<u64>,
        config: &TestnetConfig,
    ) -> Result<ObservedOutput, TestnetError> {
        let txid = transaction_id.to_string();
        for _ in 0..config.poll_attempts {
            let info = self.network_info().await?;
            let entries = self.fetch_rpc_utxos(address).await?;
            for entry in entries {
                if entry.outpoint.transaction_id.to_string() == txid
                    && entry.outpoint.index == output_index
                {
                    let depth = info
                        .virtual_daa_score
                        .saturating_sub(entry.utxo_entry.block_daa_score)
                        .saturating_add(1);
                    if finality_depth
                        .map(|required| depth >= required)
                        .unwrap_or(true)
                    {
                        return Ok(ObservedOutput {
                            transaction_id: txid,
                            output_index,
                            amount: entry.utxo_entry.amount,
                            block_daa_score: entry.utxo_entry.block_daa_score,
                            observed_depth: depth,
                        });
                    }
                }
            }
            tokio::time::sleep(config.poll_interval).await;
        }

        Err(TestnetError::Tx(format!(
            "output {txid}:{output_index} was not observed with finality depth {:?} before timeout",
            finality_depth
        )))
    }
}

/// Testnet wallet used by dry-run and ignored live integration tests.
pub struct TestWallet {
    secret_key: SecretKey,
    address: Address,
    public_key: [u8; 32],
}

impl fmt::Debug for TestWallet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TestWallet")
            .field("address", &self.address)
            .field("public_key", &hex_encode(&self.public_key))
            .field("private_key", &"<redacted>")
            .finish()
    }
}

impl TestWallet {
    /// Loads a test wallet from environment variables.
    pub fn from_env() -> Result<Self, TestnetError> {
        let key =
            read_required_env_with_fallback(KASPA_TESTNET_PRIVATE_KEY_ENV, TN12_PRIVATE_KEY_ENV)?;
        let wallet = Self::from_private_key_hex(&key)?;
        if let Some(expected) = read_optional_address(KASPA_TESTNET_ADDRESS_ENV)? {
            if expected != wallet.address {
                return Err(TestnetError::InvalidEnv {
                    var: KASPA_TESTNET_ADDRESS_ENV,
                    message: "address does not match private key".to_owned(),
                });
            }
        }
        Ok(wallet)
    }

    /// Loads a test wallet from a 32-byte private key hex string.
    pub fn from_private_key_hex(private_key_hex: &str) -> Result<Self, TestnetError> {
        let secret_bytes =
            decode_hex_32(private_key_hex).map_err(|message| TestnetError::InvalidEnv {
                var: KASPA_TESTNET_PRIVATE_KEY_ENV,
                message,
            })?;
        let secret_key =
            SecretKey::from_slice(&secret_bytes).map_err(|error| TestnetError::InvalidEnv {
                var: KASPA_TESTNET_PRIVATE_KEY_ENV,
                message: error.to_string(),
            })?;
        Self::from_secret_key(secret_key)
    }

    /// Generates an ephemeral test wallet.
    pub fn generate_ephemeral() -> Result<Self, TestnetError> {
        let mut rng = secp256k1::rand::thread_rng();
        let secret_key = SecretKey::new(&mut rng);
        Self::from_secret_key(secret_key)
    }

    /// Returns the derived testnet address.
    pub fn address(&self) -> &Address {
        &self.address
    }

    /// Returns the testnet address as a string.
    pub fn address_string(&self) -> String {
        self.address.to_string()
    }

    /// Returns the 32-byte x-only public key used by the address.
    pub fn public_key(&self) -> [u8; 32] {
        self.public_key
    }

    /// Returns a non-secret fingerprint for identifying the loaded key in logs.
    pub fn private_key_fingerprint(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.secret_key.secret_bytes());
        let digest = hasher.finalize();
        hex_encode(&digest[..8])
    }

    /// Lists wallet balance through the RPC adapter.
    pub async fn list_balance(&self, rpc: &TestnetRpcClient) -> Result<u64, TestnetError> {
        rpc.balance(&self.address).await
    }

    /// Signs a 32-byte transaction digest with BIP340 Schnorr.
    pub fn sign_spend_digest(&self, digest: [u8; 32]) -> [u8; 64] {
        let secp = Secp256k1::new();
        let keypair = Keypair::from_secret_key(&secp, &self.secret_key);
        let message = secp256k1::Message::from_digest(digest);
        let signature = secp.sign_schnorr(&message, &keypair);
        signature.serialize()
    }

    fn secret_bytes(&self) -> [u8; 32] {
        self.secret_key.secret_bytes()
    }

    fn from_secret_key(secret_key: SecretKey) -> Result<Self, TestnetError> {
        let secp = Secp256k1::new();
        let keypair = Keypair::from_secret_key(&secp, &secret_key);
        let (xonly, _) = XOnlyPublicKey::from_keypair(&keypair);
        let public_key = xonly.serialize();
        let address = Address::new(Prefix::Testnet, Version::PubKey, &public_key);

        Ok(Self {
            secret_key,
            address,
            public_key,
        })
    }
}

/// Bound contract parameter value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ParamValue {
    /// 32-byte Schnorr public key.
    PublicKey(Vec<u8>),
    /// 32-byte hash.
    Hash(Vec<u8>),
    /// Integer-like value.
    Integer(u64),
    /// Boolean value.
    Bool(bool),
    /// Raw byte vector.
    Bytes(Vec<u8>),
}

/// Instantiated contract script and metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractInstantiation {
    /// Contract name.
    pub contract_name: String,
    /// Spend path selected for this script.
    pub spend_name: String,
    /// Concrete redeem script bytes.
    pub redeem_script: Vec<u8>,
    /// P2SH locking script.
    pub script_public_key: ScriptPublicKey,
    /// P2SH address for the locking script.
    pub locking_address: Address,
    /// Hash of sorted instantiated contract params.
    pub instantiated_params_hash: String,
    /// Hash of the locking script bytes.
    pub locking_script_hash: String,
    /// Spend args expected in the P2SH signature script.
    pub spend_args: Vec<ArtifactParam>,
    /// Lock time required by this spend path, if statically known.
    pub lock_time: u64,
    /// Whether the spend path uses CLTV.
    pub uses_lock_time: bool,
    /// Warnings produced by target/source gates.
    pub warnings: Vec<String>,
}

/// Preview of a constructed transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionPreview {
    /// Transaction ID.
    pub txid: String,
    /// Transaction mass committed on the transaction.
    pub mass: u64,
    /// Visible fee in sompi.
    pub fee: u64,
    /// Serialized script hex for the primary script.
    pub script_hex: String,
    /// RPC transaction ready for broadcast.
    pub rpc_transaction: RpcTransaction,
}

/// Live or dry-run contract flow output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractRunResult {
    /// Compiled deployment plan.
    pub plan: ContractDeploymentPlan,
    /// Instantiated spend script.
    pub instantiation: ContractInstantiation,
    /// Lock transaction preview.
    pub lock: TransactionPreview,
    /// Spend transaction preview.
    pub spend: TransactionPreview,
}

/// Contract deployment plan derived from source and compiler output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractDeploymentPlan {
    /// Contract name used for proof file naming.
    pub contract_name: String,
    /// Source file name used for diagnostics.
    pub file: String,
    /// Source SHA-256 as lowercase hex.
    pub source_hash: String,
    /// Artifact SHA-256 as lowercase hex.
    pub artifact_hash: String,
    /// Legacy combined script SHA-256 as lowercase hex.
    pub script_hash: String,
    /// Legacy combined script bytes as lowercase hex.
    pub script_hex: String,
    /// Compiled artifact metadata.
    pub artifact: CompiledArtifact,
}

impl ContractDeploymentPlan {
    /// Compiles a KaspaScript contract and creates a deterministic deployment plan.
    pub fn from_source(
        contract_name: &str,
        file: &str,
        source: &str,
    ) -> Result<Self, TestnetError> {
        let artifact =
            compile_file(source, file).map_err(|error| TestnetError::Compile(error.to_string()))?;
        verify_artifact(&artifact).map_err(|error| TestnetError::Compile(error.to_string()))?;
        let artifact_bytes =
            serde_json::to_vec(&artifact).map_err(|error| TestnetError::Json(error.to_string()))?;
        let artifact_hash = sha256_hex(&artifact_bytes);
        let script_hash = sha256_hex(&artifact.bytecode);

        Ok(Self {
            contract_name: contract_name.to_owned(),
            file: file.to_owned(),
            source_hash: hex_encode(&artifact.source_hash),
            artifact_hash,
            script_hash,
            script_hex: bytecode_hex(&artifact.bytecode),
            artifact,
        })
    }
}

/// Result category stored in proof files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProofResult {
    /// The live test passed.
    Pass,
    /// The live test failed.
    Fail,
    /// The live test was intentionally gated.
    Gated,
}

/// Testnet proof file emitted by live integration tests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestnetProof {
    /// Target label.
    pub target: String,
    /// Node version reported by the node.
    pub node_version: String,
    /// Compiler version.
    pub compiler_version: String,
    /// Contract name.
    pub contract_name: String,
    /// Source SHA-256 as lowercase hex.
    pub source_hash: String,
    /// Artifact SHA-256 as lowercase hex.
    pub artifact_hash: String,
    /// Instantiated params SHA-256 as lowercase hex.
    pub instantiated_params_hash: String,
    /// Locking script SHA-256 as lowercase hex.
    pub locking_script_hash: String,
    /// Generated script bytes as lowercase hex.
    pub script_hex: String,
    /// Locking transaction ID when broadcast succeeds.
    pub lock_txid: Option<String>,
    /// Spend transaction ID when broadcast succeeds.
    pub spend_txid: Option<String>,
    /// Lock transaction DAA score if known.
    pub lock_daa_score: Option<u64>,
    /// Spend transaction DAA score if known.
    pub spend_daa_score: Option<u64>,
    /// Network reported by the node.
    pub network: String,
    /// Total visible fee in sompi.
    pub fee: u64,
    /// Total transaction mass.
    pub mass: u64,
    /// Valid spend result.
    pub valid_spend_result: String,
    /// Invalid spend rejection result.
    pub invalid_spend_rejection_result: String,
    /// Unix timestamp in seconds.
    pub timestamp: u64,
    /// Pass/fail/gated outcome.
    pub result: ProofResult,
    /// Warnings.
    pub warnings: Vec<String>,
    /// Failure or gate reason.
    pub error: Option<String>,
}

impl TestnetProof {
    /// Builds a proof from a deployment plan and live node metadata.
    pub fn from_run(
        run: &ContractRunResult,
        info: &NetworkInfo,
        lock_txid: Option<String>,
        spend_txid: Option<String>,
        result: ProofResult,
        error: Option<String>,
    ) -> Result<Self, TestnetError> {
        let mut warnings = run.instantiation.warnings.clone();
        warnings.extend(run.plan.artifact.warnings.iter().map(|warning| {
            format!(
                "{} from {}: {}",
                warning.id, warning.citation.path, warning.message
            )
        }));
        Ok(Self {
            target: info.network.clone(),
            node_version: info.node_version.clone(),
            compiler_version: run.plan.artifact.compiler_version.clone(),
            contract_name: run.plan.contract_name.clone(),
            source_hash: run.plan.source_hash.clone(),
            artifact_hash: run.plan.artifact_hash.clone(),
            instantiated_params_hash: run.instantiation.instantiated_params_hash.clone(),
            locking_script_hash: run.instantiation.locking_script_hash.clone(),
            script_hex: hex_encode(&run.instantiation.redeem_script),
            lock_txid,
            spend_txid,
            lock_daa_score: None,
            spend_daa_score: None,
            network: info.network.clone(),
            fee: run.lock.fee.saturating_add(run.spend.fee),
            mass: run.lock.mass.saturating_add(run.spend.mass),
            valid_spend_result: "constructed_and_script_validated".to_owned(),
            invalid_spend_rejection_result:
                "invalid signature rejected by local Rusty Kaspa script engine before broadcast"
                    .to_owned(),
            timestamp: unix_timestamp()?,
            result,
            warnings,
            error,
        })
    }

    /// Writes the proof JSON to disk.
    pub fn write_json(&self, path: impl AsRef<Path>) -> Result<(), TestnetError> {
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent).map_err(|error| TestnetError::Io(error.to_string()))?;
        }
        let json = serde_json::to_vec_pretty(self)
            .map_err(|error| TestnetError::Json(error.to_string()))?;
        fs::write(path, json).map_err(|error| TestnetError::Io(error.to_string()))
    }
}

/// Live contract flow harness.
#[derive(Debug)]
pub struct TestnetContractHarness<'a> {
    rpc: &'a TestnetRpcClient,
    wallet: &'a TestWallet,
}

impl<'a> TestnetContractHarness<'a> {
    /// Creates a live contract flow harness.
    pub fn new(rpc: &'a TestnetRpcClient, wallet: &'a TestWallet) -> Self {
        Self { rpc, wallet }
    }

    /// Compiles, instantiates, constructs, validates, and optionally broadcasts a contract flow.
    pub async fn deploy_and_execute(
        &self,
        contract_name: &str,
        file: &str,
        source: &str,
        amount_sompi: u64,
        config: &TestnetConfig,
    ) -> Result<TestnetProof, TestnetError> {
        let info = self.rpc.assert_target().await?;
        let utxos = self.rpc.fetch_rpc_utxos(self.wallet.address()).await?;
        let run = build_contract_run(
            contract_name,
            file,
            source,
            self.wallet,
            config.target,
            amount_sompi,
            &utxos,
        )?;

        let (lock_txid, spend_txid, lock_daa_score, spend_daa_score, result, error) =
            if config.broadcast {
                let lock_id = self
                    .rpc
                    .submit_transaction(run.lock.rpc_transaction.clone(), false, config)
                    .await?;
                let _ = self.rpc.wait_for_mempool(lock_id, config).await?;
                let lock_output = self
                    .rpc
                    .wait_for_indexed_output(
                        &run.instantiation.locking_address,
                        lock_id,
                        0,
                        run.plan.artifact.finality_depth,
                        config,
                    )
                    .await?;
                let spend_id = self
                    .rpc
                    .submit_transaction(run.spend.rpc_transaction.clone(), false, config)
                    .await?;
                let _ = self.rpc.wait_for_mempool(spend_id, config).await?;
                let spend_output = self
                    .rpc
                    .wait_for_indexed_output(self.wallet.address(), spend_id, 0, None, config)
                    .await
                    .ok();
                (
                    Some(lock_id.to_string()),
                    Some(spend_id.to_string()),
                    Some(lock_output.block_daa_score),
                    spend_output.map(|output| output.block_daa_score),
                    ProofResult::Pass,
                    None,
                )
            } else {
                (
                    None,
                    None,
                    None,
                    None,
                    ProofResult::Gated,
                    Some(
                        "dry-run only; pass --broadcast to submit typed Rusty Kaspa transactions"
                            .to_owned(),
                    ),
                )
            };

        let mut proof = TestnetProof::from_run(&run, &info, lock_txid, spend_txid, result, error)?;
        proof.lock_daa_score = lock_daa_score;
        proof.spend_daa_score = spend_daa_score;
        Ok(proof)
    }

    /// Builds a dry-run proof without broadcasting.
    pub async fn gated_proof(
        &self,
        contract_name: &str,
        file: &str,
        source: &str,
    ) -> Result<TestnetProof, TestnetError> {
        let config = TestnetConfig::new(self.rpc.target, "");
        let info = self.rpc.assert_target().await?;
        let utxos = self.rpc.fetch_rpc_utxos(self.wallet.address()).await?;
        let run = build_contract_run(
            contract_name,
            file,
            source,
            self.wallet,
            self.rpc.target,
            SOMPI_PER_KASPA / 100,
            &utxos,
        )?;
        let _ = config;
        TestnetProof::from_run(
            &run,
            &info,
            None,
            None,
            ProofResult::Gated,
            Some("dry-run proof; no broadcast".to_owned()),
        )
    }
}

/// Builds a full dry-run contract run from typed RPC UTXOs.
pub fn build_contract_run(
    contract_name: &str,
    file: &str,
    source: &str,
    wallet: &TestWallet,
    target: TestnetTarget,
    amount_sompi: u64,
    rpc_utxos: &[RpcUtxosByAddressesEntry],
) -> Result<ContractRunResult, TestnetError> {
    let plan = ContractDeploymentPlan::from_source(contract_name, file, source)?;
    let spend_name = default_spend_name(&plan.artifact, contract_name)?;
    let contract_params = default_contract_params(&plan.artifact, contract_name, wallet)?;
    let instantiation = instantiate_contract(
        &plan.artifact,
        contract_name,
        &spend_name,
        contract_params,
        target,
    )?;

    let funding = rpc_utxos
        .iter()
        .filter(|utxo| !utxo.utxo_entry.is_coinbase)
        .map(|utxo| FundingUtxo::from_rpc(utxo, wallet.address().clone()))
        .collect::<Vec<_>>();

    let lock = build_lock_transaction(wallet, target, amount_sompi, &instantiation, &funding)?;
    let lock_output = LockedContractOutput {
        outpoint: TransactionOutpoint::new(lock.tx.id(), 0),
        amount: amount_sompi,
        script_public_key: instantiation.script_public_key.clone(),
        block_daa_score: 0,
    };
    let spent_outpoints = lock
        .inputs
        .iter()
        .map(|input| input.previous_outpoint)
        .collect::<HashSet<_>>();
    let mut fee_funding = funding
        .into_iter()
        .filter(|utxo| !spent_outpoints.contains(&utxo.outpoint))
        .collect::<Vec<_>>();
    if let Some(change_utxo) = lock_change_funding(&lock.tx, wallet.address().clone())? {
        fee_funding.push(change_utxo);
    }
    let spend = build_spend_transaction(wallet, target, &instantiation, lock_output, &fee_funding)?;

    Ok(ContractRunResult {
        plan,
        instantiation,
        lock: lock.preview,
        spend: spend.preview,
    })
}

/// Instantiates a compiled artifact into a concrete P2SH spend script.
pub fn instantiate_contract(
    artifact: &CompiledArtifact,
    contract_name: &str,
    spend_name: &str,
    contract_params: BTreeMap<String, ParamValue>,
    target: TestnetTarget,
) -> Result<ContractInstantiation, TestnetError> {
    let contract = find_contract(artifact, contract_name)?;
    let spend = find_spend(contract, spend_name)?;
    validate_param_bindings(&contract.params, &contract_params)?;

    let mut transformed = Vec::new();
    let spend_param_names = spend
        .params
        .iter()
        .map(|param| (param.name.as_str(), param.ty))
        .collect::<BTreeMap<_, _>>();
    let contract_param_types = contract
        .params
        .iter()
        .map(|param| (param.name.as_str(), param.ty))
        .collect::<BTreeMap<_, _>>();

    for (index, instruction) in spend.instructions.iter().enumerate() {
        match &instruction.kind {
            InstructionKind::Push(Value::Symbol(name))
                if spend_param_names.contains_key(name.as_str()) =>
            {
                continue;
            }
            InstructionKind::Push(Value::Symbol(name)) => {
                let ty = contract_param_types.get(name.as_str()).ok_or_else(|| {
                    TestnetError::Param(format!("unbound symbol `{name}` in spend `{spend_name}`"))
                })?;
                let value = contract_params.get(name).ok_or_else(|| {
                    TestnetError::Param(format!("missing contract parameter `{name}`"))
                })?;
                let bytes_for_spk_compare =
                    previous_instruction_is_script_read(&spend.instructions, index)
                        && matches!(ty, TypeName::PublicKey);
                transformed.push(Instruction::new(
                    instruction.span,
                    InstructionKind::Push(value_to_ir(value, *ty, bytes_for_spk_compare)?),
                ));
            }
            _ => transformed.push(instruction.clone()),
        }
    }

    let redeem_script = compile_instruction_sequence(&transformed)
        .map_err(|error| TestnetError::Compile(error.to_string()))?;
    let script_public_key = pay_to_script_hash_script(&redeem_script);
    let locking_address = extract_script_pub_key_address(&script_public_key, Prefix::Testnet)
        .map_err(|error| TestnetError::Tx(error.to_string()))?;
    let instantiated_params_hash = hash_params(&contract_params)?;
    let locking_script_hash = sha256_hex(script_public_key.script());
    let (uses_lock_time, lock_time) = lock_time_requirement(&transformed);
    let mut warnings = target.warnings();
    warnings.push("script construction uses Rusty Kaspa pay_to_script_hash_script and ScriptBuilder; spend args are supplied through P2SH signature_script".to_owned());

    Ok(ContractInstantiation {
        contract_name: contract.name.clone(),
        spend_name: spend.name.clone(),
        redeem_script,
        script_public_key,
        locking_address,
        instantiated_params_hash,
        locking_script_hash,
        spend_args: spend.params.clone(),
        lock_time,
        uses_lock_time,
        warnings,
    })
}

#[derive(Clone)]
struct FundingUtxo {
    outpoint: TransactionOutpoint,
    entry: UtxoEntry,
    client_ref: UtxoEntryReference,
}

impl FundingUtxo {
    fn from_rpc(entry: &RpcUtxosByAddressesEntry, address: Address) -> Self {
        let outpoint: TransactionOutpoint = entry.outpoint.into();
        let client_entry = ClientUtxoEntry {
            address: Some(address),
            outpoint: ClientTransactionOutpoint::from(outpoint),
            amount: entry.utxo_entry.amount,
            script_public_key: entry.utxo_entry.script_public_key.clone(),
            block_daa_score: entry.utxo_entry.block_daa_score,
            is_coinbase: entry.utxo_entry.is_coinbase,
        };
        Self {
            outpoint,
            entry: UtxoEntry::new(
                entry.utxo_entry.amount,
                entry.utxo_entry.script_public_key.clone(),
                entry.utxo_entry.block_daa_score,
                entry.utxo_entry.is_coinbase,
            ),
            client_ref: UtxoEntryReference::from(client_entry),
        }
    }

    fn from_transaction_output(
        transaction: &Transaction,
        index: usize,
        address: Address,
    ) -> Result<Self, TestnetError> {
        let output = transaction
            .outputs
            .get(index)
            .ok_or_else(|| TestnetError::Tx(format!("missing transaction output {index}")))?;
        let output_index =
            u32::try_from(index).map_err(|error| TestnetError::Tx(error.to_string()))?;
        let outpoint = TransactionOutpoint::new(transaction.id(), output_index);
        let client_entry = ClientUtxoEntry {
            address: Some(address),
            outpoint: ClientTransactionOutpoint::from(outpoint),
            amount: output.value,
            script_public_key: output.script_public_key.clone(),
            block_daa_score: 0,
            is_coinbase: false,
        };
        Ok(Self {
            outpoint,
            entry: UtxoEntry::new(output.value, output.script_public_key.clone(), 0, false),
            client_ref: UtxoEntryReference::from(client_entry),
        })
    }
}

struct LockedContractOutput {
    outpoint: TransactionOutpoint,
    amount: u64,
    script_public_key: ScriptPublicKey,
    block_daa_score: u64,
}

struct BuiltTransaction {
    tx: Transaction,
    inputs: Vec<TransactionInput>,
    preview: TransactionPreview,
}

fn lock_change_funding(
    transaction: &Transaction,
    address: Address,
) -> Result<Option<FundingUtxo>, TestnetError> {
    if transaction.outputs.len() <= 1 {
        return Ok(None);
    }
    FundingUtxo::from_transaction_output(transaction, 1, address).map(Some)
}

fn build_lock_transaction(
    wallet: &TestWallet,
    target: TestnetTarget,
    amount: u64,
    instantiation: &ContractInstantiation,
    funding: &[FundingUtxo],
) -> Result<BuiltTransaction, TestnetError> {
    if amount == 0 {
        return Err(TestnetError::Tx(
            "lock amount must be greater than zero".to_owned(),
        ));
    }
    let selected = select_funding(funding, amount.saturating_add(10_000))?;
    let destination = vec![TransactionOutput::new(
        amount,
        instantiation.script_public_key.clone(),
    )];
    let built =
        build_signed_funding_transaction(wallet, target, &selected, destination, 0, "lock")?;
    Ok(built)
}

fn build_spend_transaction(
    wallet: &TestWallet,
    target: TestnetTarget,
    instantiation: &ContractInstantiation,
    locked: LockedContractOutput,
    funding: &[FundingUtxo],
) -> Result<BuiltTransaction, TestnetError> {
    let fee_funding = select_funding(funding, 10_000)?;
    let mut inputs = Vec::with_capacity(1 + fee_funding.len());
    let contract_sequence = if instantiation.uses_lock_time {
        0
    } else {
        MAX_TX_IN_SEQUENCE_NUM
    };
    inputs.push(TransactionInput::new(
        locked.outpoint,
        Vec::new(),
        contract_sequence,
        0,
    ));
    for utxo in &fee_funding {
        inputs.push(TransactionInput::new(
            utxo.outpoint,
            dummy_p2pk_signature_script(),
            MAX_TX_IN_SEQUENCE_NUM,
            1,
        ));
    }

    let mut entries = Vec::with_capacity(1 + fee_funding.len());
    entries.push(UtxoEntry::new(
        locked.amount,
        locked.script_public_key.clone(),
        locked.block_daa_score,
        false,
    ));
    entries.extend(fee_funding.iter().map(|utxo| utxo.entry.clone()));

    let mut refs = Vec::with_capacity(fee_funding.len());
    refs.push(client_ref_for_locked(&locked));
    refs.extend(fee_funding.iter().map(|utxo| utxo.client_ref.clone()));

    let output_zero =
        TransactionOutput::new(locked.amount, pay_to_address_script(wallet.address()));
    let mut fee = 0u64;
    let mut change = 0u64;
    let funding_total = fee_funding
        .iter()
        .map(|utxo| utxo.entry.amount)
        .sum::<u64>();
    let mass_calculator = mass_calculator(target);

    let mut final_tx = None;
    for _ in 0..10 {
        let mut outputs = vec![output_zero.clone()];
        if change > 0 && !mass_calculator.is_dust(change) {
            outputs.push(TransactionOutput::new(
                change,
                pay_to_address_script(wallet.address()),
            ));
        }
        let mut tx = Transaction::new(
            TX_VERSION,
            inputs.clone(),
            outputs,
            instantiation.lock_time,
            SUBNETWORK_ID_NATIVE,
            0,
            Vec::new(),
        );
        let dummy_contract_script = build_contract_signature_script(
            &instantiation.redeem_script,
            &dummy_spend_arg_payloads(&instantiation.spend_args)?,
        )?;
        tx.inputs[0].signature_script = dummy_contract_script;
        tx.inputs[0].sig_op_count =
            p2sh_sig_op_count(&tx.inputs[0].signature_script, &locked.script_public_key)?;
        let mass = calculate_mass(&mass_calculator, &tx, &refs)?;
        if mass > MAXIMUM_STANDARD_TRANSACTION_MASS {
            return Err(TestnetError::Tx(format!(
                "spend transaction mass {mass} exceeds standard limit {MAXIMUM_STANDARD_TRANSACTION_MASS}"
            )));
        }
        let next_fee = mass_calculator.calc_minimum_transaction_fee_from_mass(mass);
        let next_change =
            funding_total
                .checked_sub(next_fee)
                .ok_or(TestnetError::InsufficientFunds {
                    needed: next_fee,
                    available: funding_total,
                })?;
        tx.set_mass(mass);
        final_tx = Some((tx, mass, next_fee));
        if next_fee == fee && next_change == change {
            break;
        }
        fee = next_fee;
        change = next_change;
    }

    let (mut tx, mass, fee) =
        final_tx.ok_or_else(|| TestnetError::Tx("failed to build spend transaction".to_owned()))?;
    sign_p2pk_inputs(&mut tx, &entries, wallet, 1)?;
    sign_contract_input(&mut tx, &entries, wallet, instantiation)?;
    tx.finalize();
    validate_scripts(&tx, &entries)?;

    Ok(BuiltTransaction {
        inputs: tx.inputs.clone(),
        preview: TransactionPreview {
            txid: tx.id().to_string(),
            mass,
            fee,
            script_hex: hex_encode(&instantiation.redeem_script),
            rpc_transaction: RpcTransaction::from(&tx),
        },
        tx,
    })
}

fn build_signed_funding_transaction(
    wallet: &TestWallet,
    target: TestnetTarget,
    selected: &[FundingUtxo],
    destination_outputs: Vec<TransactionOutput>,
    lock_time: u64,
    script_label: &str,
) -> Result<BuiltTransaction, TestnetError> {
    let mut inputs = selected
        .iter()
        .map(|utxo| {
            TransactionInput::new(
                utxo.outpoint,
                dummy_p2pk_signature_script(),
                MAX_TX_IN_SEQUENCE_NUM,
                1,
            )
        })
        .collect::<Vec<_>>();
    let entries = selected
        .iter()
        .map(|utxo| utxo.entry.clone())
        .collect::<Vec<_>>();
    let refs = selected
        .iter()
        .map(|utxo| utxo.client_ref.clone())
        .collect::<Vec<_>>();
    let total_input = selected.iter().map(|utxo| utxo.entry.amount).sum::<u64>();
    let destination_total = destination_outputs
        .iter()
        .map(|output| output.value)
        .sum::<u64>();
    let mass_calculator = mass_calculator(target);
    let mut fee = 0u64;
    let mut change = 0u64;
    let mut final_tx = None;

    for _ in 0..10 {
        let mut outputs = destination_outputs.clone();
        if change > 0 && !mass_calculator.is_dust(change) {
            outputs.push(TransactionOutput::new(
                change,
                pay_to_address_script(wallet.address()),
            ));
        }
        let tx = Transaction::new(
            TX_VERSION,
            inputs.clone(),
            outputs,
            lock_time,
            SUBNETWORK_ID_NATIVE,
            0,
            Vec::new(),
        );
        let mass = calculate_mass(&mass_calculator, &tx, &refs)?;
        if mass > MAXIMUM_STANDARD_TRANSACTION_MASS {
            return Err(TestnetError::Tx(format!(
                "{script_label} transaction mass {mass} exceeds standard limit {MAXIMUM_STANDARD_TRANSACTION_MASS}"
            )));
        }
        let next_fee = mass_calculator.calc_minimum_transaction_fee_from_mass(mass);
        let needed = destination_total.saturating_add(next_fee);
        let next_change =
            total_input
                .checked_sub(needed)
                .ok_or(TestnetError::InsufficientFunds {
                    needed,
                    available: total_input,
                })?;
        final_tx = Some((tx, mass, next_fee));
        if next_fee == fee && next_change == change {
            break;
        }
        fee = next_fee;
        change = next_change;
    }

    let (mut tx, mass, fee) =
        final_tx.ok_or_else(|| TestnetError::Tx("failed to build transaction".to_owned()))?;
    inputs = tx.inputs.clone();
    sign_p2pk_inputs(&mut tx, &entries, wallet, 0)?;
    tx.set_mass(mass);
    tx.finalize();
    validate_scripts(&tx, &entries)?;

    Ok(BuiltTransaction {
        inputs,
        preview: TransactionPreview {
            txid: tx.id().to_string(),
            mass,
            fee,
            script_hex: tx
                .outputs
                .first()
                .map(|output| hex_encode(output.script_public_key.script()))
                .unwrap_or_default(),
            rpc_transaction: RpcTransaction::from(&tx),
        },
        tx,
    })
}

fn sign_p2pk_inputs(
    tx: &mut Transaction,
    entries: &[UtxoEntry],
    wallet: &TestWallet,
    start_index: usize,
) -> Result<(), TestnetError> {
    let signable = SignableTransaction::with_entries(tx.clone(), entries.to_vec());
    let verifiable = signable.as_verifiable();
    for index in start_index..tx.inputs.len() {
        tx.inputs[index].signature_script =
            sign_input(&verifiable, index, &wallet.secret_bytes(), SIG_HASH_ALL);
    }
    Ok(())
}

fn sign_contract_input(
    tx: &mut Transaction,
    entries: &[UtxoEntry],
    wallet: &TestWallet,
    instantiation: &ContractInstantiation,
) -> Result<(), TestnetError> {
    let signable = SignableTransaction::with_entries(tx.clone(), entries.to_vec());
    let verifiable = signable.as_verifiable();
    let signature_script = sign_input(&verifiable, 0, &wallet.secret_bytes(), SIG_HASH_ALL);
    let signature_payload = parse_single_signature_payload(&signature_script)?;
    let payloads = instantiation
        .spend_args
        .iter()
        .map(|param| {
            if matches!(param.ty, TypeName::Signature) {
                Ok(signature_payload.clone())
            } else {
                Err(TestnetError::Unsupported(format!(
                    "live spend arg `{}` of type {:?} is not yet supported by the P2SH builder",
                    param.name, param.ty
                )))
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    tx.inputs[0].signature_script =
        build_contract_signature_script(&instantiation.redeem_script, &payloads)?;
    tx.inputs[0].sig_op_count = p2sh_sig_op_count(
        &tx.inputs[0].signature_script,
        &entries[0].script_public_key,
    )?;
    Ok(())
}

fn validate_scripts(tx: &Transaction, entries: &[UtxoEntry]) -> Result<(), TestnetError> {
    let populated = PopulatedTransaction::new(tx, entries.to_vec());
    let mut reused = SigHashReusedValues::new();
    let sig_cache = Cache::<SigCacheKey, bool>::new(10_000);
    for (index, input) in tx.inputs.iter().enumerate() {
        let entry = entries
            .get(index)
            .ok_or_else(|| TestnetError::Script(format!("missing UTXO entry for input {index}")))?;
        let mut engine = TxScriptEngine::from_transaction_input(
            &populated,
            input,
            index,
            entry,
            &mut reused,
            &sig_cache,
        )
        .map_err(|error| TestnetError::Script(error.to_string()))?;
        engine
            .execute()
            .map_err(|error| TestnetError::Script(error.to_string()))?;
    }
    Ok(())
}

fn calculate_mass(
    calculator: &MassCalculator,
    tx: &Transaction,
    refs: &[UtxoEntryReference],
) -> Result<u64, TestnetError> {
    let compute_mass = calculator.calc_compute_mass_for_signed_consensus_transaction(tx);
    let storage_mass = calculator
        .calc_storage_mass_for_transaction_parts(refs, &tx.outputs)
        .ok_or_else(|| TestnetError::Tx("storage mass calculation failed".to_owned()))?;
    Ok(calculator.combine_mass(compute_mass, storage_mass))
}

fn select_funding(funding: &[FundingUtxo], minimum: u64) -> Result<Vec<FundingUtxo>, TestnetError> {
    let mut selected = Vec::new();
    let mut total = 0u64;
    let mut sorted = funding.to_vec();
    sorted.sort_by_key(|utxo| std::cmp::Reverse(utxo.entry.amount));
    for utxo in sorted {
        total = total.saturating_add(utxo.entry.amount);
        selected.push(utxo);
        if total >= minimum {
            return Ok(selected);
        }
    }
    Err(TestnetError::InsufficientFunds {
        needed: minimum,
        available: total,
    })
}

fn mass_calculator(_target: TestnetTarget) -> MassCalculator {
    let params = Params::from(NetworkType::Testnet);
    let network_params = match _target {
        TestnetTarget::Tn10 => {
            NetworkParams::from(NetworkId::with_suffix(NetworkType::Testnet, 10))
        }
        TestnetTarget::Tn11 | TestnetTarget::Tn12 => {
            NetworkParams::from(NetworkId::with_suffix(NetworkType::Testnet, 11))
        }
    };
    MassCalculator::new(&params, network_params)
}

fn client_ref_for_locked(locked: &LockedContractOutput) -> UtxoEntryReference {
    UtxoEntryReference::from(ClientUtxoEntry {
        address: None,
        outpoint: ClientTransactionOutpoint::from(locked.outpoint),
        amount: locked.amount,
        script_public_key: locked.script_public_key.clone(),
        block_daa_score: locked.block_daa_score,
        is_coinbase: false,
    })
}

fn default_spend_name(
    artifact: &CompiledArtifact,
    contract_name: &str,
) -> Result<String, TestnetError> {
    let contract = find_contract(artifact, contract_name)?;
    if contract.name == "AtomicSwap" {
        return Ok("refund_path".to_owned());
    }
    contract
        .spends
        .first()
        .map(|spend| spend.name.clone())
        .ok_or_else(|| TestnetError::Artifact("contract has no spend paths".to_owned()))
}

fn default_contract_params(
    artifact: &CompiledArtifact,
    contract_name: &str,
    wallet: &TestWallet,
) -> Result<BTreeMap<String, ParamValue>, TestnetError> {
    let contract = find_contract(artifact, contract_name)?;
    let mut params = BTreeMap::new();
    for param in &contract.params {
        let value = match param.ty {
            TypeName::PublicKey => ParamValue::PublicKey(wallet.public_key().to_vec()),
            TypeName::Hash => ParamValue::Hash([0u8; 32].to_vec()),
            TypeName::BlockHeight | TypeName::Amount => ParamValue::Integer(0),
            TypeName::Bool => ParamValue::Bool(true),
            TypeName::Bytes => ParamValue::Bytes(Vec::new()),
            other => {
                return Err(TestnetError::Unsupported(format!(
                    "default binding for contract parameter `{}` of type {:?} is not supported",
                    param.name, other
                )));
            }
        };
        params.insert(param.name.clone(), value);
    }
    Ok(params)
}

fn find_contract<'a>(
    artifact: &'a CompiledArtifact,
    name: &str,
) -> Result<&'a ArtifactContract, TestnetError> {
    artifact
        .contracts
        .iter()
        .find(|contract| contract.name == name || contract.name.eq_ignore_ascii_case(name))
        .or_else(|| artifact.contracts.first())
        .ok_or_else(|| TestnetError::Artifact("artifact contains no contract ABI".to_owned()))
}

fn find_spend<'a>(
    contract: &'a ArtifactContract,
    name: &str,
) -> Result<&'a ArtifactSpend, TestnetError> {
    contract
        .spends
        .iter()
        .find(|spend| spend.name == name)
        .ok_or_else(|| {
            TestnetError::Artifact(format!(
                "contract `{}` has no spend path `{name}`",
                contract.name
            ))
        })
}

fn validate_param_bindings(
    params: &[ArtifactParam],
    bindings: &BTreeMap<String, ParamValue>,
) -> Result<(), TestnetError> {
    for param in params {
        let value = bindings
            .get(&param.name)
            .ok_or_else(|| TestnetError::Param(format!("missing parameter `{}`", param.name)))?;
        validate_param_type(&param.name, param.ty, value)?;
    }
    Ok(())
}

fn validate_param_type(name: &str, ty: TypeName, value: &ParamValue) -> Result<(), TestnetError> {
    let valid = matches!(
        (ty, value),
        (TypeName::PublicKey, ParamValue::PublicKey(bytes)) if bytes.len() == 32
    ) || matches!(
        (ty, value),
        (TypeName::Hash, ParamValue::Hash(bytes)) if bytes.len() == 32
    ) || matches!(
        (ty, value),
        (
            TypeName::BlockHeight | TypeName::Amount,
            ParamValue::Integer(_)
        )
    ) || matches!((ty, value), (TypeName::Bool, ParamValue::Bool(_)))
        || matches!((ty, value), (TypeName::Bytes, ParamValue::Bytes(_)));
    if valid {
        Ok(())
    } else {
        Err(TestnetError::Param(format!(
            "parameter `{name}` expects {:?}, got {:?}",
            ty, value
        )))
    }
}

fn value_to_ir(
    value: &ParamValue,
    ty: TypeName,
    as_script_public_key: bool,
) -> Result<Value, TestnetError> {
    match (value, ty, as_script_public_key) {
        (ParamValue::PublicKey(bytes), TypeName::PublicKey, true) => {
            let spk = pay_to_address_script(&Address::new(Prefix::Testnet, Version::PubKey, bytes));
            Ok(Value::Bytes(script_public_key_bytes(&spk)))
        }
        (ParamValue::PublicKey(bytes), TypeName::PublicKey, false) => {
            Ok(Value::Bytes(bytes.clone()))
        }
        (ParamValue::Hash(bytes), TypeName::Hash, _) => Ok(Value::Bytes(bytes.clone())),
        (ParamValue::Integer(value), TypeName::BlockHeight | TypeName::Amount, _) => {
            Ok(Value::Integer(*value))
        }
        (ParamValue::Bool(value), TypeName::Bool, _) => Ok(Value::Bool(*value)),
        (ParamValue::Bytes(bytes), TypeName::Bytes, _) => Ok(Value::Bytes(bytes.clone())),
        _ => Err(TestnetError::Param(format!(
            "cannot convert {:?} to IR value for {:?}",
            value, ty
        ))),
    }
}

fn previous_instruction_is_script_read(instructions: &[Instruction], index: usize) -> bool {
    index > 0
        && matches!(
            instructions[index - 1].kind,
            InstructionKind::OutputScript(_) | InstructionKind::InputScript(_)
        )
}

fn lock_time_requirement(instructions: &[Instruction]) -> (bool, u64) {
    for instruction in instructions {
        match instruction.kind {
            InstructionKind::CheckLockHeight(value) | InstructionKind::CheckLockTime(value) => {
                return (true, value);
            }
            InstructionKind::CheckLockHeightFromStack | InstructionKind::CheckLockTimeFromStack => {
                return (true, 0);
            }
            _ => {}
        }
    }
    (false, 0)
}

fn hash_params(params: &BTreeMap<String, ParamValue>) -> Result<String, TestnetError> {
    let bytes =
        serde_json::to_vec(params).map_err(|error| TestnetError::Json(error.to_string()))?;
    Ok(sha256_hex(&bytes))
}

fn p2sh_sig_op_count(
    signature_script: &[u8],
    script_public_key: &ScriptPublicKey,
) -> Result<u8, TestnetError> {
    let count =
        get_sig_op_count::<PopulatedTransaction<'static>>(signature_script, script_public_key);
    u8::try_from(count).map_err(|error| TestnetError::Tx(error.to_string()))
}

fn build_contract_signature_script(
    redeem_script: &[u8],
    payloads: &[Vec<u8>],
) -> Result<Vec<u8>, TestnetError> {
    let mut builder = ScriptBuilder::new();
    for payload in payloads {
        builder
            .add_data(payload)
            .map_err(|error| TestnetError::Tx(error.to_string()))?;
    }
    builder
        .add_data(redeem_script)
        .map_err(|error| TestnetError::Tx(error.to_string()))?;
    Ok(builder.drain())
}

fn dummy_spend_arg_payloads(params: &[ArtifactParam]) -> Result<Vec<Vec<u8>>, TestnetError> {
    params
        .iter()
        .map(|param| match param.ty {
            TypeName::Signature => Ok(vec![0u8; 65]),
            other => Err(TestnetError::Unsupported(format!(
                "live spend arg `{}` of type {:?} is not yet supported",
                param.name, other
            ))),
        })
        .collect()
}

fn parse_single_signature_payload(signature_script: &[u8]) -> Result<Vec<u8>, TestnetError> {
    if signature_script.len() != 66 || signature_script.first() != Some(&65) {
        return Err(TestnetError::Tx(
            "Rusty Kaspa sign_input returned an unexpected signature script shape".to_owned(),
        ));
    }
    Ok(signature_script[1..].to_vec())
}

fn dummy_p2pk_signature_script() -> Vec<u8> {
    let mut script = Vec::with_capacity(66);
    script.push(65);
    script.extend([0u8; 65]);
    script
}

fn script_public_key_bytes(spk: &ScriptPublicKey) -> Vec<u8> {
    spk.version()
        .to_be_bytes()
        .into_iter()
        .chain(spk.script().iter().copied())
        .collect()
}

fn normalize_utxo(entry: RpcUtxosByAddressesEntry, virtual_daa_score: u64) -> TestnetUtxo {
    let confirmations = virtual_daa_score
        .saturating_sub(entry.utxo_entry.block_daa_score)
        .saturating_add(1);
    TestnetUtxo {
        outpoint: format!("{}:{}", entry.outpoint.transaction_id, entry.outpoint.index),
        amount: entry.utxo_entry.amount,
        block_daa_score: entry.utxo_entry.block_daa_score,
        confirmations,
        is_coinbase: entry.utxo_entry.is_coinbase,
        script_public_key_hex: hex_encode(entry.utxo_entry.script_public_key.script()),
    }
}

fn read_required_env_with_fallback(
    primary: &'static str,
    fallback: &'static str,
) -> Result<String, TestnetError> {
    match env::var(primary) {
        Ok(value) if !value.trim().is_empty() => Ok(value),
        _ => read_required_env(fallback).map_err(|_| TestnetError::MissingEnv(primary)),
    }
}

fn read_required_env(name: &'static str) -> Result<String, TestnetError> {
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => Ok(value),
        Ok(_) | Err(_) => Err(TestnetError::MissingEnv(name)),
    }
}

fn read_optional_address(name: &'static str) -> Result<Option<Address>, TestnetError> {
    let value = match env::var(name) {
        Ok(value) if !value.trim().is_empty() => value,
        Ok(_) | Err(_) => return Ok(None),
    };
    let address = Address::try_from(value.trim()).map_err(|error| TestnetError::InvalidEnv {
        var: name,
        message: error.to_string(),
    })?;
    if address.prefix != Prefix::Testnet {
        return Err(TestnetError::InvalidEnv {
            var: name,
            message: "address must use the kaspatest prefix".to_owned(),
        });
    }
    Ok(Some(address))
}

fn read_optional_usize(name: &'static str) -> Result<Option<usize>, TestnetError> {
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => {
            value
                .parse::<usize>()
                .map(Some)
                .map_err(|error| TestnetError::InvalidEnv {
                    var: name,
                    message: error.to_string(),
                })
        }
        Ok(_) | Err(_) => Ok(None),
    }
}

fn read_optional_u64(name: &'static str) -> Result<Option<u64>, TestnetError> {
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => {
            value
                .parse::<u64>()
                .map(Some)
                .map_err(|error| TestnetError::InvalidEnv {
                    var: name,
                    message: error.to_string(),
                })
        }
        Ok(_) | Err(_) => Ok(None),
    }
}

fn read_optional_bool(name: &'static str) -> Result<Option<bool>, TestnetError> {
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => match value.to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" => Ok(Some(true)),
            "0" | "false" | "no" => Ok(Some(false)),
            _ => Err(TestnetError::InvalidEnv {
                var: name,
                message: "expected true/false".to_owned(),
            }),
        },
        Ok(_) | Err(_) => Ok(None),
    }
}

fn decode_hex_32(value: &str) -> Result<[u8; 32], String> {
    let bytes = decode_hex(value.trim())?;
    if bytes.len() != 32 {
        return Err(format!("expected 32 bytes, got {}", bytes.len()));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn decode_hex(value: &str) -> Result<Vec<u8>, String> {
    let value = value.strip_prefix("0x").unwrap_or(value);
    if value.len() % 2 != 0 {
        return Err("hex string must contain an even number of digits".to_owned());
    }

    let mut bytes = Vec::with_capacity(value.len() / 2);
    let chars = value.as_bytes();
    for index in (0..chars.len()).step_by(2) {
        let high = hex_nibble(chars[index]).ok_or_else(|| {
            format!(
                "invalid hex digit `{}` at byte {}",
                chars[index] as char, index
            )
        })?;
        let low = hex_nibble(chars[index + 1]).ok_or_else(|| {
            format!(
                "invalid hex digit `{}` at byte {}",
                chars[index + 1] as char,
                index + 1
            )
        })?;
        bytes.push((high << 4) | low);
    }
    Ok(bytes)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    hex_encode(&digest)
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn unix_timestamp() -> Result<u64, TestnetError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| TestnetError::InvalidData(error.to_string()))
}

/// Backwards-compatible type aliases.
pub type Tn12Config = TestnetConfig;
/// Backwards-compatible type aliases.
pub type Tn12RpcClient = TestnetRpcClient;
/// Backwards-compatible type aliases.
pub type Tn12Utxo = TestnetUtxo;
/// Backwards-compatible type aliases.
pub type Tn12Proof = TestnetProof;
/// Backwards-compatible type aliases.
pub type Tn12ContractHarness<'a> = TestnetContractHarness<'a>;
/// Backwards-compatible type aliases.
pub type Tn12Error = TestnetError;

#[cfg(test)]
mod tests {
    use super::*;
    use kaspa_consensus_core::tx::TransactionId;

    #[test]
    fn private_key_hex_must_be_32_bytes() {
        let error = TestWallet::from_private_key_hex("abcd").expect_err("short key rejected");
        assert!(matches!(error, TestnetError::InvalidEnv { .. }));
    }

    #[test]
    fn ephemeral_wallet_uses_testnet_address_prefix() {
        let wallet = TestWallet::generate_ephemeral().expect("wallet");
        assert_eq!(wallet.address().prefix, Prefix::Testnet);
        assert!(wallet.address_string().starts_with("kaspatest:"));
        assert_eq!(wallet.public_key().len(), 32);
    }

    #[test]
    fn target_parser_rejects_mainnet() {
        let error = TestnetTarget::parse("mainnet").expect_err("mainnet rejected");
        assert!(matches!(error, TestnetError::MainnetRejected));
    }

    #[test]
    fn deployment_plan_is_deterministic() {
        let source = include_str!("../../tests/contracts/escrow.ks");
        let first =
            ContractDeploymentPlan::from_source("escrow", "escrow.ks", source).expect("first");
        let second =
            ContractDeploymentPlan::from_source("escrow", "escrow.ks", source).expect("second");

        assert_eq!(first.source_hash, second.source_hash);
        assert_eq!(first.artifact_hash, second.artifact_hash);
        assert_eq!(first.script_hash, second.script_hash);
        assert_eq!(first.script_hex, second.script_hex);
    }

    #[test]
    fn instantiation_replaces_public_keys_with_bytes() {
        let wallet = TestWallet::generate_ephemeral().expect("wallet");
        let source = include_str!("../../tests/contracts/timelock.ks");
        let plan =
            ContractDeploymentPlan::from_source("Timelock", "timelock.ks", source).expect("plan");
        let params = default_contract_params(&plan.artifact, "Timelock", &wallet).expect("params");
        let instantiation = instantiate_contract(
            &plan.artifact,
            "Timelock",
            "claim",
            params,
            TestnetTarget::Tn10,
        )
        .expect("instantiate");

        assert!(!instantiation.redeem_script.is_empty());
        assert!(instantiation.uses_lock_time);
        assert_eq!(instantiation.lock_time, 0);
        assert!(instantiation
            .locking_address
            .to_string()
            .starts_with("kaspatest:"));
    }

    #[test]
    fn proof_json_never_contains_private_key_material() {
        let wallet = TestWallet::generate_ephemeral().expect("wallet");
        let source = include_str!("../../tests/contracts/timelock.ks");
        let plan =
            ContractDeploymentPlan::from_source("Timelock", "timelock.ks", source).expect("plan");
        let params = default_contract_params(&plan.artifact, "Timelock", &wallet).expect("params");
        let instantiation = instantiate_contract(
            &plan.artifact,
            "Timelock",
            "claim",
            params,
            TestnetTarget::Tn12,
        )
        .expect("instantiate");
        let run = ContractRunResult {
            lock: TransactionPreview {
                txid: "lock".to_owned(),
                mass: 1,
                fee: 1,
                script_hex: String::new(),
                rpc_transaction: RpcTransaction {
                    version: 0,
                    inputs: Vec::new(),
                    outputs: Vec::new(),
                    lock_time: 0,
                    subnetwork_id: SUBNETWORK_ID_NATIVE,
                    gas: 0,
                    payload: Vec::new(),
                    mass: 0,
                    verbose_data: None,
                },
            },
            spend: TransactionPreview {
                txid: "spend".to_owned(),
                mass: 1,
                fee: 1,
                script_hex: String::new(),
                rpc_transaction: RpcTransaction {
                    version: 0,
                    inputs: Vec::new(),
                    outputs: Vec::new(),
                    lock_time: 0,
                    subnetwork_id: SUBNETWORK_ID_NATIVE,
                    gas: 0,
                    payload: Vec::new(),
                    mass: 0,
                    verbose_data: None,
                },
            },
            plan,
            instantiation,
        };
        let info = NetworkInfo {
            network: TN12_NETWORK_ID.to_owned(),
            node_version: "test".to_owned(),
            rpc_api_version: 1,
            rpc_api_revision: 0,
            is_synced: true,
            has_utxo_index: true,
            virtual_daa_score: 1,
            block_count: 1,
            header_count: 1,
        };
        let proof = TestnetProof::from_run(&run, &info, None, None, ProofResult::Gated, None)
            .expect("proof");
        let json = serde_json::to_string(&proof).expect("json");

        assert!(!json.contains(KASPA_TESTNET_PRIVATE_KEY_ENV));
        assert!(!json.contains("private"));
        assert!(json.contains("\"gated\""));
    }

    #[test]
    fn malformed_artifact_fails_instantiation() {
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
        let err = instantiate_contract(
            &artifact,
            "Missing",
            "spend",
            BTreeMap::new(),
            TestnetTarget::Tn10,
        )
        .expect_err("malformed artifact");
        assert!(matches!(err, TestnetError::Artifact(_)));
    }

    #[test]
    fn invalid_signature_is_rejected_by_script_engine() {
        let wallet = TestWallet::generate_ephemeral().expect("wallet");
        let source = include_str!("../../tests/contracts/timelock.ks");
        let plan =
            ContractDeploymentPlan::from_source("Timelock", "timelock.ks", source).expect("plan");
        let params = default_contract_params(&plan.artifact, "Timelock", &wallet).expect("params");
        let instantiation = instantiate_contract(
            &plan.artifact,
            "Timelock",
            "claim",
            params,
            TestnetTarget::Tn10,
        )
        .expect("instantiate");

        let outpoint = TransactionOutpoint::new(TransactionId::from([1u8; 32]), 0);
        let entry = UtxoEntry::new(100_000, instantiation.script_public_key.clone(), 0, false);
        let output = TransactionOutput::new(99_000, pay_to_address_script(wallet.address()));
        let mut tx = Transaction::new(
            TX_VERSION,
            vec![TransactionInput::new(outpoint, Vec::new(), 0, 0)],
            vec![output],
            0,
            SUBNETWORK_ID_NATIVE,
            0,
            Vec::new(),
        );
        let bad_payload = vec![1u8; 65];
        tx.inputs[0].signature_script =
            build_contract_signature_script(&instantiation.redeem_script, &[bad_payload])
                .expect("sig script");
        tx.inputs[0].sig_op_count =
            p2sh_sig_op_count(&tx.inputs[0].signature_script, &entry.script_public_key)
                .expect("sigops");
        tx.finalize();

        let err = validate_scripts(&tx, &[entry]).expect_err("invalid signature rejected");
        assert!(matches!(err, TestnetError::Script(_)));
    }
}
