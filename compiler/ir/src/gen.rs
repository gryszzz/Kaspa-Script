use std::fmt;

use kaspascript_lexer::Span;
use kaspascript_parser::{parse_file, BinaryOp, Expr, Program, Stmt, UnaryOp};
use kaspascript_semantic::{analyze_program, AnalyzeFailure};
use thiserror::Error;

use crate::instructions::{Instruction, InstructionKind};
use crate::types::Value;

/// Complete lowered IR.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IrProgram {
    pub contracts: Vec<IrContract>,
    pub kip_requirements: Vec<u16>,
}

/// Contract-level IR.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IrContract {
    pub name: String,
    pub finality_depth: Option<u64>,
    pub spends: Vec<IrSpend>,
}

/// Spend-level IR.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IrSpend {
    pub name: String,
    pub instructions: Vec<Instruction>,
}

/// IR generation error.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IrError {
    #[error("{0}")]
    Semantic(AnalyzeFailure),
    #[error("{file}:{line}:{column}: {message}")]
    Unsupported {
        file: String,
        line: usize,
        column: usize,
        message: String,
    },
}

/// Lowers source into opcode-agnostic IR.
pub fn lower_file(source: &str, file: &str) -> Result<IrProgram, IrError> {
    let program = parse_file(source, file).map_err(|error| {
        IrError::Semantic(AnalyzeFailure {
            error: kaspascript_semantic::AnalyzeError::Parse(error),
            errors: Vec::new(),
        })
    })?;
    let analysis = analyze_program(program.clone(), source, file).map_err(IrError::Semantic)?;
    lower_program(&analysis.program, analysis.kip_requirements, file)
}

/// Lowers source using `<source>` as the file name.
pub fn lower(source: &str) -> Result<IrProgram, IrError> {
    lower_file(source, "<source>")
}

/// Lowers a checked AST into IR.
pub fn lower_program(
    program: &Program,
    kip_requirements: Vec<u16>,
    file: &str,
) -> Result<IrProgram, IrError> {
    let mut contracts = Vec::new();
    for contract in &program.contracts {
        let mut spends = Vec::new();
        for spend in &contract.spends {
            let mut instructions = Vec::new();
            for stmt in &spend.body {
                match stmt {
                    Stmt::Let { name, expr, .. } => {
                        lower_expr(expr, &mut instructions, file)?;
                        instructions.push(Instruction::new(
                            name.span,
                            InstructionKind::Push(Value::Symbol(name.name.clone())),
                        ));
                    }
                    Stmt::Require { expr, span } => {
                        lower_expr(expr, &mut instructions, file)?;
                        instructions.push(Instruction::new(*span, InstructionKind::Verify));
                    }
                    Stmt::Return { expr, .. } => {
                        lower_expr(expr, &mut instructions, file)?;
                    }
                }
            }
            spends.push(IrSpend {
                name: spend.name.name.clone(),
                instructions,
            });
        }
        contracts.push(IrContract {
            name: contract.name.name.clone(),
            finality_depth: contract.finality_depth,
            spends,
        });
    }
    Ok(IrProgram {
        contracts,
        kip_requirements,
    })
}

fn lower_expr(expr: &Expr, out: &mut Vec<Instruction>, file: &str) -> Result<(), IrError> {
    match expr {
        Expr::Ident(ident) => {
            out.push(Instruction::new(
                ident.span,
                InstructionKind::Push(Value::Symbol(ident.name.clone())),
            ));
        }
        Expr::Integer { value, span } => {
            out.push(Instruction::new(
                *span,
                InstructionKind::Push(Value::Integer(*value)),
            ));
        }
        Expr::String { value, span } => {
            out.push(Instruction::new(
                *span,
                InstructionKind::Push(Value::String(value.clone())),
            ));
        }
        Expr::Bool { value, span } => {
            out.push(Instruction::new(
                *span,
                InstructionKind::Push(Value::Bool(*value)),
            ));
        }
        Expr::Array { elements, .. } => {
            for element in elements {
                lower_expr(element, out, file)?;
            }
        }
        Expr::Unary { op, expr, span } => {
            lower_expr(expr, out, file)?;
            match op {
                UnaryOp::Not => out.push(Instruction::new(*span, InstructionKind::Not)),
                UnaryOp::Negate => {
                    return Err(unsupported(
                        file,
                        *span,
                        "numeric negation is not supported in V1",
                    ));
                }
            }
        }
        Expr::Binary {
            left,
            op,
            right,
            span,
        } => lower_binary(left, *op, right, *span, out, file)?,
        Expr::Call { callee, args, span } => lower_call(callee, args, *span, out, file)?,
        Expr::Field {
            object,
            field,
            span,
        } => lower_field(object, &field.name, *span, out, file)?,
    }
    Ok(())
}

fn lower_binary(
    left: &Expr,
    op: BinaryOp,
    right: &Expr,
    span: Span,
    out: &mut Vec<Instruction>,
    file: &str,
) -> Result<(), IrError> {
    if let Some(instruction) = lower_special_binary(left, op, right, span) {
        out.push(instruction);
        return Ok(());
    }

    lower_expr(left, out, file)?;
    lower_expr(right, out, file)?;
    let kind = match op {
        BinaryOp::Or => InstructionKind::Or,
        BinaryOp::And => InstructionKind::And,
        BinaryOp::Equal => InstructionKind::Equal,
        BinaryOp::NotEqual => InstructionKind::NotEqual,
        BinaryOp::Greater => InstructionKind::GreaterThan,
        BinaryOp::GreaterEqual => InstructionKind::GreaterThanOrEqual,
        BinaryOp::Less => InstructionKind::GreaterThan,
        BinaryOp::LessEqual => InstructionKind::GreaterThanOrEqual,
        BinaryOp::Add => InstructionKind::Add,
        BinaryOp::Sub => InstructionKind::Sub,
        BinaryOp::Mul => InstructionKind::Mul,
        BinaryOp::Div => InstructionKind::Div,
        BinaryOp::Mod => InstructionKind::Mod,
    };
    out.push(Instruction::new(span, kind));
    Ok(())
}

fn lower_special_binary(
    left: &Expr,
    op: BinaryOp,
    right: &Expr,
    span: Span,
) -> Option<Instruction> {
    if op == BinaryOp::GreaterEqual && is_path(left, &["block", "height"]) {
        if let Expr::Integer { value, .. } = right {
            return Some(Instruction::new(
                span,
                InstructionKind::CheckLockHeight(*value),
            ));
        }
    }
    if op == BinaryOp::GreaterEqual && is_path(left, &["block", "time"]) {
        if let Expr::Integer { value, .. } = right {
            return Some(Instruction::new(
                span,
                InstructionKind::CheckLockTime(*value),
            ));
        }
    }
    None
}

fn lower_call(
    callee: &Expr,
    args: &[Expr],
    span: Span,
    out: &mut Vec<Instruction>,
    file: &str,
) -> Result<(), IrError> {
    if let Expr::Field { object, field, .. } = callee {
        if field.name == "verify" {
            lower_expr(object, out, file)?;
            if let Some(key) = args.first() {
                lower_expr(key, out, file)?;
            }
            out.push(Instruction::new(
                span,
                InstructionKind::CheckSig { key_slot: 0 },
            ));
            return Ok(());
        }
    }

    if let Expr::Ident(ident) = callee {
        match ident.name.as_str() {
            "input" => return lower_index_call(args, span, out, true, file),
            "output" => return lower_index_call(args, span, out, false, file),
            "multisig" => return lower_multisig(args, span, out, file),
            "zk_verify" => {
                if let Some(proof) = args.first() {
                    lower_expr(proof, out, file)?;
                }
                out.push(Instruction::new(span, InstructionKind::ZkVerifyGroth16));
                return Ok(());
            }
            "sha256" | "blake2b" | "hash160" => {
                if let Some(value) = args.first() {
                    lower_expr(value, out, file)?;
                }
                let kind = if ident.name == "sha256" {
                    InstructionKind::Sha256
                } else if ident.name == "blake2b" {
                    InstructionKind::Blake2b
                } else {
                    InstructionKind::Hash160
                };
                out.push(Instruction::new(span, kind));
                return Ok(());
            }
            _ => {}
        }
    }

    lower_expr(callee, out, file)?;
    for arg in args {
        lower_expr(arg, out, file)?;
    }
    Ok(())
}

fn lower_index_call(
    args: &[Expr],
    span: Span,
    out: &mut Vec<Instruction>,
    _is_input: bool,
    file: &str,
) -> Result<(), IrError> {
    let Some(Expr::Integer { value, .. }) = args.first() else {
        return Err(unsupported(
            file,
            span,
            "input/output index must be an integer literal",
        ));
    };
    let index = u32::try_from(*value).map_err(|_| unsupported(file, span, "index exceeds u32"))?;
    out.push(Instruction::new(
        span,
        InstructionKind::Push(Value::Integer(u64::from(index))),
    ));
    Ok(())
}

fn lower_multisig(
    args: &[Expr],
    span: Span,
    out: &mut Vec<Instruction>,
    file: &str,
) -> Result<(), IrError> {
    let required = match args.first() {
        Some(Expr::Integer { value, .. }) => {
            u32::try_from(*value).map_err(|_| unsupported(file, span, "multisig k exceeds u32"))?
        }
        _ => {
            return Err(unsupported(
                file,
                span,
                "multisig k must be an integer literal",
            ))
        }
    };
    let key_count = match args.get(1) {
        Some(Expr::Array { elements, .. }) => elements.len() as u32,
        _ => 0,
    };
    for arg in args.iter().skip(1) {
        lower_expr(arg, out, file)?;
    }
    out.push(Instruction::new(
        span,
        InstructionKind::CheckMultiSig {
            required,
            key_count,
        },
    ));
    Ok(())
}

fn lower_field(
    object: &Expr,
    field: &str,
    span: Span,
    out: &mut Vec<Instruction>,
    file: &str,
) -> Result<(), IrError> {
    if let Expr::Call { callee, args, .. } = object {
        if let Expr::Ident(root) = &**callee {
            if root.name == "input" || root.name == "output" {
                let Some(Expr::Integer { value, .. }) = args.first() else {
                    return Err(unsupported(
                        file,
                        span,
                        "input/output index must be integer",
                    ));
                };
                let index = u32::try_from(*value)
                    .map_err(|_| unsupported(file, span, "index exceeds u32"))?;
                let kind = match (root.name.as_str(), field) {
                    ("input", "value") => InstructionKind::InputValue(index),
                    ("input", "script") => InstructionKind::InputScript(index),
                    ("output", "value") => InstructionKind::OutputValue(index),
                    ("output", "script") => InstructionKind::OutputScript(index),
                    ("input" | "output", "covenant_id") => InstructionKind::CovenantId,
                    _ => return Err(unsupported(file, span, "unsupported input/output field")),
                };
                out.push(Instruction::new(span, kind));
                return Ok(());
            }
        }
    }

    if is_path(object, &["covenant_id"]) && field == "depth" {
        out.push(Instruction::new(span, InstructionKind::CovenantDepth));
        return Ok(());
    }
    if is_path(object, &["sequencing"]) {
        out.push(Instruction::new(
            span,
            InstructionKind::SequencingCommitment,
        ));
        return Ok(());
    }

    lower_expr(object, out, file)?;
    out.push(Instruction::new(
        span,
        InstructionKind::Push(Value::Symbol(field.to_owned())),
    ));
    Ok(())
}

fn is_path(expr: &Expr, expected: &[&str]) -> bool {
    let mut parts = Vec::new();
    collect_path(expr, &mut parts);
    parts == expected
}

fn collect_path<'a>(expr: &'a Expr, parts: &mut Vec<&'a str>) {
    match expr {
        Expr::Ident(ident) => parts.push(ident.name.as_str()),
        Expr::Field { object, field, .. } => {
            collect_path(object, parts);
            parts.push(field.name.as_str());
        }
        _ => {}
    }
}

fn unsupported(file: &str, span: Span, message: impl Into<String>) -> IrError {
    IrError::Unsupported {
        file: file.to_owned(),
        line: 1,
        column: span.start + 1,
        message: message.into(),
    }
}

impl fmt::Display for IrProgram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "IR contracts: {}", self.contracts.len())?;
        for contract in &self.contracts {
            writeln!(f, "contract {}", contract.name)?;
            for spend in &contract.spends {
                writeln!(
                    f,
                    "  spend {}: {} instructions",
                    spend.name,
                    spend.instructions.len()
                )?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowers_require_to_verify() {
        let ir = lower(
            r#"
            contract Simple {
              params { owner: PublicKey }
              spend s(sig: Signature) {
                require sig.verify(owner);
              }
            }
            "#,
        )
        .expect("lowers");
        assert!(matches!(
            ir.contracts[0].spends[0]
                .instructions
                .last()
                .map(|i| &i.kind),
            Some(InstructionKind::Verify)
        ));
    }
}
