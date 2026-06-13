use std::collections::BTreeSet;

use kaspascript_model::{
    ApplicationModel, BinaryOperator, Constraint, ConstraintKind, ContinuationKind,
    ContinuationModel, ContractModel, MonetaryPolicy, MonetaryResponsibility, NormalizedExpression,
    OutputBinding, OutputField, Parameter, SigningRequirement, SigningScheme, TransactionShape,
    TransitionModel, UnaryOperator,
};
use kaspascript_parser::{BinaryOp, Expr, Program, Spend, Stmt, UnaryOp};

pub(crate) fn build_application_model(program: &Program) -> ApplicationModel {
    ApplicationModel::new(
        program
            .contracts
            .iter()
            .map(|contract| ContractModel {
                name: contract.name.name.clone(),
                state: contract
                    .params
                    .iter()
                    .map(|param| Parameter {
                        name: param.name.name.clone(),
                        ty: param.ty,
                    })
                    .collect(),
                finality_depth: contract.finality_depth,
                transitions: contract.spends.iter().map(build_transition_model).collect(),
            })
            .collect(),
    )
}

fn build_transition_model(spend: &Spend) -> TransitionModel {
    let constraints = spend
        .body
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Require { expr, .. } => Some(Constraint {
                kind: classify_constraint(expr),
                expression: normalize_expression(expr),
                source_span: expr.span(),
            }),
            Stmt::Let { .. } | Stmt::Return { .. } => None,
        })
        .collect::<Vec<_>>();

    let mut referenced_inputs = BTreeSet::new();
    let mut referenced_outputs = BTreeSet::new();
    for constraint in &constraints {
        collect_transaction_indexes(
            &constraint.expression,
            &mut referenced_inputs,
            &mut referenced_outputs,
        );
    }

    let output_bindings = spend
        .body
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Require { expr, .. } => output_binding(expr),
            Stmt::Let { .. } | Stmt::Return { .. } => None,
        })
        .collect::<Vec<_>>();
    let continuation = continuation_model(&output_bindings);
    let value_constraint_count = constraints
        .iter()
        .filter(|constraint| constraint.kind == ConstraintKind::Value)
        .count();

    TransitionModel {
        name: spend.name.name.clone(),
        arguments: spend
            .params
            .iter()
            .map(|param| Parameter {
                name: param.name.name.clone(),
                ty: param.ty,
            })
            .collect(),
        signing_requirements: signing_requirements(spend),
        constraints,
        transaction_shape: TransactionShape {
            referenced_inputs: referenced_inputs.into_iter().collect(),
            referenced_outputs: referenced_outputs.into_iter().collect(),
            exact_input_count: None,
            exact_output_count: None,
            additional_inputs_permitted: true,
            additional_outputs_permitted: true,
        },
        monetary_policy: MonetaryPolicy {
            value_constraint_count,
            fee_handling: MonetaryResponsibility::ExternalExplicit,
            change_handling: MonetaryResponsibility::ExternalExplicit,
            compiler_injects_outputs: false,
            compiler_injects_recipients: false,
        },
        output_bindings,
        continuation,
    }
}

fn signing_requirements(spend: &Spend) -> Vec<SigningRequirement> {
    let mut requirements = Vec::new();
    for stmt in &spend.body {
        let Stmt::Require { expr, .. } = stmt else {
            continue;
        };
        collect_signing_requirements(expr, &mut requirements);
    }
    requirements
}

fn collect_signing_requirements(expr: &Expr, out: &mut Vec<SigningRequirement>) {
    match expr {
        Expr::Call { callee, args, .. } => {
            if let Expr::Field { object, field, .. } = &**callee {
                if field.name == "verify" {
                    out.push(SigningRequirement {
                        scheme: SigningScheme::SingleSignature,
                        threshold: 1,
                        authorized_keys: args.first().map(expression_label).into_iter().collect(),
                        signature_arguments: vec![expression_label(object)],
                    });
                }
            } else if let Expr::Ident(ident) = &**callee {
                if ident.name == "multisig" {
                    let threshold = match args.first() {
                        Some(Expr::Integer { value, .. }) => {
                            u32::try_from(*value).unwrap_or(u32::MAX)
                        }
                        _ => 0,
                    };
                    out.push(SigningRequirement {
                        scheme: SigningScheme::Multisig,
                        threshold,
                        authorized_keys: args.get(1).map(array_labels).unwrap_or_default(),
                        signature_arguments: args.get(2).map(array_labels).unwrap_or_default(),
                    });
                }
            }
            collect_signing_requirements(callee, out);
            for arg in args {
                collect_signing_requirements(arg, out);
            }
        }
        Expr::Array { elements, .. } => {
            for element in elements {
                collect_signing_requirements(element, out);
            }
        }
        Expr::Unary { expr, .. } => collect_signing_requirements(expr, out),
        Expr::Binary { left, right, .. } => {
            collect_signing_requirements(left, out);
            collect_signing_requirements(right, out);
        }
        Expr::Field { object, .. } => collect_signing_requirements(object, out),
        Expr::Ident(_) | Expr::Integer { .. } | Expr::String { .. } | Expr::Bool { .. } => {}
    }
}

fn array_labels(expr: &Expr) -> Vec<String> {
    match expr {
        Expr::Array { elements, .. } => elements.iter().map(expression_label).collect(),
        other => vec![expression_label(other)],
    }
}

fn expression_label(expr: &Expr) -> String {
    normalize_expression(expr).to_string()
}

fn classify_constraint(expr: &Expr) -> ConstraintKind {
    if expression_contains(expr, |name| name == "zk_verify") {
        ConstraintKind::Proof
    } else if expression_contains(expr, |name| name == "covenant_id" || name == "covenant") {
        ConstraintKind::Covenant
    } else if expression_contains(expr, |name| name == "sequencing") {
        ConstraintKind::Sequencing
    } else if expression_contains(expr, |name| name == "verify" || name == "multisig") {
        ConstraintKind::Authorization
    } else if expression_contains(expr, |name| name == "block") {
        ConstraintKind::Timelock
    } else if expression_contains(expr, |name| {
        matches!(name, "sha256" | "blake2b" | "hash160")
    }) {
        ConstraintKind::Hashlock
    } else if expression_contains_field(expr, "value") {
        ConstraintKind::Value
    } else if expression_contains_field(expr, "script") {
        ConstraintKind::Script
    } else if expression_contains(expr, |name| matches!(name, "input_count" | "output_count")) {
        ConstraintKind::TransactionShape
    } else {
        ConstraintKind::Generic
    }
}

fn expression_contains(expr: &Expr, predicate: impl Fn(&str) -> bool + Copy) -> bool {
    match expr {
        Expr::Ident(ident) => predicate(&ident.name),
        Expr::Integer { .. } | Expr::String { .. } | Expr::Bool { .. } => false,
        Expr::Array { elements, .. } => elements
            .iter()
            .any(|element| expression_contains(element, predicate)),
        Expr::Unary { expr, .. } => expression_contains(expr, predicate),
        Expr::Binary { left, right, .. } => {
            expression_contains(left, predicate) || expression_contains(right, predicate)
        }
        Expr::Call { callee, args, .. } => {
            expression_contains(callee, predicate)
                || args
                    .iter()
                    .any(|argument| expression_contains(argument, predicate))
        }
        Expr::Field { object, field, .. } => {
            predicate(&field.name) || expression_contains(object, predicate)
        }
    }
}

fn expression_contains_field(expr: &Expr, expected: &str) -> bool {
    match expr {
        Expr::Field { object, field, .. } => {
            field.name == expected || expression_contains_field(object, expected)
        }
        Expr::Array { elements, .. } => elements
            .iter()
            .any(|element| expression_contains_field(element, expected)),
        Expr::Unary { expr, .. } => expression_contains_field(expr, expected),
        Expr::Binary { left, right, .. } => {
            expression_contains_field(left, expected) || expression_contains_field(right, expected)
        }
        Expr::Call { callee, args, .. } => {
            expression_contains_field(callee, expected)
                || args
                    .iter()
                    .any(|argument| expression_contains_field(argument, expected))
        }
        Expr::Ident(_) | Expr::Integer { .. } | Expr::String { .. } | Expr::Bool { .. } => false,
    }
}

fn normalize_expression(expr: &Expr) -> NormalizedExpression {
    match expr {
        Expr::Ident(ident) => NormalizedExpression::Symbol {
            name: ident.name.clone(),
        },
        Expr::Integer { value, .. } => NormalizedExpression::Integer { value: *value },
        Expr::String { value, .. } => NormalizedExpression::String {
            value: value.clone(),
        },
        Expr::Bool { value, .. } => NormalizedExpression::Bool { value: *value },
        Expr::Array { elements, .. } => NormalizedExpression::Array {
            elements: elements.iter().map(normalize_expression).collect(),
        },
        Expr::Unary { op, expr, .. } => NormalizedExpression::Unary {
            op: match op {
                UnaryOp::Not => UnaryOperator::Not,
                UnaryOp::Negate => UnaryOperator::Negate,
            },
            operand: Box::new(normalize_expression(expr)),
        },
        Expr::Binary {
            left, op, right, ..
        } => NormalizedExpression::Binary {
            op: normalize_binary_operator(*op),
            left: Box::new(normalize_expression(left)),
            right: Box::new(normalize_expression(right)),
        },
        Expr::Call { callee, args, .. } => NormalizedExpression::Call {
            function: expression_path(callee),
            arguments: args.iter().map(normalize_expression).collect(),
        },
        Expr::Field { object, field, .. } => NormalizedExpression::Field {
            object: Box::new(normalize_expression(object)),
            field: field.name.clone(),
        },
    }
}

fn expression_path(expr: &Expr) -> String {
    match expr {
        Expr::Ident(ident) => ident.name.clone(),
        Expr::Field { object, field, .. } => format!("{}.{}", expression_path(object), field.name),
        _ => normalize_expression(expr).to_string(),
    }
}

fn normalize_binary_operator(op: BinaryOp) -> BinaryOperator {
    match op {
        BinaryOp::Or => BinaryOperator::Or,
        BinaryOp::And => BinaryOperator::And,
        BinaryOp::Equal => BinaryOperator::Equal,
        BinaryOp::NotEqual => BinaryOperator::NotEqual,
        BinaryOp::Greater => BinaryOperator::Greater,
        BinaryOp::GreaterEqual => BinaryOperator::GreaterEqual,
        BinaryOp::Less => BinaryOperator::Less,
        BinaryOp::LessEqual => BinaryOperator::LessEqual,
        BinaryOp::Add => BinaryOperator::Add,
        BinaryOp::Sub => BinaryOperator::Sub,
        BinaryOp::Mul => BinaryOperator::Mul,
        BinaryOp::Div => BinaryOperator::Div,
        BinaryOp::Mod => BinaryOperator::Mod,
    }
}

fn collect_transaction_indexes(
    expr: &NormalizedExpression,
    inputs: &mut BTreeSet<u32>,
    outputs: &mut BTreeSet<u32>,
) {
    match expr {
        NormalizedExpression::Call {
            function,
            arguments,
        } => {
            if let Some(NormalizedExpression::Integer { value }) = arguments.first() {
                if let Ok(index) = u32::try_from(*value) {
                    match function.as_str() {
                        "input" => {
                            inputs.insert(index);
                        }
                        "output" => {
                            outputs.insert(index);
                        }
                        _ => {}
                    }
                }
            }
            for argument in arguments {
                collect_transaction_indexes(argument, inputs, outputs);
            }
        }
        NormalizedExpression::Array { elements } => {
            for element in elements {
                collect_transaction_indexes(element, inputs, outputs);
            }
        }
        NormalizedExpression::Unary { operand, .. } => {
            collect_transaction_indexes(operand, inputs, outputs);
        }
        NormalizedExpression::Binary { left, right, .. } => {
            collect_transaction_indexes(left, inputs, outputs);
            collect_transaction_indexes(right, inputs, outputs);
        }
        NormalizedExpression::Field { object, .. } => {
            collect_transaction_indexes(object, inputs, outputs);
        }
        NormalizedExpression::Symbol { .. }
        | NormalizedExpression::Integer { .. }
        | NormalizedExpression::String { .. }
        | NormalizedExpression::Bool { .. } => {}
    }
}

fn output_binding(expr: &Expr) -> Option<OutputBinding> {
    let Expr::Binary {
        left, op, right, ..
    } = expr
    else {
        return None;
    };
    let relation = normalize_binary_operator(*op);

    if let Some((output_index, field)) = output_field(left) {
        return Some(OutputBinding {
            output_index,
            field,
            relation,
            expected: normalize_expression(right),
        });
    }
    if let Some((output_index, field)) = output_field(right) {
        return Some(OutputBinding {
            output_index,
            field,
            relation: reverse_relation(relation),
            expected: normalize_expression(left),
        });
    }
    None
}

fn output_field(expr: &Expr) -> Option<(u32, OutputField)> {
    let Expr::Field { object, field, .. } = expr else {
        return None;
    };
    let Expr::Call { callee, args, .. } = &**object else {
        return None;
    };
    let Expr::Ident(root) = &**callee else {
        return None;
    };
    if root.name != "output" {
        return None;
    }
    let Some(Expr::Integer { value, .. }) = args.first() else {
        return None;
    };
    let index = u32::try_from(*value).ok()?;
    let field = match field.name.as_str() {
        "value" => OutputField::Value,
        "script" => OutputField::Script,
        "covenant_id" => OutputField::CovenantId,
        _ => return None,
    };
    Some((index, field))
}

fn reverse_relation(relation: BinaryOperator) -> BinaryOperator {
    match relation {
        BinaryOperator::Greater => BinaryOperator::Less,
        BinaryOperator::GreaterEqual => BinaryOperator::LessEqual,
        BinaryOperator::Less => BinaryOperator::Greater,
        BinaryOperator::LessEqual => BinaryOperator::GreaterEqual,
        other => other,
    }
}

fn continuation_model(bindings: &[OutputBinding]) -> ContinuationModel {
    let mut covenant_outputs = bindings
        .iter()
        .filter(|binding| binding.field == OutputField::CovenantId)
        .map(|binding| binding.output_index)
        .collect::<BTreeSet<_>>();
    if !covenant_outputs.is_empty() {
        return ContinuationModel {
            kind: ContinuationKind::CovenantLineageBound,
            successor_outputs: covenant_outputs.iter().copied().collect(),
            note: "Source binds successor output covenant lineage.".to_owned(),
        };
    }

    covenant_outputs.extend(
        bindings
            .iter()
            .filter(|binding| binding.field == OutputField::Script)
            .map(|binding| binding.output_index),
    );
    if !covenant_outputs.is_empty() {
        return ContinuationModel {
            kind: ContinuationKind::OutputScriptBound,
            successor_outputs: covenant_outputs.iter().copied().collect(),
            note: "Source binds successor output ownership/script, but does not prove covenant lineage."
                .to_owned(),
        };
    }

    ContinuationModel {
        kind: ContinuationKind::Unspecified,
        successor_outputs: Vec::new(),
        note: "Source does not identify a successor state output; wallets and applications must not infer continuation."
            .to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use kaspascript_model::{ConstraintKind, ContinuationKind, SigningScheme};
    use kaspascript_parser::parse;

    use super::*;

    #[test]
    fn extracts_wallet_and_transaction_intent() {
        let program = parse(
            r#"
            contract Escrow {
              params { buyer: PublicKey, seller: PublicKey }
              spend release(sig_a: Signature, sig_b: Signature) {
                require multisig(2, [buyer, seller], [sig_a, sig_b]);
                require output(0).value >= input(0).value;
                require output(0).script == seller;
              }
            }
            "#,
        )
        .expect("parse");
        let model = build_application_model(&program);
        let transition = &model.contracts[0].transitions[0];

        assert_eq!(
            transition.signing_requirements[0].scheme,
            SigningScheme::Multisig
        );
        assert_eq!(transition.transaction_shape.referenced_inputs, vec![0]);
        assert_eq!(transition.transaction_shape.referenced_outputs, vec![0]);
        assert!(transition
            .constraints
            .iter()
            .any(|constraint| constraint.kind == ConstraintKind::Value));
        assert_eq!(
            transition.continuation.kind,
            ContinuationKind::OutputScriptBound
        );
        assert!(!transition.monetary_policy.compiler_injects_outputs);
    }
}
