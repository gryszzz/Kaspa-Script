use std::collections::HashSet;
use std::fmt;

use kaspascript_lexer::Position;
use kaspascript_parser::{parse, Contract, Expr, Ident, ParamValue, ParseError, Program};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Analysis {
    pub contract_count: usize,
    pub spend_count: usize,
    pub require_count: usize,
    pub finality_depths: Vec<FinalityDepth>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalityDepth {
    pub contract: String,
    pub value: u32,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalysisError {
    Parse(ParseError),
    Semantic(Vec<SemanticError>),
}

impl fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnalysisError::Parse(error) => write!(f, "{error}"),
            AnalysisError::Semantic(errors) => {
                write!(f, "semantic analysis failed with {} error(s)", errors.len())
            }
        }
    }
}

impl std::error::Error for AnalysisError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticError {
    pub message: String,
    pub position: Position,
}

impl SemanticError {
    fn new(message: impl Into<String>, position: Position) -> Self {
        Self {
            message: message.into(),
            position,
        }
    }
}

impl fmt::Display for SemanticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at line {}, column {}, byte {}",
            self.message, self.position.line, self.position.column, self.position.offset
        )
    }
}

impl std::error::Error for SemanticError {}

pub fn analyze(source: &str) -> Result<Analysis, AnalysisError> {
    let program = parse(source).map_err(AnalysisError::Parse)?;
    analyze_program(&program).map_err(AnalysisError::Semantic)
}

pub fn analyze_program(program: &Program) -> Result<Analysis, Vec<SemanticError>> {
    let mut errors = Vec::new();
    let mut finality_depths = Vec::new();

    if program.contracts.is_empty() {
        errors.push(SemanticError::new(
            "source must contain at least one contract",
            Position::new(1, 1, 0),
        ));
    }

    let mut contract_names = HashSet::new();
    for contract in &program.contracts {
        if !contract_names.insert(contract.name.name.as_str()) {
            errors.push(SemanticError::new(
                format!("duplicate contract `{}`", contract.name.name),
                contract.name.span.start,
            ));
        }

        validate_contract(contract, &mut finality_depths, &mut errors);
    }

    if errors.is_empty() {
        Ok(Analysis {
            contract_count: program.contracts.len(),
            spend_count: program
                .contracts
                .iter()
                .map(|contract| contract.spends.len())
                .sum(),
            require_count: program
                .contracts
                .iter()
                .flat_map(|contract| &contract.spends)
                .map(|spend| spend.requires.len())
                .sum(),
            finality_depths,
        })
    } else {
        Err(errors)
    }
}

fn validate_contract(
    contract: &Contract,
    finality_depths: &mut Vec<FinalityDepth>,
    errors: &mut Vec<SemanticError>,
) {
    if !contract.has_params_block {
        errors.push(SemanticError::new(
            format!(
                "contract `{}` must declare a params block",
                contract.name.name
            ),
            contract.name.span.start,
        ));
    }

    if contract.spends.is_empty() {
        errors.push(SemanticError::new(
            format!(
                "contract `{}` must declare at least one spend path",
                contract.name.name
            ),
            contract.name.span.start,
        ));
    }

    let mut param_names = HashSet::new();
    let mut contract_scope = HashSet::new();

    for param in &contract.params {
        if !param_names.insert(param.name.name.as_str()) {
            errors.push(SemanticError::new(
                format!("duplicate parameter `{}`", param.name.name),
                param.name.span.start,
            ));
        }

        contract_scope.insert(param.name.name.clone());

        if param.name.name == "finality_depth" {
            match &param.value {
                ParamValue::Integer(value) => match value.parse::<u32>() {
                    Ok(depth) if depth > 0 => finality_depths.push(FinalityDepth {
                        contract: contract.name.name.clone(),
                        value: depth,
                        position: param.name.span.start,
                    }),
                    Ok(_) => errors.push(SemanticError::new(
                        "`finality_depth` must be greater than zero",
                        param.name.span.start,
                    )),
                    Err(_) => errors.push(SemanticError::new(
                        "`finality_depth` exceeds the supported u32 range",
                        param.name.span.start,
                    )),
                },
                _ => errors.push(SemanticError::new(
                    "`finality_depth` must be an integer literal",
                    param.name.span.start,
                )),
            }
        } else if !matches!(param.value, ParamValue::Type(_)) {
            errors.push(SemanticError::new(
                format!(
                    "parameter `{}` must declare a KaspaScript type",
                    param.name.name
                ),
                param.name.span.start,
            ));
        }
    }

    let mut spend_names = HashSet::new();
    for spend in &contract.spends {
        if !spend_names.insert(spend.name.name.as_str()) {
            errors.push(SemanticError::new(
                format!("duplicate spend path `{}`", spend.name.name),
                spend.name.span.start,
            ));
        }

        if spend.requires.is_empty() {
            errors.push(SemanticError::new(
                format!(
                    "spend path `{}` must contain at least one require",
                    spend.name.name
                ),
                spend.name.span.start,
            ));
        }

        let mut spend_scope = contract_scope.clone();
        let mut spend_param_names = HashSet::new();

        for param in &spend.params {
            if contract_scope.contains(&param.name.name) {
                errors.push(SemanticError::new(
                    format!(
                        "spend parameter `{}` shadows a contract parameter",
                        param.name.name
                    ),
                    param.name.span.start,
                ));
            }

            if !spend_param_names.insert(param.name.name.as_str()) {
                errors.push(SemanticError::new(
                    format!("duplicate spend parameter `{}`", param.name.name),
                    param.name.span.start,
                ));
            }

            spend_scope.insert(param.name.name.clone());
        }

        for require in &spend.requires {
            let mut roots = Vec::new();
            collect_root_identifiers(&require.expr, &mut roots);
            for root in roots {
                if !spend_scope.contains(&root.name) && !is_builtin_root(&root.name) {
                    errors.push(SemanticError::new(
                        format!("unknown identifier `{}`", root.name),
                        root.span.start,
                    ));
                }
            }
        }
    }
}

fn collect_root_identifiers<'expr>(expr: &'expr Expr, roots: &mut Vec<&'expr Ident>) {
    match expr {
        Expr::Identifier(ident) => roots.push(ident),
        Expr::Integer { .. } | Expr::String { .. } | Expr::Bool { .. } => {}
        Expr::Array { elements, .. } => {
            for element in elements {
                collect_root_identifiers(element, roots);
            }
        }
        Expr::Member { object, .. } => collect_root_identifiers(object, roots),
        Expr::Call { callee, args, .. } => {
            collect_root_identifiers(callee, roots);
            for arg in args {
                collect_root_identifiers(arg, roots);
            }
        }
        Expr::Binary { left, right, .. } => {
            collect_root_identifiers(left, roots);
            collect_root_identifiers(right, roots);
        }
    }
}

fn is_builtin_root(name: &str) -> bool {
    matches!(
        name,
        "block"
            | "covenant"
            | "covenant_id"
            | "input"
            | "multisig"
            | "output"
            | "sequencing"
            | "zk_verify"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyzes_production_vault_contract() {
        let source = include_str!("../../../contracts/production/DAGSafeVault.ks");
        let analysis = analyze(source).expect("production vault must pass semantic analysis");

        assert_eq!(analysis.contract_count, 1);
        assert_eq!(analysis.spend_count, 3);
        assert_eq!(analysis.require_count, 17);
        assert_eq!(analysis.finality_depths.len(), 1);
        assert_eq!(analysis.finality_depths[0].value, 10);
    }

    #[test]
    fn rejects_duplicate_contract_parameters() {
        let source = r#"
            contract Broken {
              params {
                owner: PublicKey,
                owner: PublicKey,
              }

              spend withdraw(sig: Signature) {
                require sig.verify(owner);
              }
            }
        "#;

        let errors = analyze(source).expect_err("duplicate parameter must fail");
        let AnalysisError::Semantic(errors) = errors else {
            panic!("expected semantic errors");
        };
        assert!(errors
            .iter()
            .any(|error| error.message == "duplicate parameter `owner`"));
    }

    #[test]
    fn rejects_invalid_finality_depth() {
        let source = r#"
            contract Broken {
              params {
                owner: PublicKey,
                finality_depth: 0,
              }

              spend withdraw(sig: Signature) {
                require sig.verify(owner);
              }
            }
        "#;

        let errors = analyze(source).expect_err("zero finality depth must fail");
        let AnalysisError::Semantic(errors) = errors else {
            panic!("expected semantic errors");
        };
        assert!(errors
            .iter()
            .any(|error| error.message == "`finality_depth` must be greater than zero"));
    }

    #[test]
    fn rejects_unknown_identifiers_in_requires() {
        let source = r#"
            contract Broken {
              params {
                owner: PublicKey,
              }

              spend withdraw(sig: Signature) {
                require sig.verify(attacker);
              }
            }
        "#;

        let errors = analyze(source).expect_err("unknown identifier must fail");
        let AnalysisError::Semantic(errors) = errors else {
            panic!("expected semantic errors");
        };
        assert!(errors
            .iter()
            .any(|error| error.message == "unknown identifier `attacker`"));
    }

    #[test]
    fn rejects_spend_without_require() {
        let source = r#"
            contract Broken {
              params {
                owner: PublicKey,
              }

              spend withdraw(sig: Signature) {
              }
            }
        "#;

        let errors = analyze(source).expect_err("empty spend must fail");
        let AnalysisError::Semantic(errors) = errors else {
            panic!("expected semantic errors");
        };
        assert!(errors.iter().any(
            |error| error.message == "spend path `withdraw` must contain at least one require"
        ));
    }
}
