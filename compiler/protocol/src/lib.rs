use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Network {
    Tn12,
    Mainnet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ProtocolFeature {
    BaseScript,
    TransactionIntrospection,
    CovenantIds,
    SequencingCommitments,
    ZkVerification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeatureStatus {
    Stable,
    ActiveTestnet,
    PendingActivation,
    Unpinned,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureSpec {
    pub feature: ProtocolFeature,
    pub status: FeatureStatus,
    pub source: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolLimits {
    pub max_source_bytes: usize,
    pub max_contracts: usize,
    pub max_params_per_contract: usize,
    pub max_spends_per_contract: usize,
    pub max_spend_params: usize,
    pub max_requires_per_spend: usize,
    pub max_expression_depth: usize,
}

impl Default for ProtocolLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 256 * 1024,
            max_contracts: 128,
            max_params_per_contract: 256,
            max_spends_per_contract: 128,
            max_spend_params: 64,
            max_requires_per_spend: 256,
            max_expression_depth: 64,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolManifest {
    pub name: &'static str,
    pub network: Network,
    pub source_commit: Option<&'static str>,
    pub features: Vec<FeatureSpec>,
    pub limits: ProtocolLimits,
    pub bytecode_emission_allowed: bool,
}

impl ProtocolManifest {
    pub fn supports(&self, feature: ProtocolFeature) -> bool {
        self.features.iter().any(|spec| {
            spec.feature == feature
                && !matches!(
                    spec.status,
                    FeatureStatus::Unpinned | FeatureStatus::Unsupported
                )
        })
    }

    pub fn feature_status(&self, feature: ProtocolFeature) -> Option<FeatureStatus> {
        self.features
            .iter()
            .find(|spec| spec.feature == feature)
            .map(|spec| spec.status)
    }

    pub fn validate_requirements(
        &self,
        requirements: &[ProtocolFeature],
    ) -> Result<(), ProtocolError> {
        let mut missing = Vec::new();
        let mut unpinned = Vec::new();

        for requirement in requirements {
            match self.feature_status(*requirement) {
                Some(FeatureStatus::Unpinned | FeatureStatus::Unsupported) => {
                    unpinned.push(*requirement);
                }
                Some(_) => {}
                None => missing.push(*requirement),
            }
        }

        if !missing.is_empty() {
            return Err(ProtocolError::MissingFeatures(missing));
        }

        if !unpinned.is_empty() {
            return Err(ProtocolError::UnpinnedFeatures(unpinned));
        }

        Ok(())
    }

    pub fn ensure_bytecode_emission_allowed(&self) -> Result<(), ProtocolError> {
        if self.bytecode_emission_allowed {
            Ok(())
        } else {
            Err(ProtocolError::BytecodeEmissionLocked {
                manifest: self.name,
            })
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    MissingFeatures(Vec<ProtocolFeature>),
    UnpinnedFeatures(Vec<ProtocolFeature>),
    BytecodeEmissionLocked { manifest: &'static str },
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProtocolError::MissingFeatures(features) => {
                write!(f, "protocol target does not support: ")?;
                format_features(f, features)
            }
            ProtocolError::UnpinnedFeatures(features) => {
                write!(f, "protocol target has unpinned feature definitions: ")?;
                format_features(f, features)
            }
            ProtocolError::BytecodeEmissionLocked { manifest } => write!(
                f,
                "bytecode emission is locked for protocol manifest `{manifest}` until consensus opcode definitions are pinned"
            ),
        }
    }
}

impl std::error::Error for ProtocolError {}

pub fn toccata_tn12_manifest() -> ProtocolManifest {
    verified_tn12_manifest()
}

pub fn verified_tn12_manifest() -> ProtocolManifest {
    ProtocolManifest {
        name: "verified-tn12",
        network: Network::Tn12,
        source_commit: Some("rusty-kaspa:a07d8b38d45f38a02a1f35f601e874358f6c7846"),
        features: vec![
            FeatureSpec {
                feature: ProtocolFeature::BaseScript,
                status: FeatureStatus::ActiveTestnet,
                source: "kaspanet/rusty-kaspa crypto/txscript/src/opcodes/mod.rs",
            },
            FeatureSpec {
                feature: ProtocolFeature::TransactionIntrospection,
                status: FeatureStatus::ActiveTestnet,
                source: "kaspanet/kips kip-0010.md and rusty-kaspa txscript opcodes",
            },
            FeatureSpec {
                feature: ProtocolFeature::CovenantIds,
                status: FeatureStatus::Unsupported,
                source: "unsupported for verified-tn12; current Toccata sources are not lowered by KaspaScript",
            },
            FeatureSpec {
                feature: ProtocolFeature::SequencingCommitments,
                status: FeatureStatus::Stable,
                source: "kaspanet/kips kip-0015.md block-header commitment, not script opcode",
            },
            FeatureSpec {
                feature: ProtocolFeature::ZkVerification,
                status: FeatureStatus::Unsupported,
                source: "unsupported for verified-tn12; current Toccata ZK precompile stack ABI is not lowered by KaspaScript",
            },
        ],
        limits: ProtocolLimits::default(),
        bytecode_emission_allowed: true,
    }
}

pub fn toccata_preview_manifest() -> ProtocolManifest {
    let mut manifest = verified_tn12_manifest();
    manifest.name = "toccata-preview";
    manifest.bytecode_emission_allowed = true;
    for feature in &mut manifest.features {
        if matches!(
            feature.feature,
            ProtocolFeature::CovenantIds | ProtocolFeature::ZkVerification
        ) {
            feature.status = FeatureStatus::Unpinned;
            feature.source =
                "preview-gated; current upstream Toccata source exists, KaspaScript lowering is not implemented";
        }
    }
    if let Some(feature) = manifest
        .features
        .iter_mut()
        .find(|feature| feature.feature == ProtocolFeature::SequencingCommitments)
    {
        feature.source =
            "kaspanet/kips kip-0021 and rusty-kaspa OpChainblockSeqCommit; contract lowering remains gated";
    }
    manifest
}

pub fn future_mainnet_manifest() -> ProtocolManifest {
    let mut manifest = verified_tn12_manifest();
    manifest.name = "future-mainnet";
    manifest.network = Network::Mainnet;
    manifest.bytecode_emission_allowed = false;
    manifest
}

fn format_features(f: &mut fmt::Formatter<'_>, features: &[ProtocolFeature]) -> fmt::Result {
    for (index, feature) in features.iter().enumerate() {
        if index > 0 {
            write!(f, ", ")?;
        }
        write!(f, "{feature}")?;
    }
    Ok(())
}

impl fmt::Display for ProtocolFeature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            ProtocolFeature::BaseScript => "base-script",
            ProtocolFeature::TransactionIntrospection => "transaction-introspection",
            ProtocolFeature::CovenantIds => "covenant-ids",
            ProtocolFeature::SequencingCommitments => "sequencing-commitments",
            ProtocolFeature::ZkVerification => "zk-verification",
        };
        f.write_str(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verified_tn12_manifest_allows_bytecode_for_pinned_features() {
        let manifest = verified_tn12_manifest();

        assert_eq!(manifest.ensure_bytecode_emission_allowed(), Ok(()));
        assert!(manifest.supports(ProtocolFeature::TransactionIntrospection));
        assert!(!manifest.supports(ProtocolFeature::CovenantIds));
    }

    #[test]
    fn reports_unpinned_or_unsupported_future_requirements() {
        let manifest = toccata_preview_manifest();
        let requirements = [
            ProtocolFeature::BaseScript,
            ProtocolFeature::CovenantIds,
            ProtocolFeature::SequencingCommitments,
        ];

        assert_eq!(
            manifest.validate_requirements(&requirements),
            Err(ProtocolError::UnpinnedFeatures(vec![
                ProtocolFeature::CovenantIds
            ]))
        );
    }

    #[test]
    fn base_script_is_available_for_frontend_analysis() {
        let manifest = toccata_tn12_manifest();

        assert!(manifest.supports(ProtocolFeature::BaseScript));
        assert_eq!(
            manifest.feature_status(ProtocolFeature::BaseScript),
            Some(FeatureStatus::ActiveTestnet)
        );
    }
}
