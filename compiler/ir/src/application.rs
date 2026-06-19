use std::collections::BTreeSet;

use kaspascript_model::{
    ApplicationModel, BinaryOperator, Constraint, ConstraintKind, ContinuationKind,
    ContinuationModel, ContractModel, MonetaryPolicy, MonetaryResponsibility,
    NamedContinuationOutput, NormalizedExpression, OutputBinding, OutputField, Parameter,
    SigningRequirement, SigningScheme, TransactionShape, TransitionModel, UnaryOperator,
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
    let named_successor_outputs = named_successor_outputs(spend);
    let continuation = continuation_model(&output_bindings, &named_successor_outputs);
    let value_constraint_count = constraints
        .iter()
        .filter(|constraint| constraint.kind == ConstraintKind::Value)
        .count();
    let (exact_input_count, exact_output_count) = exact_transaction_counts(spend);

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
            exact_input_count,
            exact_output_count,
            additional_inputs_permitted: exact_input_count.is_none(),
            additional_outputs_permitted: exact_output_count.is_none(),
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
    } else if expression_contains(expr, |name| {
        matches!(name, "input_count" | "output_count" | "continuation")
    }) {
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

fn exact_transaction_counts(spend: &Spend) -> (Option<u32>, Option<u32>) {
    let mut input_count = None;
    let mut output_count = None;

    for stmt in &spend.body {
        let Stmt::Require { expr, .. } = stmt else {
            continue;
        };
        collect_exact_transaction_counts(expr, &mut input_count, &mut output_count);
    }

    (input_count, output_count)
}

fn collect_exact_transaction_counts(
    expr: &Expr,
    input_count: &mut Option<u32>,
    output_count: &mut Option<u32>,
) {
    if let Expr::Binary {
        left,
        op: BinaryOp::And,
        right,
        ..
    } = expr
    {
        collect_exact_transaction_counts(left, input_count, output_count);
        collect_exact_transaction_counts(right, input_count, output_count);
        return;
    }

    let Some((subject, count)) = exact_count_constraint(expr) else {
        return;
    };
    match subject {
        CountSubject::Input => {
            *input_count = Some(count);
        }
        CountSubject::Output => {
            *output_count = Some(count);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CountSubject {
    Input,
    Output,
}

fn exact_count_constraint(expr: &Expr) -> Option<(CountSubject, u32)> {
    let Expr::Binary {
        left,
        op: BinaryOp::Equal,
        right,
        ..
    } = expr
    else {
        return None;
    };

    if let (Some(subject), Expr::Integer { value, .. }) = (count_subject(left), &**right) {
        return Some((subject, u32::try_from(*value).ok()?));
    }
    if let (Expr::Integer { value, .. }, Some(subject)) = (&**left, count_subject(right)) {
        return Some((subject, u32::try_from(*value).ok()?));
    }
    None
}

fn count_subject(expr: &Expr) -> Option<CountSubject> {
    let Expr::Ident(ident) = expr else {
        return None;
    };
    match ident.name.as_str() {
        "input_count" => Some(CountSubject::Input),
        "output_count" => Some(CountSubject::Output),
        _ => None,
    }
}

fn named_successor_outputs(spend: &Spend) -> Vec<NamedContinuationOutput> {
    let mut outputs = Vec::new();
    for stmt in &spend.body {
        let Stmt::Require { expr, .. } = stmt else {
            continue;
        };
        collect_named_successor_outputs(expr, &mut outputs);
    }
    outputs
}

fn collect_named_successor_outputs(expr: &Expr, out: &mut Vec<NamedContinuationOutput>) {
    if let Some(output) = named_successor_output(expr) {
        out.push(output);
    }

    match expr {
        Expr::Array { elements, .. } => {
            for element in elements {
                collect_named_successor_outputs(element, out);
            }
        }
        Expr::Unary { expr, .. } => collect_named_successor_outputs(expr, out),
        Expr::Binary { left, right, .. } => {
            collect_named_successor_outputs(left, out);
            collect_named_successor_outputs(right, out);
        }
        Expr::Call { callee, args, .. } => {
            collect_named_successor_outputs(callee, out);
            for arg in args {
                collect_named_successor_outputs(arg, out);
            }
        }
        Expr::Field { object, .. } => collect_named_successor_outputs(object, out),
        Expr::Ident(_) | Expr::Integer { .. } | Expr::String { .. } | Expr::Bool { .. } => {}
    }
}

fn named_successor_output(expr: &Expr) -> Option<NamedContinuationOutput> {
    let Expr::Call { callee, args, .. } = expr else {
        return None;
    };
    let Expr::Ident(ident) = &**callee else {
        return None;
    };
    if ident.name != "continuation" {
        return None;
    }
    let Some(Expr::String { value: name, .. }) = args.first() else {
        return None;
    };
    let output_index = args.get(1).and_then(output_call_index)?;
    Some(NamedContinuationOutput {
        name: name.clone(),
        output_index,
    })
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
    if args.len() != 1 {
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

fn output_call_index(expr: &Expr) -> Option<u32> {
    let Expr::Call { callee, args, .. } = expr else {
        return None;
    };
    let Expr::Ident(root) = &**callee else {
        return None;
    };
    if root.name != "output" {
        return None;
    }
    if args.len() != 1 {
        return None;
    }
    let Some(Expr::Integer { value, .. }) = args.first() else {
        return None;
    };
    u32::try_from(*value).ok()
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

fn continuation_model(
    bindings: &[OutputBinding],
    named_successor_outputs: &[NamedContinuationOutput],
) -> ContinuationModel {
    let mut covenant_outputs = bindings
        .iter()
        .filter(|binding| binding.field == OutputField::CovenantId)
        .map(|binding| binding.output_index)
        .collect::<BTreeSet<_>>();
    let named_indexes = named_successor_outputs
        .iter()
        .map(|output| output.output_index)
        .collect::<BTreeSet<_>>();
    if !covenant_outputs.is_empty() {
        covenant_outputs.extend(named_indexes);
        return ContinuationModel {
            kind: ContinuationKind::CovenantLineageBound,
            successor_outputs: covenant_outputs.iter().copied().collect(),
            named_successor_outputs: named_successor_outputs.to_vec(),
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
        covenant_outputs.extend(named_indexes);
        return ContinuationModel {
            kind: ContinuationKind::OutputScriptBound,
            successor_outputs: covenant_outputs.iter().copied().collect(),
            named_successor_outputs: named_successor_outputs.to_vec(),
            note: "Source binds successor output ownership/script, but does not prove covenant lineage."
                .to_owned(),
        };
    }

    if !named_successor_outputs.is_empty() {
        return ContinuationModel {
            kind: ContinuationKind::NamedOutput,
            successor_outputs: named_successor_outputs
                .iter()
                .map(|output| output.output_index)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            named_successor_outputs: named_successor_outputs.to_vec(),
            note: "Source names successor output(s), but does not bind ownership/script or covenant lineage."
                .to_owned(),
        };
    }

    ContinuationModel {
        kind: ContinuationKind::Unspecified,
        successor_outputs: Vec::new(),
        named_successor_outputs: Vec::new(),
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

    #[test]
    fn extracts_exact_counts_and_named_continuation_outputs() {
        let program = parse(
            r#"
            contract Channel {
              params { owner: PublicKey }
              spend advance(sig: Signature) {
                require sig.verify(owner);
                require input_count == 1 && output_count == 2;
                require continuation("state", output(1));
              }
            }
            "#,
        )
        .expect("parse");
        let model = build_application_model(&program);
        let transition = &model.contracts[0].transitions[0];

        assert_eq!(transition.transaction_shape.exact_input_count, Some(1));
        assert_eq!(transition.transaction_shape.exact_output_count, Some(2));
        assert!(!transition.transaction_shape.additional_inputs_permitted);
        assert!(!transition.transaction_shape.additional_outputs_permitted);
        assert_eq!(transition.transaction_shape.referenced_outputs, vec![1]);
        assert_eq!(transition.continuation.kind, ContinuationKind::NamedOutput);
        assert_eq!(transition.continuation.successor_outputs, vec![1]);
        assert_eq!(
            transition.continuation.named_successor_outputs[0].name,
            "state"
        );
        assert!(transition
            .constraints
            .iter()
            .any(|constraint| constraint.kind == ConstraintKind::TransactionShape));
    }
}
