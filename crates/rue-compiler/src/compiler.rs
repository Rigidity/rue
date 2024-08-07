#![allow(clippy::map_unwrap_or)]

use rue_typing::{HashMap, TypePath};

pub(crate) use builtins::Builtins;

use rowan::TextRange;
use rue_typing::{Comparison, TypeId, TypeSystem};
use symbol_table::SymbolTable;

use crate::{
    database::{Database, HirId, ScopeId, SymbolId},
    hir::{Hir, Op},
    scope::Scope,
    symbol::{Function, Symbol},
    value::{GuardPath, Value},
    ErrorKind,
};

mod block;
mod builtins;
mod context;
mod expr;
mod item;
mod path;
mod stmt;
mod symbol_table;
mod ty;

pub use context::*;

/// Responsible for lowering the AST into the HIR.
/// Performs name resolution and type checking.
pub struct Compiler<'a> {
    // The database is mutable because we need to allocate new symbols and types.
    db: &'a mut Database,

    // The type system is responsible for type checking and type inference.
    ty: &'a mut TypeSystem,

    // The scope stack is used to keep track of the current scope.
    scope_stack: Vec<ScopeId>,

    // The symbol stack is used for calculating types referenced in symbols.
    symbol_stack: Vec<SymbolId>,

    // The type definition stack is used for calculating types referenced in types.
    type_definition_stack: Vec<TypeId>,

    // Overridden symbol types due to type guards.
    type_overrides: Vec<HashMap<SymbolId, TypeId>>,

    // The generic type stack is used for overriding generic types that are being checked against.
    generic_type_stack: Vec<HashMap<TypeId, TypeId>>,

    // Whether or not generic type inference is allowed.
    allow_generic_inference_stack: Vec<bool>,

    // Whether the current expression is directly the callee of a function call.
    is_callee: bool,

    // The symbol table is used for storing all named symbols and types.
    // It also stored types referenced by symbols.
    sym: SymbolTable,

    // Common types and other values that are built-in to the compiler.
    builtins: Builtins,
}

impl<'a> Compiler<'a> {
    pub fn new(db: &'a mut Database, ty: &'a mut TypeSystem, builtins: Builtins) -> Self {
        Self {
            db,
            ty,
            scope_stack: vec![builtins.scope_id],
            symbol_stack: Vec::new(),
            type_definition_stack: Vec::new(),
            type_overrides: Vec::new(),
            generic_type_stack: Vec::new(),
            allow_generic_inference_stack: vec![false],
            is_callee: false,
            sym: SymbolTable::default(),
            builtins,
        }
    }

    /// Lowering is completed, extract the diagnostics.
    pub fn finish(self) -> SymbolTable {
        self.sym
    }

    fn hir_path(&mut self, mut value: HirId, path_items: &[TypePath]) -> HirId {
        for path in path_items {
            match path {
                TypePath::First => value = self.db.alloc_hir(Hir::Op(Op::First, value)),
                TypePath::Rest => value = self.db.alloc_hir(Hir::Op(Op::Rest, value)),
            }
        }
        value
    }

    fn type_reference(&mut self, referenced_type_id: TypeId) {
        if let Some(symbol_id) = self.symbol_stack.last() {
            self.sym
                .insert_symbol_type_reference(*symbol_id, referenced_type_id);
        }

        if let Some(type_id) = self.type_definition_stack.last() {
            self.sym
                .insert_type_type_reference(*type_id, referenced_type_id);
        }
    }

    fn symbol_name(&self, symbol_id: SymbolId) -> Option<String> {
        for &scope_id in self.scope_stack.iter().rev() {
            if let Some(name) = self.db.scope(scope_id).symbol_name(symbol_id) {
                return Some(name.to_string());
            }
        }
        None
    }

    fn type_name(&self, type_id: TypeId) -> String {
        let mut names = HashMap::new();

        for &scope_id in &self.scope_stack {
            for type_id in self.db.scope(scope_id).local_types() {
                if let Some(name) = self.db.scope(scope_id).type_name(type_id) {
                    names.insert(type_id, name.to_string());
                }
            }
        }

        self.ty.stringify_named(type_id, names)
    }

    fn type_check(&mut self, from: TypeId, to: TypeId, range: TextRange) {
        let comparison = if self.allow_generic_inference_stack.last().copied().unwrap() {
            self.ty
                .compare_with_generics(from, to, &mut self.generic_type_stack, true)
        } else {
            self.ty.compare(from, to)
        };

        if comparison > Comparison::Assignable {
            self.db.error(
                ErrorKind::TypeMismatch(self.type_name(from), self.type_name(to)),
                range,
            );
        }
    }

    fn cast_check(&mut self, from: TypeId, to: TypeId, range: TextRange) {
        let comparison = if self.allow_generic_inference_stack.last().copied().unwrap() {
            self.ty
                .compare_with_generics(from, to, &mut self.generic_type_stack, true)
        } else {
            self.ty.compare(from, to)
        };

        if comparison > Comparison::Castable {
            self.db.error(
                ErrorKind::CastMismatch(self.type_name(from), self.type_name(to)),
                range,
            );
        }
    }

    fn unknown(&self) -> Value {
        Value::new(self.builtins.unknown, self.ty.std().unknown)
    }

    fn build_overrides(&mut self, guards: HashMap<GuardPath, TypeId>) -> HashMap<SymbolId, TypeId> {
        type GuardItem = (Vec<TypePath>, TypeId);

        let mut symbol_guards: HashMap<SymbolId, Vec<GuardItem>> = HashMap::new();

        for (guard_path, type_id) in guards {
            symbol_guards
                .entry(guard_path.symbol_id)
                .or_default()
                .push((guard_path.items, type_id));
        }

        let mut overrides = HashMap::new();

        for (symbol_id, mut items) in symbol_guards {
            // Order by length.
            items.sort_by_key(|(items, _)| items.len());

            let mut type_id = self.symbol_type(symbol_id);

            for (path_items, new_type_id) in items {
                type_id = self.ty.replace(type_id, new_type_id, &path_items);
            }

            overrides.insert(symbol_id, type_id);
        }

        overrides
    }

    fn symbol_type(&self, symbol_id: SymbolId) -> TypeId {
        for guards in self.type_overrides.iter().rev() {
            if let Some(type_id) = guards.get(&symbol_id) {
                return *type_id;
            }
        }

        match self.db.symbol(symbol_id) {
            Symbol::Unknown | Symbol::Module(..) => unreachable!(),
            Symbol::Function(Function { type_id, .. })
            | Symbol::InlineFunction(Function { type_id, .. })
            | Symbol::Parameter(type_id) => *type_id,
            Symbol::Let(value) | Symbol::Const(value) | Symbol::InlineConst(value) => value.type_id,
        }
    }

    fn scope(&self) -> &Scope {
        self.db
            .scope(self.scope_stack.last().copied().expect("no scope found"))
    }

    fn scope_mut(&mut self) -> &mut Scope {
        self.db
            .scope_mut(self.scope_stack.last().copied().expect("no scope found"))
    }
}
