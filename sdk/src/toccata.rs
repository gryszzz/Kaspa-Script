//! Toccata compatibility facade types.
//!
//! These types preserve the Toccata v1 transaction fields KaspaScript must
//! reason about before it wires directly into Rusty Kaspa transaction builders.
//! They are fixture and planning surfaces, not consensus transaction types.

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const TOCCATA_COMPATIBILITY_FIXTURE_SCHEMA_VERSION: &str =
    "kaspascript.sdk.toccata.compatibility-fixtures.v0";
pub const TOCCATA_TRANSACTION_FACADE_SCHEMA_VERSION: &str =
    "kaspascript.sdk.toccata.transaction-facade.v0";
pub const RUSTY_KASPA_TOCCATA_COMPAT_TAG: &str = "v2.0.1";
pub const RUSTY_KASPA_TOCCATA_COMPAT_COMMIT: &str = "cfafeb4c093fa37a303f1b9f19c58f986b870ce3";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToccataCompatibilityFixtures {
    pub schema_version: String,
    pub source_release: UpstreamReleasePin,
    pub transaction_facade: ToccataTransactionFacade,
    pub seq_commit_lane_proof: SeqCommitLaneProofFixture,
    pub readiness: FixtureReadiness,
}

impl ToccataCompatibilityFixtures {
    pub fn validate(&self) -> Result<(), ToccataFacadeError> {
        if self.schema_version != TOCCATA_COMPATIBILITY_FIXTURE_SCHEMA_VERSION {
            return Err(ToccataFacadeError::SchemaVersionMismatch {
                expected: TOCCATA_COMPATIBILITY_FIXTURE_SCHEMA_VERSION,
                actual: self.schema_version.clone(),
            });
        }
        if self.source_release.tag != RUSTY_KASPA_TOCCATA_COMPAT_TAG {
            return Err(ToccataFacadeError::ReleaseTagMismatch {
                expected: RUSTY_KASPA_TOCCATA_COMPAT_TAG,
                actual: self.source_release.tag.clone(),
            });
        }

        self.transaction_facade.validate()?;

        let lane_target = self
            .transaction_facade
            .lane_target
            .as_ref()
            .ok_or(ToccataFacadeError::MissingLaneTarget)?;
        if lane_target.lane_id != self.seq_commit_lane_proof.request.lane_id {
            return Err(ToccataFacadeError::LaneMismatch {
                transaction_lane: lane_target.lane_id,
                proof_lane: self.seq_commit_lane_proof.request.lane_id,
            });
        }

        if !self
            .transaction_facade
            .proof_requirements
            .iter()
            .any(|requirement| requirement.kind == ProofRequirementKind::SeqCommitLaneProof)
        {
            return Err(ToccataFacadeError::MissingSeqCommitProofRequirement);
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpstreamReleasePin {
    pub repo: String,
    pub tag: String,
    pub commit: String,
    pub published_at: String,
    pub role: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToccataTransactionFacade {
    pub schema_version: String,
    pub rusty_kaspa_tag: String,
    pub transaction_version: u16,
    pub storage_mass: u64,
    pub builder_status: String,
    pub inputs: Vec<ToccataInputFacade>,
    pub outputs: Vec<ToccataOutputFacade>,
    pub lane_target: Option<UserLaneTarget>,
    pub proof_requirements: Vec<ProofRequirement>,
}

impl ToccataTransactionFacade {
    pub fn validate(&self) -> Result<(), ToccataFacadeError> {
        if self.schema_version != TOCCATA_TRANSACTION_FACADE_SCHEMA_VERSION {
            return Err(ToccataFacadeError::SchemaVersionMismatch {
                expected: TOCCATA_TRANSACTION_FACADE_SCHEMA_VERSION,
                actual: self.schema_version.clone(),
            });
        }
        if self.rusty_kaspa_tag != RUSTY_KASPA_TOCCATA_COMPAT_TAG {
            return Err(ToccataFacadeError::ReleaseTagMismatch {
                expected: RUSTY_KASPA_TOCCATA_COMPAT_TAG,
                actual: self.rusty_kaspa_tag.clone(),
            });
        }
        if self.transaction_version != 1 {
            return Err(ToccataFacadeError::UnsupportedTransactionVersion(
                self.transaction_version,
            ));
        }
        if self.storage_mass == 0 {
            return Err(ToccataFacadeError::MissingStorageMass);
        }
        if self
            .inputs
            .iter()
            .any(|input| input.compute_commit.is_empty())
        {
            return Err(ToccataFacadeError::MissingInputComputeCommit);
        }
        if !self.outputs.iter().any(|output| output.covenant.is_some()) {
            return Err(ToccataFacadeError::MissingCovenantBinding);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToccataInputFacade {
    pub previous_outpoint: String,
    pub sequence: u64,
    pub compute_commit: String,
    pub role: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToccataOutputFacade {
    pub value: u64,
    pub script_public_key_hex: String,
    pub covenant: Option<CovenantBindingFacade>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CovenantBindingFacade {
    pub covenant_id: String,
    pub payload_hash: String,
    pub state_index: u32,
    pub source_field: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserLaneTarget {
    pub lane_id: u64,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProofRequirement {
    pub kind: ProofRequirementKind,
    pub source: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofRequirementKind {
    SeqCommitLaneProof,
    CovenantBinding,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeqCommitLaneProofFixture {
    pub rpc_surface: SeqCommitRpcSurface,
    pub source: String,
    pub request: SeqCommitLaneProofRequest,
    pub response: SeqCommitLaneProofResponse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeqCommitRpcSurface {
    GrpcAndWrpc,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeqCommitLaneProofRequest {
    pub daa_score: u64,
    pub lane_id: u64,
    pub block_hash: String,
    pub accepted_id_merkle_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeqCommitLaneProofResponse {
    pub lane_root: String,
    pub proof_hash: String,
    pub item_count: u64,
    pub inactive_lane_shortcut: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureReadiness {
    pub status: String,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ToccataFacadeError {
    #[error("schema version `{actual}` does not match expected `{expected}`")]
    SchemaVersionMismatch {
        expected: &'static str,
        actual: String,
    },
    #[error("release tag `{actual}` does not match expected `{expected}`")]
    ReleaseTagMismatch {
        expected: &'static str,
        actual: String,
    },
    #[error("Toccata transaction facade must use version 1, got {0}")]
    UnsupportedTransactionVersion(u16),
    #[error("Toccata transaction facade must carry storage_mass")]
    MissingStorageMass,
    #[error("Toccata transaction facade input is missing compute_commit")]
    MissingInputComputeCommit,
    #[error("Toccata transaction facade must include at least one covenant binding")]
    MissingCovenantBinding,
    #[error("Toccata transaction facade must include a user lane target")]
    MissingLaneTarget,
    #[error("transaction lane {transaction_lane} does not match proof lane {proof_lane}")]
    LaneMismatch {
        transaction_lane: u64,
        proof_lane: u64,
    },
    #[error("fixture must require a seq-commit lane proof")]
    MissingSeqCommitProofRequirement,
}

pub fn sample_toccata_compatibility_fixtures() -> ToccataCompatibilityFixtures {
    ToccataCompatibilityFixtures {
        schema_version: TOCCATA_COMPATIBILITY_FIXTURE_SCHEMA_VERSION.to_owned(),
        source_release: UpstreamReleasePin {
            repo: "https://github.com/kaspanet/rusty-kaspa".to_owned(),
            tag: RUSTY_KASPA_TOCCATA_COMPAT_TAG.to_owned(),
            commit: RUSTY_KASPA_TOCCATA_COMPAT_COMMIT.to_owned(),
            published_at: "2026-06-15T19:14:22Z".to_owned(),
            role: "current Toccata compatibility target for KaspaScript SDK facade fixtures"
                .to_owned(),
        },
        transaction_facade: sample_toccata_transaction_facade(),
        seq_commit_lane_proof: sample_seq_commit_lane_proof_fixture(),
        readiness: FixtureReadiness {
            status: "fixture-only".to_owned(),
            blockers: vec![
                "not wired to rusty-kaspa transaction builders".to_owned(),
                "not broadcastable".to_owned(),
                "mainnet activation has not been independently verified".to_owned(),
            ],
        },
    }
}

pub fn sample_toccata_transaction_facade() -> ToccataTransactionFacade {
    ToccataTransactionFacade {
        schema_version: TOCCATA_TRANSACTION_FACADE_SCHEMA_VERSION.to_owned(),
        rusty_kaspa_tag: RUSTY_KASPA_TOCCATA_COMPAT_TAG.to_owned(),
        transaction_version: 1,
        storage_mass: 640,
        builder_status: "fixture-only-not-a-rusty-kaspa-transaction".to_owned(),
        inputs: vec![ToccataInputFacade {
            previous_outpoint:
                "9f0d6e2a9b9f5c3d8e7a6b5c4d3e2f109876543210abcdef0011223344556677:0"
                    .to_owned(),
            sequence: 0,
            compute_commit:
                "3b1f0f0a8c6d4e2c1a09f8e7d6c5b4a39281716151413121100ffeeddccbbaa9"
                    .to_owned(),
            role: "state-input".to_owned(),
        }],
        outputs: vec![ToccataOutputFacade {
            value: 100_000_000,
            script_public_key_hex: "20d0f1c2b3a4958675647382910ffeeddccbbaa99887766554433221100aabbccac"
                .to_owned(),
            covenant: Some(CovenantBindingFacade {
                covenant_id:
                    "4f8b2c6d0a1e3f5799aabbccddeeff00112233445566778899aabbccddeeff00"
                        .to_owned(),
                payload_hash:
                    "68a1b2c3d4e5f60718293a4b5c6d7e8f90112233445566778899aabbccddeeff"
                        .to_owned(),
                state_index: 0,
                source_field: "TransactionOutput.covenant".to_owned(),
            }),
        }],
        lane_target: Some(UserLaneTarget {
            lane_id: 7,
            source: "v2.0.1 user-lane transaction generation fixture".to_owned(),
        }),
        proof_requirements: vec![
            ProofRequirement {
                kind: ProofRequirementKind::SeqCommitLaneProof,
                source: "get_seq_commit_lane_proof".to_owned(),
                reason: "sequencing-aware transitions need lane proof payloads before production lowering".to_owned(),
            },
            ProofRequirement {
                kind: ProofRequirementKind::CovenantBinding,
                source: "TransactionOutput.covenant".to_owned(),
                reason: "state continuation must preserve covenant lineage".to_owned(),
            },
        ],
    }
}

pub fn sample_seq_commit_lane_proof_fixture() -> SeqCommitLaneProofFixture {
    SeqCommitLaneProofFixture {
        rpc_surface: SeqCommitRpcSurface::GrpcAndWrpc,
        source: "rusty-kaspa v2.0.1 get_seq_commit_lane_proof RPC".to_owned(),
        request: SeqCommitLaneProofRequest {
            daa_score: 474_165_565,
            lane_id: 7,
            block_hash: "0000000f6a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5e6f708192a3b4c5"
                .to_owned(),
            accepted_id_merkle_root:
                "56e7d6c5b4a39281716151413121100ffeeddccbbaa99887766554433221100aa".to_owned(),
        },
        response: SeqCommitLaneProofResponse {
            lane_root: "a0b1c2d3e4f5061728394a5b6c7d8e9f00112233445566778899aabbccddeeff"
                .to_owned(),
            proof_hash: "bbccddee00112233445566778899aabbccddeeff00112233445566778899aa".to_owned(),
            item_count: 2,
            inactive_lane_shortcut: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn toccata_fixture_matches_published_golden() {
        let fixture = sample_toccata_compatibility_fixtures();
        fixture.validate().expect("valid fixture");

        let actual = serde_json::to_string_pretty(&fixture).expect("fixture json");
        assert_eq!(
            actual.trim_end(),
            include_str!("../../tests/fixtures/toccata/v2_0_1_compatibility.json").trim_end()
        );
    }

    #[test]
    fn toccata_fixture_file_roundtrips_through_sdk_facade() {
        let fixture: ToccataCompatibilityFixtures = serde_json::from_str(include_str!(
            "../../tests/fixtures/toccata/v2_0_1_compatibility.json"
        ))
        .expect("fixture parses");

        fixture.validate().expect("fixture validates");
        assert_eq!(fixture.source_release.tag, RUSTY_KASPA_TOCCATA_COMPAT_TAG);
        assert_eq!(fixture.transaction_facade.transaction_version, 1);
        assert_eq!(
            fixture.transaction_facade.outputs[0]
                .covenant
                .as_ref()
                .expect("covenant")
                .source_field,
            "TransactionOutput.covenant"
        );
    }

    #[test]
    fn facade_validation_rejects_missing_toccata_v1_fields() {
        let mut facade = sample_toccata_transaction_facade();
        facade.transaction_version = 0;
        assert!(matches!(
            facade.validate(),
            Err(ToccataFacadeError::UnsupportedTransactionVersion(0))
        ));

        let mut facade = sample_toccata_transaction_facade();
        facade.inputs[0].compute_commit.clear();
        assert!(matches!(
            facade.validate(),
            Err(ToccataFacadeError::MissingInputComputeCommit)
        ));

        let mut facade = sample_toccata_transaction_facade();
        facade.outputs[0].covenant = None;
        assert!(matches!(
            facade.validate(),
            Err(ToccataFacadeError::MissingCovenantBinding)
        ));
    }

    #[test]
    fn toccata_fixture_schema_file_is_valid_json() {
        let schema: Value = serde_json::from_str(include_str!(
            "../../docs/schemas/kaspascript.sdk.toccata.compatibility-fixtures.v0.schema.json"
        ))
        .expect("schema json");

        assert_eq!(
            schema["$schema"],
            Value::String("https://json-schema.org/draft/2020-12/schema".to_owned())
        );
        assert_eq!(
            schema["properties"]["schema_version"]["const"],
            Value::String(TOCCATA_COMPATIBILITY_FIXTURE_SCHEMA_VERSION.to_owned())
        );
    }
}
