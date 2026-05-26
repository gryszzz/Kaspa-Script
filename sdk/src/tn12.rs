//! TN12 integration harness for KaspaScript.
//!
//! This module is feature-gated because it connects to live Kaspa Testnet 12
//! infrastructure through Rusty Kaspa wRPC crates. Offline compiler tests do not
//! depend on it.

use std::env;
use std::fmt;
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use kaspa_addresses::{Address, Prefix, Version};
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_rpc_core::prelude::{
    RpcFeeEstimate, RpcMempoolEntry, RpcTransaction, RpcTransactionId, RpcUtxosByAddressesEntry,
};
use kaspa_wrpc_client::prelude::{NetworkId, NetworkType};
use kaspa_wrpc_client::{KaspaRpcClient, WrpcEncoding};
use kaspascript_codegen::{bytecode_hex, compile_file, verify_artifact, CompiledArtifact};
use secp256k1::{Keypair, Message, Secp256k1, SecretKey, XOnlyPublicKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Environment variable containing a TN12 wRPC URL.
pub const TN12_RPC_URL_ENV: &str = "KASPA_TN12_RPC_URL";
/// Environment variable containing a 32-byte testnet private key in hex.
pub const TN12_PRIVATE_KEY_ENV: &str = "KASPA_TN12_PRIVATE_KEY";
/// Optional environment variable containing a funded testnet faucet address.
pub const TN12_FAUCET_ADDRESS_ENV: &str = "KASPA_TN12_FAUCET_ADDRESS";
/// Expected Kaspa network ID for this harness.
pub const TN12_NETWORK_ID: &str = "testnet-12";

const DEFAULT_POLL_ATTEMPTS: usize = 120;
const DEFAULT_POLL_INTERVAL_MS: u64 = 1_000;
const TX_BACKEND_GATE: &str = "contract lock/spend broadcasting is gated: the current SDK transaction builder is preview-only and cannot yet construct, instantiate, or sign real Rusty Kaspa transactions from KaspaScript artifacts";

/// TN12 harness error.
#[derive(Debug, Error)]
pub enum Tn12Error {
    /// A required environment variable is missing.
    #[error("missing environment variable `{0}`")]
    MissingEnv(&'static str),
    /// An environment variable was present but invalid.
    #[error("invalid environment variable `{var}`: {message}")]
    InvalidEnv { var: &'static str, message: String },
    /// The connected node is not Testnet 12.
    #[error("refusing non-TN12 node: expected `{expected}`, got `{actual}`")]
    NetworkMismatch { expected: String, actual: String },
    /// The connected node cannot serve indexed UTXO queries.
    #[error("connected node does not expose the UTXO index")]
    UtxoIndexDisabled,
    /// The wallet does not have a spendable UTXO matching the request.
    #[error(
        "no spendable UTXO found for minimum {minimum} sompi and {min_confirmations} confirmations"
    )]
    NoSpendableUtxo {
        minimum: u64,
        min_confirmations: u64,
    },
    /// The live RPC call failed.
    #[error("TN12 RPC error: {0}")]
    Rpc(String),
    /// The KaspaScript compiler rejected the contract.
    #[error("compile error: {0}")]
    Compile(String),
    /// JSON serialization or deserialization failed.
    #[error("JSON error: {0}")]
    Json(String),
    /// Filesystem IO failed.
    #[error("IO error: {0}")]
    Io(String),
    /// A requested operation is intentionally gated.
    #[error("unsupported TN12 operation: {0}")]
    Unsupported(&'static str),
    /// Raw data supplied to the harness is invalid.
    #[error("invalid data: {0}")]
    InvalidData(String),
}

/// Runtime configuration for live TN12 tests.
#[derive(Clone)]
pub struct Tn12Config {
    /// wRPC URL for a TN12 node, for example `ws://127.0.0.1:17210`.
    pub rpc_url: String,
    /// Optional funded faucet address used by test scripts and docs.
    pub faucet_address: Option<Address>,
    /// Number of status-poll attempts used by live tests.
    pub poll_attempts: usize,
    /// Delay between status-poll attempts.
    pub poll_interval: Duration,
}

impl fmt::Debug for Tn12Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tn12Config")
            .field("rpc_url", &self.rpc_url)
            .field("faucet_address", &self.faucet_address)
            .field("poll_attempts", &self.poll_attempts)
            .field("poll_interval", &self.poll_interval)
            .finish()
    }
}

impl Tn12Config {
    /// Loads TN12 configuration from environment variables.
    pub fn from_env() -> Result<Self, Tn12Error> {
        let rpc_url = read_required_env(TN12_RPC_URL_ENV)?;
        let faucet_address = read_optional_address(TN12_FAUCET_ADDRESS_ENV)?;
        let poll_attempts =
            read_optional_usize("KASPA_TN12_POLL_ATTEMPTS")?.unwrap_or(DEFAULT_POLL_ATTEMPTS);
        let poll_interval_ms =
            read_optional_u64("KASPA_TN12_POLL_INTERVAL_MS")?.unwrap_or(DEFAULT_POLL_INTERVAL_MS);

        Ok(Self {
            rpc_url,
            faucet_address,
            poll_attempts,
            poll_interval: Duration::from_millis(poll_interval_ms),
        })
    }

    /// Creates a config directly from a wRPC URL.
    pub fn new(rpc_url: impl Into<String>) -> Self {
        Self {
            rpc_url: rpc_url.into(),
            faucet_address: None,
            poll_attempts: DEFAULT_POLL_ATTEMPTS,
            poll_interval: Duration::from_millis(DEFAULT_POLL_INTERVAL_MS),
        }
    }
}

/// Live network metadata returned by a TN12 node.
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
pub struct Tn12Utxo {
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

/// Transaction status as observed by the TN12 RPC adapter.
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

/// Live TN12 wRPC adapter.
pub struct Tn12RpcClient {
    client: KaspaRpcClient,
}

impl fmt::Debug for Tn12RpcClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tn12RpcClient").finish_non_exhaustive()
    }
}

impl Tn12RpcClient {
    /// Creates a TN12 RPC client without connecting.
    pub fn new(config: &Tn12Config) -> Result<Self, Tn12Error> {
        let network = NetworkId::with_suffix(NetworkType::Testnet, 12);
        let client = KaspaRpcClient::new(
            WrpcEncoding::Borsh,
            Some(config.rpc_url.as_str()),
            None,
            Some(network),
            None,
        )
        .map_err(|error| Tn12Error::Rpc(error.to_string()))?;

        Ok(Self { client })
    }

    /// Connects to the configured node and hard-fails unless it is TN12.
    pub async fn connect(config: &Tn12Config) -> Result<Self, Tn12Error> {
        let adapter = Self::new(config)?;
        adapter
            .client
            .connect(None)
            .await
            .map_err(|error| Tn12Error::Rpc(error.to_string()))?;
        adapter.assert_tn12().await?;
        Ok(adapter)
    }

    /// Disconnects the underlying wRPC client.
    pub async fn disconnect(&self) -> Result<(), Tn12Error> {
        self.client
            .disconnect()
            .await
            .map_err(|error| Tn12Error::Rpc(error.to_string()))
    }

    /// Fetches network and node metadata.
    pub async fn network_info(&self) -> Result<NetworkInfo, Tn12Error> {
        let server = self
            .client
            .get_server_info()
            .await
            .map_err(|error| Tn12Error::Rpc(error.to_string()))?;
        let dag = self
            .client
            .get_block_dag_info()
            .await
            .map_err(|error| Tn12Error::Rpc(error.to_string()))?;

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

    /// Hard-fails if the connected node is not Testnet 12.
    pub async fn assert_tn12(&self) -> Result<NetworkInfo, Tn12Error> {
        let info = self.network_info().await?;
        if info.network == TN12_NETWORK_ID {
            Ok(info)
        } else {
            Err(Tn12Error::NetworkMismatch {
                expected: TN12_NETWORK_ID.to_owned(),
                actual: info.network,
            })
        }
    }

    /// Fetches all indexed UTXOs for an address.
    pub async fn fetch_utxos(&self, address: &Address) -> Result<Vec<Tn12Utxo>, Tn12Error> {
        let info = self.assert_tn12().await?;
        if !info.has_utxo_index {
            return Err(Tn12Error::UtxoIndexDisabled);
        }

        let entries = self
            .client
            .get_utxos_by_addresses(vec![address.clone()])
            .await
            .map_err(|error| Tn12Error::Rpc(error.to_string()))?;

        Ok(entries
            .into_iter()
            .map(|entry| normalize_utxo(entry, info.virtual_daa_score))
            .collect())
    }

    /// Fetches the indexed balance for an address.
    pub async fn balance(&self, address: &Address) -> Result<u64, Tn12Error> {
        let info = self.assert_tn12().await?;
        if !info.has_utxo_index {
            return Err(Tn12Error::UtxoIndexDisabled);
        }

        self.client
            .get_balance_by_address(address.clone())
            .await
            .map_err(|error| Tn12Error::Rpc(error.to_string()))
    }

    /// Fetches the node fee estimate.
    pub async fn estimate_fees(&self) -> Result<FeeEstimateSnapshot, Tn12Error> {
        self.client
            .get_fee_estimate()
            .await
            .map(FeeEstimateSnapshot::from)
            .map_err(|error| Tn12Error::Rpc(error.to_string()))
    }

    /// Fetches the node fee estimate, returning `None` when the node does not expose it.
    pub async fn estimate_fees_if_supported(
        &self,
    ) -> Result<Option<FeeEstimateSnapshot>, Tn12Error> {
        match self.client.get_fee_estimate().await {
            Ok(estimate) => Ok(Some(estimate.into())),
            Err(error) => {
                let message = error.to_string();
                if message.to_ascii_lowercase().contains("not found")
                    || message.to_ascii_lowercase().contains("not supported")
                    || message.to_ascii_lowercase().contains("unimplemented")
                {
                    Ok(None)
                } else {
                    Err(Tn12Error::Rpc(message))
                }
            }
        }
    }

    /// Submits a typed Rusty Kaspa RPC transaction to TN12.
    pub async fn submit_transaction(
        &self,
        transaction: RpcTransaction,
        allow_orphan: bool,
    ) -> Result<RpcTransactionId, Tn12Error> {
        self.assert_tn12().await?;
        self.client
            .submit_transaction(transaction, allow_orphan)
            .await
            .map_err(|error| Tn12Error::Rpc(error.to_string()))
    }

    /// Submits raw transaction bytes.
    ///
    /// Rusty Kaspa's `kaspa-rpc-core` submit API accepts a typed
    /// `RpcTransaction`, not arbitrary raw bytes. This method exists so callers
    /// get a deterministic hard failure instead of an unsafe ad-hoc decoder.
    pub async fn submit_raw_transaction(
        &self,
        raw_transaction: &[u8],
    ) -> Result<String, Tn12Error> {
        self.assert_tn12().await?;
        if raw_transaction.is_empty() {
            return Err(Tn12Error::InvalidData(
                "raw transaction bytes cannot be empty".to_owned(),
            ));
        }
        Err(Tn12Error::Unsupported(
            "raw transaction submission is not exposed by kaspa-rpc-core; submit a typed RpcTransaction after the real transaction builder is implemented",
        ))
    }

    /// Polls the mempool status for a transaction ID.
    pub async fn poll_transaction_status(
        &self,
        transaction_id: RpcTransactionId,
    ) -> Result<TransactionStatus, Tn12Error> {
        self.assert_tn12().await?;
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
        config: &Tn12Config,
    ) -> Result<TransactionStatus, Tn12Error> {
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

    /// Waits for transaction confirmation.
    ///
    /// This remains gated until the harness wires an accepted-transaction index
    /// or block subscription source that proves inclusion for a specific txid.
    pub async fn wait_for_confirmation(
        &self,
        _transaction_id: RpcTransactionId,
        _config: &Tn12Config,
    ) -> Result<TransactionStatus, Tn12Error> {
        self.assert_tn12().await?;
        Err(Tn12Error::Unsupported(
            "confirmation polling requires accepted-transaction indexing and is not implemented yet",
        ))
    }

    /// Waits for a transaction to satisfy a finality depth.
    pub async fn wait_for_finality(
        &self,
        _transaction_id: RpcTransactionId,
        _finality_depth: u64,
        _config: &Tn12Config,
    ) -> Result<TransactionStatus, Tn12Error> {
        self.assert_tn12().await?;
        Err(Tn12Error::Unsupported(
            "finality polling requires proven transaction inclusion before DAA-depth tracking",
        ))
    }
}

/// Testnet wallet used by ignored TN12 integration tests.
pub struct TestWallet {
    secret_key: SecretKey,
    keypair: Keypair,
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
    /// Loads a test wallet from `KASPA_TN12_PRIVATE_KEY`.
    pub fn from_env() -> Result<Self, Tn12Error> {
        let key = read_required_env(TN12_PRIVATE_KEY_ENV)?;
        Self::from_private_key_hex(&key)
    }

    /// Loads a test wallet from a 32-byte private key hex string.
    pub fn from_private_key_hex(private_key_hex: &str) -> Result<Self, Tn12Error> {
        let secret_bytes =
            decode_hex_32(private_key_hex).map_err(|message| Tn12Error::InvalidEnv {
                var: TN12_PRIVATE_KEY_ENV,
                message,
            })?;
        let secret_key =
            SecretKey::from_slice(&secret_bytes).map_err(|error| Tn12Error::InvalidEnv {
                var: TN12_PRIVATE_KEY_ENV,
                message: error.to_string(),
            })?;
        Self::from_secret_key(secret_key)
    }

    /// Generates an ephemeral test wallet.
    pub fn generate_ephemeral() -> Result<Self, Tn12Error> {
        let mut rng = secp256k1::rand::thread_rng();
        let secret_key = SecretKey::new(&mut rng);
        Self::from_secret_key(secret_key)
    }

    /// Returns the derived TN12 address.
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

    /// Lists wallet balance through the TN12 RPC adapter.
    pub async fn list_balance(&self, rpc: &Tn12RpcClient) -> Result<u64, Tn12Error> {
        rpc.balance(&self.address).await
    }

    /// Selects a spendable UTXO matching minimum value and confirmation rules.
    pub async fn select_spendable_utxo(
        &self,
        rpc: &Tn12RpcClient,
        minimum: u64,
        min_confirmations: u64,
    ) -> Result<Tn12Utxo, Tn12Error> {
        let utxos = rpc.fetch_utxos(&self.address).await?;
        utxos
            .into_iter()
            .filter(|utxo| !utxo.is_coinbase)
            .filter(|utxo| utxo.amount >= minimum)
            .filter(|utxo| utxo.confirmations >= min_confirmations)
            .max_by_key(|utxo| utxo.amount)
            .ok_or(Tn12Error::NoSpendableUtxo {
                minimum,
                min_confirmations,
            })
    }

    /// Signs a 32-byte transaction digest with BIP340 Schnorr.
    pub fn sign_spend_digest(&self, digest: [u8; 32]) -> [u8; 64] {
        let secp = Secp256k1::new();
        let message = Message::from_digest(digest);
        let signature = secp.sign_schnorr(&message, &self.keypair);
        signature.serialize()
    }

    fn from_secret_key(secret_key: SecretKey) -> Result<Self, Tn12Error> {
        let secp = Secp256k1::new();
        let keypair = Keypair::from_secret_key(&secp, &secret_key);
        let (xonly, _) = XOnlyPublicKey::from_keypair(&keypair);
        let public_key = xonly.serialize();
        let address = Address::new(Prefix::Testnet, Version::PubKey, &public_key);

        Ok(Self {
            secret_key,
            keypair,
            address,
            public_key,
        })
    }
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
    /// Script SHA-256 as lowercase hex.
    pub script_hash: String,
    /// Generated script bytes as lowercase hex.
    pub script_hex: String,
    /// Compiled artifact metadata.
    pub artifact: CompiledArtifact,
}

impl ContractDeploymentPlan {
    /// Compiles a KaspaScript contract and creates a deterministic deployment plan.
    pub fn from_source(contract_name: &str, file: &str, source: &str) -> Result<Self, Tn12Error> {
        let artifact =
            compile_file(source, file).map_err(|error| Tn12Error::Compile(error.to_string()))?;
        verify_artifact(&artifact).map_err(|error| Tn12Error::Compile(error.to_string()))?;
        let artifact_bytes =
            serde_json::to_vec(&artifact).map_err(|error| Tn12Error::Json(error.to_string()))?;
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

/// Result category stored in TN12 proof files.
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

/// TN12 proof file emitted by live integration tests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tn12Proof {
    /// Contract name.
    pub contract_name: String,
    /// Source SHA-256 as lowercase hex.
    pub source_hash: String,
    /// Artifact SHA-256 as lowercase hex.
    pub artifact_hash: String,
    /// Generated script bytes as lowercase hex.
    pub script_hex: String,
    /// Locking transaction ID when broadcast succeeds.
    pub lock_txid: Option<String>,
    /// Spend transaction ID when broadcast succeeds.
    pub spend_txid: Option<String>,
    /// Network reported by the node.
    pub network: String,
    /// Node version reported by the node.
    pub node_version: String,
    /// Unix timestamp in seconds.
    pub timestamp: u64,
    /// Pass/fail/gated outcome.
    pub result: ProofResult,
    /// Failure or gate reason.
    pub error: Option<String>,
}

impl Tn12Proof {
    /// Builds a proof from a deployment plan and live node metadata.
    pub fn from_plan(
        plan: &ContractDeploymentPlan,
        info: &NetworkInfo,
        lock_txid: Option<String>,
        spend_txid: Option<String>,
        result: ProofResult,
        error: Option<String>,
    ) -> Result<Self, Tn12Error> {
        Ok(Self {
            contract_name: plan.contract_name.clone(),
            source_hash: plan.source_hash.clone(),
            artifact_hash: plan.artifact_hash.clone(),
            script_hex: plan.script_hex.clone(),
            lock_txid,
            spend_txid,
            network: info.network.clone(),
            node_version: info.node_version.clone(),
            timestamp: unix_timestamp()?,
            result,
            error,
        })
    }

    /// Writes the proof JSON to disk.
    pub fn write_json(&self, path: impl AsRef<Path>) -> Result<(), Tn12Error> {
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent).map_err(|error| Tn12Error::Io(error.to_string()))?;
        }
        let json =
            serde_json::to_vec_pretty(self).map_err(|error| Tn12Error::Json(error.to_string()))?;
        fs::write(path, json).map_err(|error| Tn12Error::Io(error.to_string()))
    }
}

/// Live contract flow harness.
#[derive(Debug)]
pub struct Tn12ContractHarness<'a> {
    rpc: &'a Tn12RpcClient,
    wallet: &'a TestWallet,
}

impl<'a> Tn12ContractHarness<'a> {
    /// Creates a live contract flow harness.
    pub fn new(rpc: &'a Tn12RpcClient, wallet: &'a TestWallet) -> Self {
        Self { rpc, wallet }
    }

    /// Compiles a contract, checks TN12 safety, and gates unsupported broadcast flow.
    pub async fn deploy_and_execute(
        &self,
        contract_name: &str,
        file: &str,
        source: &str,
    ) -> Result<Tn12Proof, Tn12Error> {
        let _plan = ContractDeploymentPlan::from_source(contract_name, file, source)?;
        self.rpc.assert_tn12().await?;
        let _balance = self.wallet.list_balance(self.rpc).await?;
        Err(Tn12Error::Unsupported(TX_BACKEND_GATE))
    }

    /// Builds a gated proof without broadcasting.
    pub async fn gated_proof(
        &self,
        contract_name: &str,
        file: &str,
        source: &str,
    ) -> Result<Tn12Proof, Tn12Error> {
        let plan = ContractDeploymentPlan::from_source(contract_name, file, source)?;
        let info = self.rpc.assert_tn12().await?;
        Tn12Proof::from_plan(
            &plan,
            &info,
            None,
            None,
            ProofResult::Gated,
            Some(TX_BACKEND_GATE.to_owned()),
        )
    }
}

fn read_required_env(name: &'static str) -> Result<String, Tn12Error> {
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => Ok(value),
        Ok(_) | Err(_) => Err(Tn12Error::MissingEnv(name)),
    }
}

fn read_optional_address(name: &'static str) -> Result<Option<Address>, Tn12Error> {
    let value = match env::var(name) {
        Ok(value) if !value.trim().is_empty() => value,
        Ok(_) | Err(_) => return Ok(None),
    };
    let address = Address::try_from(value.trim()).map_err(|error| Tn12Error::InvalidEnv {
        var: name,
        message: error.to_string(),
    })?;
    if address.prefix != Prefix::Testnet {
        return Err(Tn12Error::InvalidEnv {
            var: name,
            message: "address must use the kaspatest prefix".to_owned(),
        });
    }
    Ok(Some(address))
}

fn read_optional_usize(name: &'static str) -> Result<Option<usize>, Tn12Error> {
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => {
            value
                .parse::<usize>()
                .map(Some)
                .map_err(|error| Tn12Error::InvalidEnv {
                    var: name,
                    message: error.to_string(),
                })
        }
        Ok(_) | Err(_) => Ok(None),
    }
}

fn read_optional_u64(name: &'static str) -> Result<Option<u64>, Tn12Error> {
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => {
            value
                .parse::<u64>()
                .map(Some)
                .map_err(|error| Tn12Error::InvalidEnv {
                    var: name,
                    message: error.to_string(),
                })
        }
        Ok(_) | Err(_) => Ok(None),
    }
}

fn normalize_utxo(entry: RpcUtxosByAddressesEntry, virtual_daa_score: u64) -> Tn12Utxo {
    let confirmations = virtual_daa_score
        .saturating_sub(entry.utxo_entry.block_daa_score)
        .saturating_add(1);
    Tn12Utxo {
        outpoint: format!("{}:{}", entry.outpoint.transaction_id, entry.outpoint.index),
        amount: entry.utxo_entry.amount,
        block_daa_score: entry.utxo_entry.block_daa_score,
        confirmations,
        is_coinbase: entry.utxo_entry.is_coinbase,
        script_public_key_hex: hex_encode(entry.utxo_entry.script_public_key.script()),
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

fn unix_timestamp() -> Result<u64, Tn12Error> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| Tn12Error::InvalidData(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_key_hex_must_be_32_bytes() {
        let error = TestWallet::from_private_key_hex("abcd").expect_err("short key rejected");
        assert!(matches!(error, Tn12Error::InvalidEnv { .. }));
    }

    #[test]
    fn ephemeral_wallet_uses_testnet_address_prefix() {
        let wallet = TestWallet::generate_ephemeral().expect("wallet");
        assert_eq!(wallet.address().prefix, Prefix::Testnet);
        assert!(wallet.address_string().starts_with("kaspatest:"));
        assert_eq!(wallet.public_key().len(), 32);
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
    fn proof_json_never_contains_private_key_material() {
        let source = include_str!("../../tests/contracts/timelock.ks");
        let plan =
            ContractDeploymentPlan::from_source("timelock", "timelock.ks", source).expect("plan");
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
        let proof = Tn12Proof::from_plan(
            &plan,
            &info,
            None,
            None,
            ProofResult::Gated,
            Some(TX_BACKEND_GATE.to_owned()),
        )
        .expect("proof");
        let json = serde_json::to_string(&proof).expect("json");

        assert!(!json.contains(TN12_PRIVATE_KEY_ENV));
        assert!(!json.contains("private"));
        assert!(json.contains("\"gated\""));
    }
}
