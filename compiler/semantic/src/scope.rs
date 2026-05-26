use std::collections::HashMap;

use kaspascript_lexer::TypeName;

/// Lexical scope for contract parameters, spend parameters, and let bindings.
#[derive(Debug, Clone, Default)]
pub struct Scope {
    symbols: HashMap<String, TypeName>,
}

impl Scope {
    /// Creates an empty scope.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a symbol and returns false if it already exists.
    pub fn insert(&mut self, name: impl Into<String>, ty: TypeName) -> bool {
        self.symbols.insert(name.into(), ty).is_none()
    }

    /// Looks up a symbol type.
    pub fn get(&self, name: &str) -> Option<TypeName> {
        self.symbols.get(name).copied()
    }

    /// Returns true if the scope contains the name.
    pub fn contains(&self, name: &str) -> bool {
        self.symbols.contains_key(name)
    }
}
