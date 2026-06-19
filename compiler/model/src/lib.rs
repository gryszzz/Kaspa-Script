//! Canonical KaspaScript application model shared by compiler and tooling.
//!
//! This model describes what a checked KaspaScript program means around the
//! emitted script: state, transition constraints, signing intent, transaction
//! shape, monetary responsibilities, and the boundary between compiler proofs
//! and external verification.

use std::fmt;

use kaspascript_lexer::{Span, TypeName};
use serde::{Deserialize, Serialize};

/// Stable schema identifier for serialized application models.
pub const APPLICATION_MODEL_SCHEMA_VERSION: &str = "kaspascript.application.v0";

/// A complete, inspectable KaspaScript application model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationModel {
    pub schema_version: String,
    pub execution_model: ExecutionModel,
    pub contracts: Vec<ContractModel>,
    pub assurances: AssuranceProfile,
}

impl ApplicationModel {
    pub fn new(contracts: Vec<ContractModel>) -> Self {
        Self {
            schema_version: APPLICATION_MODEL_SCHEMA_VERSION.to_owned(),
            execution_model: ExecutionModel::KaspaUtxoStateMachine,
            contracts,
            assurances: AssuranceProfile::kaspascript_defaults(),
        }
    }

    pub fn empty() -> Self {
        Self::new(Vec::new())
    }

    pub fn transition(&self, contract: &str, transition: &str) -> Option<&TransitionModel> {
        self.contracts
            .iter()
            .find(|candidate| candidate.name == contract)
            .and_then(|contract| {
                contract
                    .transitions
                    .iter()
                    .find(|candidate| candidate.name == transition)
            })
    }
}

/// KaspaScript's execution model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionModel {
    KaspaUtxoStateMachine,
}

/// Contract state and transition model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractModel {
    pub name: String,
    pub state: Vec<Parameter>,
    pub finality_depth: Option<u64>,
    pub transitions: Vec<TransitionModel>,
}

/// Named typed value supplied to a contract or transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub ty: TypeName,
}

/// One spend path interpreted as a UTXO transition contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionModel {
    pub name: String,
    pub arguments: Vec<Parameter>,
    pub signing_requirements: Vec<SigningRequirement>,
    pub constraints: Vec<Constraint>,
    pub transaction_shape: TransactionShape,
    pub monetary_policy: MonetaryPolicy,
    pub output_bindings: Vec<OutputBinding>,
    pub continuation: ContinuationModel,
}

/// Signatures a wallet must collect before instantiating a transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SigningRequirement {
    pub scheme: SigningScheme,
    pub threshold: u32,
    pub authorized_keys: Vec<String>,
    pub signature_arguments: Vec<String>,
}

/// Signature policy recognized by source analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SigningScheme {
    SingleSignature,
    Multisig,
}

/// A source `require` preserved as a normalized transition constraint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Constraint {
    pub kind: ConstraintKind,
    pub expression: NormalizedExpression,
    pub source_span: Span,
}

/// Constraint category used by wallets, indexers, and inspection tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConstraintKind {
    Authorization,
    Value,
    Script,
    Timelock,
    Hashlock,
    Covenant,
    Sequencing,
    Proof,
    TransactionShape,
    Generic,
}

/// Normalized source expression that does not expose parser internals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum NormalizedExpression {
    Symbol {
        name: String,
    },
    Integer {
        value: u64,
    },
    String {
        value: String,
    },
    Bool {
        value: bool,
    },
    Array {
        elements: Vec<NormalizedExpression>,
    },
    Unary {
        op: UnaryOperator,
        operand: Box<NormalizedExpression>,
    },
    Binary {
        op: BinaryOperator,
        left: Box<NormalizedExpression>,
        right: Box<NormalizedExpression>,
    },
    Call {
        function: String,
        arguments: Vec<NormalizedExpression>,
    },
    Field {
        object: Box<NormalizedExpression>,
        field: String,
    },
}

impl fmt::Display for NormalizedExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Symbol { name } => f.write_str(name),
            Self::Integer { value } => write!(f, "{value}"),
            Self::String { value } => write!(f, "{value:?}"),
            Self::Bool { value } => write!(f, "{value}"),
            Self::Array { elements } => {
                f.write_str("[")?;
                for (index, element) in elements.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{element}")?;
                }
                f.write_str("]")
            }
            Self::Unary { op, operand } => write!(f, "{op}{operand}"),
            Self::Binary { op, left, right } => write!(f, "{left} {op} {right}"),
            Self::Call {
                function,
                arguments,
            } => {
                write!(f, "{function}(")?;
                for (index, argument) in arguments.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{argument}")?;
                }
                f.write_str(")")
            }
            Self::Field { object, field } => write!(f, "{object}.{field}"),
        }
    }
}

/// Unary operator in a normalized constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UnaryOperator {
    Not,
    Negate,
}

impl fmt::Display for UnaryOperator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Not => "!",
            Self::Negate => "-",
        })
    }
}

/// Binary operator in a normalized constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BinaryOperator {
    Or,
    And,
    Equal,
    NotEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

impl fmt::Display for BinaryOperator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Or => "||",
            Self::And => "&&",
            Self::Equal => "==",
            Self::NotEqual => "!=",
            Self::Greater => ">",
            Self::GreaterEqual => ">=",
            Self::Less => "<",
            Self::LessEqual => "<=",
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
            Self::Mod => "%",
        })
    }
}

/// Transaction indexes referenced by a transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionShape {
    pub referenced_inputs: Vec<u32>,
    pub referenced_outputs: Vec<u32>,
    pub exact_input_count: Option<u32>,
    pub exact_output_count: Option<u32>,
    pub additional_inputs_permitted: bool,
    pub additional_outputs_permitted: bool,
}

/// Explicit statement of who owns fee and change decisions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonetaryPolicy {
    pub value_constraint_count: usize,
    pub fee_handling: MonetaryResponsibility,
    pub change_handling: MonetaryResponsibility,
    pub compiler_injects_outputs: bool,
    pub compiler_injects_recipients: bool,
}

/// Responsibility for transaction-level monetary construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MonetaryResponsibility {
    ExternalExplicit,
}

/// An output field constrained by source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputBinding {
    pub output_index: u32,
    pub field: OutputField,
    pub relation: BinaryOperator,
    pub expected: NormalizedExpression,
}

/// Output fields that can be bound by a transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutputField {
    Value,
    Script,
    CovenantId,
}

impl fmt::Display for OutputField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Value => "value",
            Self::Script => "script",
            Self::CovenantId => "covenant_id",
        })
    }
}

/// How source constrains successor state or output ownership.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContinuationModel {
    pub kind: ContinuationKind,
    pub successor_outputs: Vec<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub named_successor_outputs: Vec<NamedContinuationOutput>,
    pub note: String,
}

/// Continuation strength inferred from source constraints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContinuationKind {
    Unspecified,
    NamedOutput,
    OutputScriptBound,
    CovenantLineageBound,
}

/// A source-declared successor output name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedContinuationOutput {
    pub name: String,
    pub output_index: u32,
}

/// What compilation proves and what remains external.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssuranceProfile {
    pub compiler_guarantees: Vec<Assurance>,
    pub external_obligations: Vec<Assurance>,
}

impl AssuranceProfile {
    pub fn kaspascript_defaults() -> Self {
        Self {
            compiler_guarantees: vec![
                Assurance::new(
                    "checked-source",
                    AssuranceActor::Compiler,
                    "The source parsed and passed KaspaScript semantic checks.",
                ),
                Assurance::new(
                    "deterministic-lowering",
                    AssuranceActor::Compiler,
                    "The checked program lowers deterministically to typed IR and artifact metadata.",
                ),
                Assurance::new(
                    "explicit-monetary-effects",
                    AssuranceActor::Compiler,
                    "Compilation does not create transaction outputs, recipients, fees, or change.",
                ),
                Assurance::new(
                    "inspectable-signing-intent",
                    AssuranceActor::Compiler,
                    "Recognized signature requirements and transaction constraints are preserved in the application model.",
                ),
            ],
            external_obligations: vec![
                Assurance::new(
                    "transaction-instantiation",
                    AssuranceActor::Application,
                    "Construct concrete inputs, outputs, arguments, and continuation records that satisfy every transition constraint.",
                ),
                Assurance::new(
                    "fees-and-change",
                    AssuranceActor::Wallet,
                    "Choose and display fees, change, and every recipient explicitly before signing.",
                ),
                Assurance::new(
                    "signature-collection",
                    AssuranceActor::Wallet,
                    "Resolve authorized keys and collect the signatures required by the selected transition.",
                ),
                Assurance::new(
                    "network-validation",
                    AssuranceActor::Node,
                    "Validate consensus, standardness, mass, fee policy, and target-network rules.",
                ),
                Assurance::new(
                    "lineage-and-reorgs",
                    AssuranceActor::Indexer,
                    "Track accepted UTXOs, covenant lineage, duplicate transitions, and reorg effects.",
                ),
                Assurance::new(
                    "activation-evidence",
                    AssuranceActor::Operator,
                    "Verify release, network, KIP, and activation assumptions against current primary sources.",
                ),
            ],
        }
    }
}

/// One guarantee or external integration duty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Assurance {
    pub id: String,
    pub actor: AssuranceActor,
    pub statement: String,
}

impl Assurance {
    pub fn new(id: impl Into<String>, actor: AssuranceActor, statement: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            actor,
            statement: statement.into(),
        }
    }
}

/// Component responsible for an assurance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AssuranceActor {
    Compiler,
    Application,
    Wallet,
    Indexer,
    Node,
    Operator,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn application_schema_version_is_stable() {
        let model = ApplicationModel::empty();
        let json = serde_json::to_value(model).expect("serialize");

        assert_eq!(
            json["schema_version"],
            serde_json::Value::String(APPLICATION_MODEL_SCHEMA_VERSION.to_owned())
        );
        assert_eq!(
            json["execution_model"],
            serde_json::Value::String("kaspa-utxo-state-machine".to_owned())
        );
    }

    #[test]
    fn published_application_schema_is_valid_json() {
        let schema = include_str!("../../../docs/schemas/kaspascript.application.v0.schema.json");
        let json: serde_json::Value = serde_json::from_str(schema).expect("schema json");

        assert_eq!(
            json["properties"]["schema_version"]["const"],
            serde_json::Value::String(APPLICATION_MODEL_SCHEMA_VERSION.to_owned())
        );
        assert_eq!(
            json["$schema"],
            serde_json::Value::String("https://json-schema.org/draft/2020-12/schema".to_owned())
        );
    }
}
