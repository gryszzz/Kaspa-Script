use kaspascript_lexer::TypeName;
use serde::{Deserialize, Serialize};

/// IR literal or symbolic value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Value {
    Integer(u64),
    Bool(bool),
    String(String),
    Symbol(String),
    Type(TypeName),
}
