use std::fmt;

use kaspascript_lexer::{Position, Span, TypeName};
use kaspascript_parser::{
    parse, BinaryOp, Contract, Expr, ParamValue, ParseError, Program, Require,
};
use kaspascript_protocol::{ProtocolFeature, ProtocolLimits};
use kaspascript_semantic::{analyze_program, Analysis, SemanticError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrProgram {
    pub contracts: Vec<IrContract>,
    pub required_features: Vec<ProtocolFeature>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrContract {
    pub name: String,
    pub params: Vec<IrParam>,
    pub spends: Vec<IrSpend>,
    pub finality_depth: Option<u32>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrParam {
    pub name: String,
    pub value: IrParamValue,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrParamValue {
    Type(TypeName),
    Integer(u64),
    String(String),
    Bool(bool),
    Identifier(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrSpend {
    pub name: String,
    pub params: Vec<IrTypedParam>,
    pub instructions: Vec<IrInstruction>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrTypedParam {
    pub name: String,
    pub ty: TypeName,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrInstruction {
    pub id: u32,
    pub kind: IrInstructionKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrInstructionKind {
    Require(IrExpr),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrExpr {
    Symbol {
        name: String,
        span: Span,
    },
    Integer {
        value: u64,
        span: Span,
    },
    String {
        value: String,
        span: Span,
    },
    Bool {
        value: bool,
        span: Span,
    },
    Array {
        elements: Vec<IrExpr>,
        span: Span,
    },
    Member {
        object: Box<IrExpr>,
        field: String,
        span: Span,
    },
    Call {
        callee: Box<IrExpr>,
        args: Vec<IrExpr>,
        span: Span,
    },
    Binary {
        left: Box<IrExpr>,
        op: IrBinaryOp,
        right: Box<IrExpr>,
        span: Span,
    },
}

impl IrExpr {
    pub fn span(&self) -> Span {
        match self {
            IrExpr::Symbol { span, .. }
            | IrExpr::Integer { span, .. }
            | IrExpr::String { span, .. }
            | IrExpr::Bool { span, .. }
            | IrExpr::Array { span, .. }
            | IrExpr::Member { span, .. }
            | IrExpr::Call { span, .. }
            | IrExpr::Binary { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrBinaryOp {
    Equal,
    NotEqual,
    GreaterEqual,
    LessEqual,
    Greater,
    Less,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrError {
    Parse(ParseError),
    Semantic(Vec<SemanticError>),
    Limit(IrLimitError),
    IntegerOverflow { literal: String, position: Position },
}

impl fmt::Display for IrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IrError::Parse(error) => write!(f, "{error}"),
            IrError::Semantic(errors) => {
                write!(f, "semantic analysis failed with {} error(s)", errors.len())
            }
            IrError::Limit(error) => write!(f, "{error}"),
            IrError::IntegerOverflow { literal, position } => write!(
                f,
                "integer literal `{literal}` exceeds u64 range at line {}, column {}, byte {}",
                position.line, position.column, position.offset
            ),
        }
    }
}

impl std::error::Error for IrError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrLimitError {
    pub message: String,
    pub position: Position,
}

impl IrLimitError {
    fn new(message: impl Into<String>, position: Position) -> Self {
        Self {
            message: message.into(),
            position,
        }
    }
}

impl fmt::Display for IrLimitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at line {}, column {}, byte {}",
            self.message, self.position.line, self.position.column, self.position.offset
        )
    }
}

impl std::error::Error for IrLimitError {}

pub fn lower(source: &str) -> Result<IrProgram, IrError> {
    lower_with_limits(source, &ProtocolLimits::default())
}

pub fn lower_with_limits(source: &str, limits: &ProtocolLimits) -> Result<IrProgram, IrError> {
    if source.len() > limits.max_source_bytes {
        return Err(IrError::Limit(IrLimitError::new(
            format!(
                "source is {} bytes, exceeding the configured {} byte limit",
                source.len(),
                limits.max_source_bytes
            ),
            Position::new(1, 1, 0),
        )));
    }

    let program = parse(source).map_err(IrError::Parse)?;
    let analysis = analyze_program(&program).map_err(IrError::Semantic)?;

    validate_limits(&program, limits)?;
    lower_program(&program, analysis)
}

pub fn lower_program(program: &Program, analysis: Analysis) -> Result<IrProgram, IrError> {
    let contracts = program
        .contracts
        .iter()
        .map(lower_contract)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(IrProgram {
        contracts,
        required_features: analysis.required_features,
    })
}

fn validate_limits(program: &Program, limits: &ProtocolLimits) -> Result<(), IrError> {
    if program.contracts.len() > limits.max_contracts {
        let position = program
            .contracts
            .get(limits.max_contracts)
            .map(|contract| contract.name.span.start)
            .unwrap_or_else(|| Position::new(1, 1, 0));
        return Err(IrError::Limit(IrLimitError::new(
            format!(
                "program declares {} contracts, exceeding the configured {} contract limit",
                program.contracts.len(),
                limits.max_contracts
            ),
            position,
        )));
    }

    for contract in &program.contracts {
        validate_contract_limits(contract, limits)?;
    }

    Ok(())
}

fn validate_contract_limits(contract: &Contract, limits: &ProtocolLimits) -> Result<(), IrError> {
    if contract.params.len() > limits.max_params_per_contract {
        return Err(IrError::Limit(IrLimitError::new(
            format!(
                "contract `{}` declares {} parameters, exceeding the configured {} parameter limit",
                contract.name.name,
                contract.params.len(),
                limits.max_params_per_contract
            ),
            contract.name.span.start,
        )));
    }

    if contract.spends.len() > limits.max_spends_per_contract {
        return Err(IrError::Limit(IrLimitError::new(
            format!(
                "contract `{}` declares {} spend paths, exceeding the configured {} spend limit",
                contract.name.name,
                contract.spends.len(),
                limits.max_spends_per_contract
            ),
            contract.name.span.start,
        )));
    }

    for spend in &contract.spends {
        if spend.params.len() > limits.max_spend_params {
            return Err(IrError::Limit(IrLimitError::new(
                format!(
                    "spend path `{}` declares {} parameters, exceeding the configured {} parameter limit",
                    spend.name.name,
                    spend.params.len(),
                    limits.max_spend_params
                ),
                spend.name.span.start,
            )));
        }

        if spend.requires.len() > limits.max_requires_per_spend {
            return Err(IrError::Limit(IrLimitError::new(
                format!(
                    "spend path `{}` declares {} require statements, exceeding the configured {} require limit",
                    spend.name.name,
                    spend.requires.len(),
                    limits.max_requires_per_spend
                ),
                spend.name.span.start,
            )));
        }

        for require in &spend.requires {
            let depth = expression_depth(&require.expr);
            if depth > limits.max_expression_depth {
                return Err(IrError::Limit(IrLimitError::new(
                    format!(
                        "require expression depth {depth} exceeds the configured {} depth limit",
                        limits.max_expression_depth
                    ),
                    require.span.start,
                )));
            }
        }
    }

    Ok(())
}

fn lower_contract(contract: &Contract) -> Result<IrContract, IrError> {
    let finality_depth = contract
        .params
        .iter()
        .find(|param| param.name.name == "finality_depth")
        .and_then(|param| match &param.value {
            ParamValue::Integer(value) => value.parse::<u32>().ok(),
            _ => None,
        });

    let params = contract
        .params
        .iter()
        .map(lower_param)
        .collect::<Result<Vec<_>, _>>()?;
    let spends = contract
        .spends
        .iter()
        .map(|spend| {
            let params = spend
                .params
                .iter()
                .map(|param| IrTypedParam {
                    name: param.name.name.clone(),
                    ty: param.ty,
                    span: param.span,
                })
                .collect();
            let instructions = spend
                .requires
                .iter()
                .enumerate()
                .map(|(index, require)| lower_require(index, require))
                .collect::<Result<Vec<_>, _>>()?;

            Ok(IrSpend {
                name: spend.name.name.clone(),
                params,
                instructions,
                span: spend.span,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(IrContract {
        name: contract.name.name.clone(),
        params,
        spends,
        finality_depth,
        span: contract.span,
    })
}

fn lower_param(param: &kaspascript_parser::Param) -> Result<IrParam, IrError> {
    let value = match &param.value {
        ParamValue::Type(ty) => IrParamValue::Type(*ty),
        ParamValue::Integer(value) => {
            IrParamValue::Integer(parse_u64_literal(value, param.name.span.start)?)
        }
        ParamValue::String(value) => IrParamValue::String(value.clone()),
        ParamValue::Bool(value) => IrParamValue::Bool(*value),
        ParamValue::Identifier(ident) => IrParamValue::Identifier(ident.name.clone()),
    };

    Ok(IrParam {
        name: param.name.name.clone(),
        value,
        span: param.span,
    })
}

fn lower_require(index: usize, require: &Require) -> Result<IrInstruction, IrError> {
    Ok(IrInstruction {
        id: u32::try_from(index).expect("require limit keeps instruction ids in u32 range"),
        kind: IrInstructionKind::Require(lower_expr(&require.expr)?),
        span: require.span,
    })
}

fn lower_expr(expr: &Expr) -> Result<IrExpr, IrError> {
    match expr {
        Expr::Identifier(ident) => Ok(IrExpr::Symbol {
            name: ident.name.clone(),
            span: ident.span,
        }),
        Expr::Integer { value, span } => Ok(IrExpr::Integer {
            value: parse_u64_literal(value, span.start)?,
            span: *span,
        }),
        Expr::String { value, span } => Ok(IrExpr::String {
            value: value.clone(),
            span: *span,
        }),
        Expr::Bool { value, span } => Ok(IrExpr::Bool {
            value: *value,
            span: *span,
        }),
        Expr::Array { elements, span } => Ok(IrExpr::Array {
            elements: elements
                .iter()
                .map(lower_expr)
                .collect::<Result<Vec<_>, _>>()?,
            span: *span,
        }),
        Expr::Member {
            object,
            field,
            span,
        } => Ok(IrExpr::Member {
            object: Box::new(lower_expr(object)?),
            field: field.name.clone(),
            span: *span,
        }),
        Expr::Call { callee, args, span } => Ok(IrExpr::Call {
            callee: Box::new(lower_expr(callee)?),
            args: args.iter().map(lower_expr).collect::<Result<Vec<_>, _>>()?,
            span: *span,
        }),
        Expr::Binary {
            left,
            op,
            right,
            span,
        } => Ok(IrExpr::Binary {
            left: Box::new(lower_expr(left)?),
            op: lower_binary_op(*op),
            right: Box::new(lower_expr(right)?),
            span: *span,
        }),
    }
}

fn lower_binary_op(op: BinaryOp) -> IrBinaryOp {
    match op {
        BinaryOp::Equal => IrBinaryOp::Equal,
        BinaryOp::NotEqual => IrBinaryOp::NotEqual,
        BinaryOp::GreaterEqual => IrBinaryOp::GreaterEqual,
        BinaryOp::LessEqual => IrBinaryOp::LessEqual,
        BinaryOp::Greater => IrBinaryOp::Greater,
        BinaryOp::Less => IrBinaryOp::Less,
    }
}

fn parse_u64_literal(literal: &str, position: Position) -> Result<u64, IrError> {
    literal
        .parse::<u64>()
        .map_err(|_| IrError::IntegerOverflow {
            literal: literal.to_owned(),
            position,
        })
}

fn expression_depth(expr: &Expr) -> usize {
    match expr {
        Expr::Identifier(_) | Expr::Integer { .. } | Expr::String { .. } | Expr::Bool { .. } => 1,
        Expr::Array { elements, .. } => {
            1 + elements.iter().map(expression_depth).max().unwrap_or(0)
        }
        Expr::Member { object, .. } => 1 + expression_depth(object),
        Expr::Call { callee, args, .. } => {
            let max_arg_depth = args.iter().map(expression_depth).max().unwrap_or(0);
            1 + expression_depth(callee).max(max_arg_depth)
        }
        Expr::Binary { left, right, .. } => 1 + expression_depth(left).max(expression_depth(right)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowers_production_vault_to_ir() {
        let source = include_str!("../../../contracts/production/DAGSafeVault.ks");
        let ir = lower(source).expect("production vault must lower");

        assert_eq!(ir.contracts.len(), 1);
        assert_eq!(ir.contracts[0].name, "DAGSafeVault");
        assert_eq!(ir.contracts[0].finality_depth, Some(10));
        assert_eq!(ir.contracts[0].spends.len(), 3);
        assert_eq!(ir.contracts[0].spends[0].instructions.len(), 6);
        assert!(ir.required_features.contains(&ProtocolFeature::CovenantIds));
        assert!(ir
            .required_features
            .contains(&ProtocolFeature::SequencingCommitments));
        assert!(ir
            .required_features
            .contains(&ProtocolFeature::TransactionIntrospection));
    }

    #[test]
    fn instruction_ids_are_deterministic_per_spend() {
        let source = r#"
            contract SimpleSig {
              params { owner: PublicKey }
              spend withdraw(sig: Signature) {
                require sig.verify(owner);
                require true;
              }
            }
        "#;

        let ir = lower(source).expect("simple contract must lower");
        let instructions = &ir.contracts[0].spends[0].instructions;

        assert_eq!(instructions[0].id, 0);
        assert_eq!(instructions[1].id, 1);
    }

    #[test]
    fn rejects_integer_overflow_during_lowering() {
        let source = r#"
            contract Overflow {
              params { owner: PublicKey }
              spend withdraw(sig: Signature) {
                require output(0).value == 340282366920938463463374607431768211455;
              }
            }
        "#;

        let error = lower(source).expect_err("overflowing integer must fail");
        assert!(matches!(error, IrError::IntegerOverflow { .. }));
    }

    #[test]
    fn enforces_protocol_limits_before_ir_emission() {
        let source = r#"
            contract TooManyRequires {
              params { owner: PublicKey }
              spend withdraw(sig: Signature) {
                require sig.verify(owner);
                require true;
              }
            }
        "#;
        let limits = ProtocolLimits {
            max_requires_per_spend: 1,
            ..ProtocolLimits::default()
        };

        let error = lower_with_limits(source, &limits).expect_err("limit must fail");
        assert!(matches!(error, IrError::Limit(_)));
    }
}
