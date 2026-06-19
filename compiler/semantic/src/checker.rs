use std::collections::HashSet;
use std::fmt;

use kaspascript_lexer::{locate, SourceLocation, Span, TypeName};
use kaspascript_parser::{
    parse_file, BinaryOp, Contract, Expr, Ident, ParseError, Program, Spend, Stmt, UnaryOp,
};
use thiserror::Error;

use crate::scope::Scope;

/// A semantic error with source location.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub struct SemanticError {
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub span: Span,
    pub message: String,
}

impl fmt::Display for SemanticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}: {}",
            self.file, self.line, self.column, self.message
        )
    }
}

/// Full semantic analysis output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Analysis {
    pub program: Program,
    pub kip_requirements: Vec<u16>,
    pub finality_depth: Option<u64>,
}

/// Semantic analysis failure.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AnalyzeError {
    #[error("{0}")]
    Parse(ParseError),
    #[error("semantic analysis failed with {0} error(s)")]
    Semantic(usize),
}

/// Result that preserves all collected semantic errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyzeFailure {
    pub error: AnalyzeError,
    pub errors: Vec<SemanticError>,
}

impl fmt::Display for AnalyzeFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.errors.is_empty() {
            write!(f, "{}", self.error)
        } else {
            for (index, error) in self.errors.iter().enumerate() {
                if index > 0 {
                    writeln!(f)?;
                }
                write!(f, "{error}")?;
            }
            Ok(())
        }
    }
}

impl std::error::Error for AnalyzeFailure {}

/// Parses and semantically checks a file.
pub fn analyze_file(source: &str, file: &str) -> Result<Analysis, AnalyzeFailure> {
    let program = parse_file(source, file).map_err(|error| AnalyzeFailure {
        error: AnalyzeError::Parse(error),
        errors: Vec::new(),
    })?;
    analyze_program(program, source, file)
}

/// Parses and semantically checks source.
pub fn analyze(source: &str) -> Result<Analysis, AnalyzeFailure> {
    analyze_file(source, "<source>")
}

/// Semantically checks an already parsed program.
pub fn analyze_program(
    program: Program,
    source: &str,
    file: &str,
) -> Result<Analysis, AnalyzeFailure> {
    let mut checker = Checker::new(source, file);
    checker.check_program(&program);
    if checker.errors.is_empty() {
        let kip_requirements = collect_kip_requirements(&program);
        let finality_depth = program
            .contracts
            .iter()
            .filter_map(|contract| contract.finality_depth)
            .max();
        Ok(Analysis {
            program,
            kip_requirements,
            finality_depth,
        })
    } else {
        Err(AnalyzeFailure {
            error: AnalyzeError::Semantic(checker.errors.len()),
            errors: checker.errors,
        })
    }
}

struct Checker<'source> {
    source: &'source str,
    file: &'source str,
    errors: Vec<SemanticError>,
    spend_names: HashSet<String>,
}

impl<'source> Checker<'source> {
    fn new(source: &'source str, file: &'source str) -> Self {
        Self {
            source,
            file,
            errors: Vec::new(),
            spend_names: HashSet::new(),
        }
    }

    fn check_program(&mut self, program: &Program) {
        let mut contract_names = HashSet::new();
        for contract in &program.contracts {
            if !contract_names.insert(contract.name.name.clone()) {
                self.error(
                    contract.name.span,
                    format!("duplicate contract `{}`", contract.name.name),
                );
            }
            self.check_contract(contract);
        }
    }

    fn check_contract(&mut self, contract: &Contract) {
        if matches!(contract.finality_depth, Some(0)) {
            self.error(contract.name.span, "`finality_depth` must be > 0");
        }

        let mut contract_scope = Scope::new();
        for param in &contract.params {
            if !contract_scope.insert(param.name.name.clone(), param.ty) {
                self.error(
                    param.name.span,
                    format!("duplicate parameter `{}`", param.name.name),
                );
            }
        }

        self.spend_names = contract
            .spends
            .iter()
            .map(|spend| spend.name.name.clone())
            .collect();

        let mut seen_spends = HashSet::new();
        for spend in &contract.spends {
            if !seen_spends.insert(spend.name.name.clone()) {
                self.error(
                    spend.name.span,
                    format!("duplicate spend `{}`", spend.name.name),
                );
            }
            self.check_spend(contract, spend, &contract_scope);
        }
    }

    fn check_spend(&mut self, contract: &Contract, spend: &Spend, contract_scope: &Scope) {
        let mut scope = contract_scope.clone();
        let mut local_names = HashSet::new();
        let mut shape_facts = ShapeFacts::default();
        let mut continuation_names = HashSet::new();

        for param in &spend.params {
            if contract_scope.contains(&param.name.name) {
                self.error(
                    param.name.span,
                    format!(
                        "spend parameter `{}` shadows a contract parameter",
                        param.name.name
                    ),
                );
            }
            if !local_names.insert(param.name.name.clone()) {
                self.error(
                    param.name.span,
                    format!("duplicate spend parameter `{}`", param.name.name),
                );
            }
            scope.insert(param.name.name.clone(), param.ty);
        }

        let mut has_require = false;
        for stmt in &spend.body {
            match stmt {
                Stmt::Let { name, expr, .. } => {
                    let ty = self.infer_expr(expr, &scope);
                    if !local_names.insert(name.name.clone()) || scope.contains(&name.name) {
                        self.error(name.span, format!("duplicate let binding `{}`", name.name));
                    } else if let Some(ty) = ty {
                        scope.insert(name.name.clone(), ty);
                    }
                }
                Stmt::Require { expr, .. } => {
                    has_require = true;
                    let ty = self.infer_expr(expr, &scope);
                    if !matches!(ty, Some(TypeName::Bool)) {
                        self.error(expr.span(), "`require` expression must be Bool");
                    }
                    self.collect_shape_facts(expr, &mut shape_facts);
                    self.collect_continuation_names(expr, &mut continuation_names);
                }
                Stmt::Return { expr, .. } => {
                    self.infer_expr(expr, &scope);
                }
            }
        }

        if !has_require {
            self.error(
                spend.name.span,
                format!(
                    "spend `{}` must contain at least one require",
                    spend.name.name
                ),
            );
        }

        if spend_uses_name(spend, "covenant_id") && contract.finality_depth.unwrap_or(0) == 0 {
            self.error(
                spend.name.span,
                "`covenant_id` requires `finality_depth` > 0",
            );
        }

        self.check_shape_facts(&shape_facts);
    }

    fn infer_expr(&mut self, expr: &Expr, scope: &Scope) -> Option<TypeName> {
        match expr {
            Expr::Ident(ident) => self.infer_identifier(ident, scope),
            Expr::Integer { .. } => Some(TypeName::Amount),
            Expr::String { .. } => Some(TypeName::Bytes),
            Expr::Bool { .. } => Some(TypeName::Bool),
            Expr::Array { elements, .. } => {
                for element in elements {
                    self.infer_expr(element, scope);
                }
                Some(TypeName::Bytes)
            }
            Expr::Unary { op, expr, .. } => {
                let ty = self.infer_expr(expr, scope);
                if *op == UnaryOp::Not && !matches!(ty, Some(TypeName::Bool)) {
                    self.error(expr.span(), "`!` requires Bool");
                }
                Some(TypeName::Bool)
            }
            Expr::Binary {
                left, op, right, ..
            } => {
                let left_ty = self.infer_expr(left, scope);
                let right_ty = self.infer_expr(right, scope);
                match op {
                    BinaryOp::And | BinaryOp::Or => {
                        if !matches!(left_ty, Some(TypeName::Bool))
                            || !matches!(right_ty, Some(TypeName::Bool))
                        {
                            self.error(expr.span(), "logical operators require Bool operands");
                        }
                        Some(TypeName::Bool)
                    }
                    BinaryOp::Equal
                    | BinaryOp::NotEqual
                    | BinaryOp::Greater
                    | BinaryOp::GreaterEqual
                    | BinaryOp::Less
                    | BinaryOp::LessEqual => Some(TypeName::Bool),
                    BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::Mul
                    | BinaryOp::Div
                    | BinaryOp::Mod => Some(TypeName::Amount),
                }
            }
            Expr::Call { callee, args, .. } => self.infer_call(callee, args, scope),
            Expr::Field { object, field, .. } => self.infer_field(object, field, scope),
        }
    }

    fn infer_identifier(&mut self, ident: &Ident, scope: &Scope) -> Option<TypeName> {
        if self.spend_names.contains(&ident.name) {
            self.error(
                ident.span,
                "spend functions cannot be referenced from require expressions",
            );
            return None;
        }
        match ident.name.as_str() {
            "block" => Some(TypeName::BlockHeight),
            "covenant_id" => Some(TypeName::CovenantID),
            "sequencing" => Some(TypeName::Hash),
            "input_count" | "output_count" => Some(TypeName::Amount),
            "input" | "output" | "multisig" | "zk_verify" | "sha256" | "blake2b" | "hash160" => {
                Some(TypeName::Bool)
            }
            name => {
                let found = scope.get(name);
                if found.is_none() {
                    self.error(ident.span, format!("undefined identifier `{name}`"));
                }
                found
            }
        }
    }

    fn infer_call(&mut self, callee: &Expr, args: &[Expr], scope: &Scope) -> Option<TypeName> {
        if let Expr::Field { object, field, .. } = callee {
            if field.name == "verify" {
                let receiver = self.infer_expr(object, scope);
                if !matches!(receiver, Some(TypeName::Signature)) {
                    self.error(object.span(), "`verify` receiver must be Signature");
                }
                if args.len() != 1 {
                    self.error(field.span, "`verify` requires one PublicKey argument");
                } else if !matches!(self.infer_expr(&args[0], scope), Some(TypeName::PublicKey)) {
                    self.error(args[0].span(), "`verify` argument must be PublicKey");
                }
                return Some(TypeName::Bool);
            }
        }

        if let Expr::Ident(ident) = callee {
            match ident.name.as_str() {
                "input" => return self.check_index_call(args, ident.span, TypeName::Input),
                "output" => return self.check_index_call(args, ident.span, TypeName::Output),
                "continuation" => {
                    self.check_continuation(args, ident.span);
                    return Some(TypeName::Bool);
                }
                "zk_verify" => {
                    if args.len() != 1 {
                        self.error(ident.span, "`zk_verify` requires one proof argument");
                    } else if !matches!(self.infer_expr(&args[0], scope), Some(TypeName::ZKProof)) {
                        self.error(args[0].span(), "`zk_verify` argument must be ZKProof");
                    }
                    return Some(TypeName::Bool);
                }
                "multisig" => {
                    self.check_multisig(args, ident.span, scope);
                    return Some(TypeName::Bool);
                }
                "sha256" | "blake2b" | "hash160" => {
                    if args.len() != 1 {
                        self.error(ident.span, "hash functions require one argument");
                    } else {
                        self.infer_expr(&args[0], scope);
                    }
                    return Some(TypeName::Hash);
                }
                _ => {}
            }
        }

        self.infer_expr(callee, scope);
        for arg in args {
            self.infer_expr(arg, scope);
        }
        Some(TypeName::Bool)
    }

    fn check_index_call(&mut self, args: &[Expr], span: Span, ty: TypeName) -> Option<TypeName> {
        if args.len() != 1 {
            self.error(span, "input/output requires one integer index");
        } else {
            match args[0] {
                Expr::Integer { value, .. } if value <= u64::from(u32::MAX) => {}
                Expr::Integer { .. } => {
                    self.error(args[0].span(), "input/output index exceeds u32 range");
                }
                _ => {
                    self.error(
                        args[0].span(),
                        "input/output index must be a non-negative integer literal",
                    );
                }
            }
        }
        Some(ty)
    }

    fn check_continuation(&mut self, args: &[Expr], span: Span) {
        if args.len() != 2 {
            self.error(span, "`continuation` requires a name and output(index)");
            return;
        }

        match &args[0] {
            Expr::String { value, span } if is_valid_continuation_name(value) => {
                if value.trim() != value {
                    self.error(
                        *span,
                        "`continuation` name must not have leading or trailing space",
                    );
                }
            }
            Expr::String { span, .. } => {
                self.error(
                    *span,
                    "`continuation` name must be a non-empty identifier label",
                );
            }
            other => self.error(other.span(), "`continuation` name must be a string literal"),
        }

        if output_call_index(&args[1]).is_none() {
            self.error(
                args[1].span(),
                "`continuation` second argument must be output(index)",
            );
        }
    }

    fn check_multisig(&mut self, args: &[Expr], span: Span, scope: &Scope) {
        if args.len() != 3 {
            self.error(span, "`multisig` requires k, keys, and signatures");
            return;
        }
        let k = match args[0] {
            Expr::Integer { value, .. } => Some(value),
            _ => {
                self.error(args[0].span(), "`multisig` k must be an integer literal");
                None
            }
        };
        let key_count = match &args[1] {
            Expr::Array { elements, .. } => Some(elements.len() as u64),
            _ => None,
        };
        if let (Some(k), Some(key_count)) = (k, key_count) {
            if k > key_count {
                self.error(args[0].span(), "`multisig` k cannot exceed number of keys");
            }
        }
        for arg in args {
            self.infer_expr(arg, scope);
        }
    }

    fn infer_field(&mut self, object: &Expr, field: &Ident, scope: &Scope) -> Option<TypeName> {
        if let Expr::Ident(root) = object {
            if root.name == "block" {
                return match field.name.as_str() {
                    "height" => Some(TypeName::BlockHeight),
                    "time" => Some(TypeName::BlockHeight),
                    _ => {
                        self.error(field.span, "`block` only supports `.height` and `.time`");
                        None
                    }
                };
            }
            if root.name == "covenant_id" && field.name == "depth" {
                return Some(TypeName::BlockHeight);
            }
            if root.name == "sequencing" {
                return Some(TypeName::BlockHeight);
            }
        }

        match self.infer_expr(object, scope) {
            Some(TypeName::Input) | Some(TypeName::Output) => match field.name.as_str() {
                "value" => Some(TypeName::Amount),
                "script" => Some(TypeName::Bytes),
                "covenant_id" => Some(TypeName::CovenantID),
                _ => {
                    self.error(field.span, "unknown input/output field");
                    None
                }
            },
            other => other,
        }
    }

    fn error(&mut self, span: Span, message: impl Into<String>) {
        let SourceLocation { file, line, column } = locate(self.source, self.file, span.start);
        self.errors.push(SemanticError {
            file,
            line,
            column,
            span,
            message: message.into(),
        });
    }

    fn collect_shape_facts(&mut self, expr: &Expr, facts: &mut ShapeFacts) {
        collect_exact_counts_from_conjunction(expr, facts, self);
        collect_index_references(expr, facts);
    }

    fn collect_continuation_names(&mut self, expr: &Expr, names: &mut HashSet<String>) {
        if let Some((name, span)) = continuation_name(expr) {
            if !names.insert(name.clone()) {
                self.error(span, format!("duplicate continuation name `{name}`"));
            }
        }

        match expr {
            Expr::Array { elements, .. } => {
                for element in elements {
                    self.collect_continuation_names(element, names);
                }
            }
            Expr::Unary { expr, .. } => self.collect_continuation_names(expr, names),
            Expr::Binary { left, right, .. } => {
                self.collect_continuation_names(left, names);
                self.collect_continuation_names(right, names);
            }
            Expr::Call { callee, args, .. } => {
                self.collect_continuation_names(callee, names);
                for arg in args {
                    self.collect_continuation_names(arg, names);
                }
            }
            Expr::Field { object, .. } => self.collect_continuation_names(object, names),
            Expr::Ident(_) | Expr::Integer { .. } | Expr::String { .. } | Expr::Bool { .. } => {}
        }
    }

    fn check_shape_facts(&mut self, facts: &ShapeFacts) {
        check_count_facts(self, "input_count", &facts.inputs);
        check_count_facts(self, "output_count", &facts.outputs);
    }
}

#[derive(Debug, Default)]
struct ShapeFacts {
    inputs: CountFacts,
    outputs: CountFacts,
}

#[derive(Debug, Default)]
struct CountFacts {
    exact: Option<(u64, Span)>,
    max_index: Option<(u64, Span)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CountSubject {
    Input,
    Output,
}

fn collect_exact_counts_from_conjunction(
    expr: &Expr,
    facts: &mut ShapeFacts,
    checker: &mut Checker<'_>,
) {
    if let Expr::Binary {
        left,
        op: BinaryOp::And,
        right,
        ..
    } = expr
    {
        collect_exact_counts_from_conjunction(left, facts, checker);
        collect_exact_counts_from_conjunction(right, facts, checker);
        return;
    }

    let Some((subject, value, span)) = exact_count_constraint(expr) else {
        return;
    };
    let facts = match subject {
        CountSubject::Input => &mut facts.inputs,
        CountSubject::Output => &mut facts.outputs,
    };
    if value > u64::from(u32::MAX) {
        checker.error(span, "exact transaction count exceeds u32 range");
    }
    if let Some((existing, _)) = facts.exact {
        if existing != value {
            checker.error(span, "conflicting exact transaction count constraints");
        }
    } else {
        facts.exact = Some((value, span));
    }
}

fn exact_count_constraint(expr: &Expr) -> Option<(CountSubject, u64, Span)> {
    let Expr::Binary {
        left,
        op: BinaryOp::Equal,
        right,
        ..
    } = expr
    else {
        return None;
    };

    if let (Some(subject), Expr::Integer { value, span }) = (count_subject(left), &**right) {
        return Some((subject, *value, *span));
    }
    if let (Expr::Integer { value, span }, Some(subject)) = (&**left, count_subject(right)) {
        return Some((subject, *value, *span));
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

fn collect_index_references(expr: &Expr, facts: &mut ShapeFacts) {
    if let Expr::Call { callee, args, .. } = expr {
        if let Expr::Ident(ident) = &**callee {
            let facts = match ident.name.as_str() {
                "input" => Some(&mut facts.inputs),
                "output" => Some(&mut facts.outputs),
                _ => None,
            };
            if let (Some(facts), Some(Expr::Integer { value, span })) = (facts, args.first()) {
                match facts.max_index {
                    Some((current, _)) if current >= *value => {}
                    _ => facts.max_index = Some((*value, *span)),
                }
            }
        }
    }

    match expr {
        Expr::Array { elements, .. } => {
            for element in elements {
                collect_index_references(element, facts);
            }
        }
        Expr::Unary { expr, .. } => collect_index_references(expr, facts),
        Expr::Binary { left, right, .. } => {
            collect_index_references(left, facts);
            collect_index_references(right, facts);
        }
        Expr::Call { callee, args, .. } => {
            collect_index_references(callee, facts);
            for arg in args {
                collect_index_references(arg, facts);
            }
        }
        Expr::Field { object, .. } => collect_index_references(object, facts),
        Expr::Ident(_) | Expr::Integer { .. } | Expr::String { .. } | Expr::Bool { .. } => {}
    }
}

fn check_count_facts(checker: &mut Checker<'_>, label: &str, facts: &CountFacts) {
    let Some((exact, _)) = facts.exact else {
        return;
    };
    let Some((max_index, max_span)) = facts.max_index else {
        return;
    };
    if exact <= max_index {
        checker.error(
            max_span,
            format!("`{label}` must exceed the highest referenced index"),
        );
    }
}

fn continuation_name(expr: &Expr) -> Option<(String, Span)> {
    let Expr::Call { callee, args, .. } = expr else {
        return None;
    };
    let Expr::Ident(ident) = &**callee else {
        return None;
    };
    if ident.name != "continuation" {
        return None;
    }
    let Some(Expr::String { value, span }) = args.first() else {
        return None;
    };
    Some((value.clone(), *span))
}

fn output_call_index(expr: &Expr) -> Option<u32> {
    let Expr::Call { callee, args, .. } = expr else {
        return None;
    };
    let Expr::Ident(ident) = &**callee else {
        return None;
    };
    if ident.name != "output" {
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

fn is_valid_continuation_name(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch == '-' || ch.is_ascii_alphanumeric())
}

fn spend_uses_name(spend: &Spend, name: &str) -> bool {
    spend.body.iter().any(|stmt| match stmt {
        Stmt::Let { expr, .. } | Stmt::Require { expr, .. } | Stmt::Return { expr, .. } => {
            expr_uses_name(expr, name)
        }
    })
}

fn expr_uses_name(expr: &Expr, name: &str) -> bool {
    match expr {
        Expr::Ident(ident) => ident.name == name,
        Expr::Integer { .. } | Expr::String { .. } | Expr::Bool { .. } => false,
        Expr::Array { elements, .. } => elements.iter().any(|expr| expr_uses_name(expr, name)),
        Expr::Unary { expr, .. } => expr_uses_name(expr, name),
        Expr::Binary { left, right, .. } => {
            expr_uses_name(left, name) || expr_uses_name(right, name)
        }
        Expr::Call { callee, args, .. } => {
            expr_uses_name(callee, name) || args.iter().any(|expr| expr_uses_name(expr, name))
        }
        Expr::Field { object, field, .. } => field.name == name || expr_uses_name(object, name),
    }
}

fn collect_kip_requirements(program: &Program) -> Vec<u16> {
    let mut kips = HashSet::new();
    for contract in &program.contracts {
        for spend in &contract.spends {
            for stmt in &spend.body {
                let expr = match stmt {
                    Stmt::Let { expr, .. }
                    | Stmt::Require { expr, .. }
                    | Stmt::Return { expr, .. } => expr,
                };
                collect_expr_kips(expr, &mut kips);
            }
        }
    }
    let mut kips = kips.into_iter().collect::<Vec<_>>();
    kips.sort_unstable();
    kips
}

fn collect_expr_kips(expr: &Expr, kips: &mut HashSet<u16>) {
    match expr {
        Expr::Ident(ident) => match ident.name.as_str() {
            "input" | "output" | "input_count" | "output_count" => {
                kips.insert(10);
            }
            "covenant_id" => {
                kips.insert(20);
            }
            "zk_verify" => {
                kips.insert(16);
            }
            "sequencing" => {
                kips.insert(15);
            }
            _ => {}
        },
        Expr::Integer { .. } | Expr::String { .. } | Expr::Bool { .. } => {}
        Expr::Array { elements, .. } => {
            for element in elements {
                collect_expr_kips(element, kips);
            }
        }
        Expr::Unary { expr, .. } => collect_expr_kips(expr, kips),
        Expr::Binary { left, right, .. } => {
            collect_expr_kips(left, kips);
            collect_expr_kips(right, kips);
        }
        Expr::Call { callee, args, .. } => {
            collect_expr_kips(callee, kips);
            for arg in args {
                collect_expr_kips(arg, kips);
            }
        }
        Expr::Field { object, field, .. } => {
            if field.name == "covenant_id" {
                kips.insert(20);
            }
            collect_expr_kips(object, kips);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_multiple_errors() {
        let result = analyze(
            r#"
            contract Bad {
              params { owner: PublicKey }
              spend s(sig: PublicKey) {
                require sig.verify(owner);
                require missing;
              }
            }
            "#,
        )
        .expect_err("semantic errors");
        assert!(result.errors.len() >= 2);
    }

    #[test]
    fn checks_multisig_static_k() {
        let result = analyze(
            r#"
            contract Bad {
              params { a: PublicKey, b: PublicKey, sig: Signature }
              spend s() {
                require multisig(3, [a, b], [sig]);
              }
            }
            "#,
        )
        .expect_err("k exceeds keys");
        assert!(result
            .errors
            .iter()
            .any(|error| error.message == "`multisig` k cannot exceed number of keys"));
    }

    #[test]
    fn accepts_exact_counts_and_named_continuation() {
        let analysis = analyze(
            r#"
            contract Shape {
              params { owner: PublicKey }
              spend advance(sig: Signature) {
                require sig.verify(owner);
                require input_count == 1 && output_count == 2;
                require continuation("state", output(1));
                require output(1).script == owner;
              }
            }
            "#,
        )
        .expect("transaction shape syntax is valid");

        assert!(analysis.kip_requirements.contains(&10));
    }

    #[test]
    fn rejects_exact_count_below_referenced_index() {
        let result = analyze(
            r#"
            contract BadShape {
              params { owner: PublicKey }
              spend s(sig: Signature) {
                require sig.verify(owner);
                require input_count == 1;
                require input(1).value >= 0;
              }
            }
            "#,
        )
        .expect_err("referenced input exceeds exact count");

        assert!(result.errors.iter().any(|error| {
            error.message == "`input_count` must exceed the highest referenced index"
        }));
    }

    #[test]
    fn rejects_duplicate_continuation_names() {
        let result = analyze(
            r#"
            contract BadContinuation {
              params { owner: PublicKey }
              spend s(sig: Signature) {
                require sig.verify(owner);
                require continuation("state", output(0));
                require continuation("state", output(1));
              }
            }
            "#,
        )
        .expect_err("duplicate continuation name");

        assert!(result
            .errors
            .iter()
            .any(|error| error.message == "duplicate continuation name `state`"));
    }
}
