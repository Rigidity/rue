use indexmap::{IndexMap, IndexSet};
use rowan::TextRange;

use crate::{
    environment::Environment,
    hir::Hir,
    symbol::{Function, Module, Symbol},
    Database, EnvironmentId, ErrorKind, HirId, ScopeId, SymbolId,
};

#[derive(Debug, Default, Clone)]
pub struct DependencyGraph {
    environments: IndexMap<ScopeId, EnvironmentId>,
    parent_scopes: IndexMap<ScopeId, IndexSet<ScopeId>>,
    symbol_references: IndexMap<SymbolId, usize>,
    references: IndexMap<SymbolId, IndexSet<SymbolId>>,
}

impl DependencyGraph {
    pub fn scopes(&self) -> Vec<ScopeId> {
        self.environments.keys().copied().collect()
    }

    pub fn symbol_references(&self, symbol_id: SymbolId) -> usize {
        self.symbol_references
            .get(&symbol_id)
            .copied()
            .unwrap_or_default()
    }

    pub fn all_references(&self, symbol_id: SymbolId) -> IndexSet<SymbolId> {
        let mut visited = IndexSet::new();
        let mut stack = vec![symbol_id];

        while let Some(symbol_id) = stack.pop() {
            if !visited.insert(symbol_id) {
                continue;
            }

            if let Some(references) = self.references.get(&symbol_id) {
                for &reference in references {
                    stack.push(reference);
                }
            }
        }

        visited.shift_remove(&symbol_id);
        visited
    }

    pub fn environment_id(&self, scope_id: ScopeId) -> EnvironmentId {
        self.environments[&scope_id]
    }
}

impl DependencyGraph {
    pub fn build(db: &mut Database, entrypoint: &Module) -> Self {
        let mut builder = GraphBuilder {
            db,
            graph: Self::default(),
            symbol_stack: IndexSet::new(),
            visited: IndexSet::new(),
        };
        builder.walk_module(entrypoint);
        assert!(builder.symbol_stack.is_empty(), "symbol stack is not empty");
        builder.visited.clear();
        builder.ref_module(entrypoint);
        builder.graph
    }
}

struct GraphBuilder<'a> {
    db: &'a mut Database,
    graph: DependencyGraph,
    symbol_stack: IndexSet<SymbolId>,
    visited: IndexSet<(ScopeId, HirId)>,
}

impl<'a> GraphBuilder<'a> {
    fn walk_module(&mut self, module: &Module) {
        if self.graph.environments.contains_key(&module.scope_id) {
            log::debug!(
                "Skipping module {}, since it's already been visited.",
                self.db.dbg_scope(module.scope_id)
            );
            return;
        }

        let environment_id = self.db.alloc_env(Environment::default());
        self.graph
            .environments
            .insert(module.scope_id, environment_id);

        for &symbol_id in &module.exported_symbols {
            self.walk_export(module.scope_id, symbol_id);
        }
    }

    fn walk_export(&mut self, scope_id: ScopeId, symbol_id: SymbolId) {
        match self.db.symbol(symbol_id).clone() {
            Symbol::Unknown | Symbol::Let(..) | Symbol::Parameter(..) => unreachable!(),
            Symbol::Module(module) => {
                self.walk_module(&module);
            }
            Symbol::Function(function) | Symbol::InlineFunction(function) => {
                self.graph
                    .parent_scopes
                    .entry(function.scope_id)
                    .or_default();

                self.walk_function(&function);
            }
            Symbol::Const(constant) | Symbol::InlineConst(constant) => {
                self.walk_hir(scope_id, constant.hir_id);
            }
        }
    }

    fn walk_function(&mut self, function: &Function) {
        if self.graph.environments.contains_key(&function.scope_id) {
            log::debug!(
                "Skipping function {}, since it's already been visited.",
                self.db.dbg_scope(function.scope_id)
            );
            return;
        }

        let parameters = self
            .db
            .scope(function.scope_id)
            .local_symbols()
            .into_iter()
            .filter(|&symbol_id| matches!(self.db.symbol(symbol_id), Symbol::Parameter(_)))
            .collect();

        let environment_id = self
            .db
            .alloc_env(Environment::function(parameters, !function.nil_terminated));

        self.graph
            .environments
            .insert(function.scope_id, environment_id);

        self.walk_hir(function.scope_id, function.hir_id);
    }

    fn walk_hir(&mut self, scope_id: ScopeId, hir_id: HirId) {
        if !self.visited.insert((scope_id, hir_id)) {
            return;
        }

        match self.db.hir(hir_id).clone() {
            Hir::Unknown | Hir::Atom(..) => {}
            Hir::Op(_op, hir_id) => {
                self.walk_hir(scope_id, hir_id);
            }
            Hir::Raise(hir_id) => {
                if let Some(hir_id) = hir_id {
                    self.walk_hir(scope_id, hir_id);
                }
            }
            Hir::Pair(first, rest) => {
                self.walk_hir(scope_id, first);
                self.walk_hir(scope_id, rest);
            }
            Hir::FunctionCall(callee, args, _varargs) => {
                self.walk_hir(scope_id, callee);
                for arg in args {
                    self.walk_hir(scope_id, arg);
                }
            }
            Hir::If(condition, then_block, else_block) => {
                self.walk_hir(scope_id, condition);
                self.walk_hir(scope_id, then_block);
                self.walk_hir(scope_id, else_block);
            }
            Hir::BinaryOp(_op, lhs, rhs) => {
                self.walk_hir(scope_id, lhs);
                self.walk_hir(scope_id, rhs);
            }
            Hir::Substr(value, start, end) => {
                self.walk_hir(scope_id, value);
                self.walk_hir(scope_id, start);
                self.walk_hir(scope_id, end);
            }
            Hir::Definition(child_scope_id, hir_id) => {
                self.walk_definition(scope_id, child_scope_id, hir_id);
            }
            Hir::Reference(symbol_id, text_range) => {
                self.walk_reference(scope_id, symbol_id, text_range);
            }
        }
    }

    fn walk_definition(
        &mut self,
        parent_scope_id: ScopeId,
        child_scope_id: ScopeId,
        hir_id: HirId,
    ) {
        if self.graph.environments.contains_key(&child_scope_id) {
            log::debug!(
                "Skipping definition {}, since it's already been visited.",
                self.db.dbg_scope(child_scope_id)
            );
            return;
        }

        self.graph
            .parent_scopes
            .entry(child_scope_id)
            .or_default()
            .insert(parent_scope_id);

        let parent_env_id = self.graph.environments[&parent_scope_id];
        let child_env_id = self.db.alloc_env(Environment::binding(parent_env_id));
        self.graph.environments.insert(child_scope_id, child_env_id);
        self.walk_hir(child_scope_id, hir_id);
    }

    fn walk_reference(&mut self, scope_id: ScopeId, symbol_id: SymbolId, text_range: TextRange) {
        let symbol = self.db.symbol(symbol_id).clone();

        for &key in &self.symbol_stack {
            self.graph
                .references
                .entry(key)
                .or_default()
                .insert(symbol_id);
        }

        if !self.symbol_stack.insert(symbol_id) {
            let error = match symbol {
                Symbol::Const(..) => Some(ErrorKind::RecursiveConstantReference),
                Symbol::InlineConst(..) => Some(ErrorKind::RecursiveInlineConstantReference),
                Symbol::InlineFunction(..) => Some(ErrorKind::RecursiveInlineFunctionCall),
                _ => None,
            };

            if let Some(error) = error {
                self.db.error(error, text_range);
                return;
            }
        }

        match symbol {
            Symbol::Unknown | Symbol::Module(..) => unreachable!(),
            Symbol::Function(function) | Symbol::InlineFunction(function) => {
                self.graph
                    .parent_scopes
                    .entry(function.scope_id)
                    .or_default()
                    .insert(scope_id);

                self.walk_function(&function);
            }
            Symbol::Parameter(..) => {}
            Symbol::Let(value) | Symbol::Const(value) | Symbol::InlineConst(value) => {
                self.walk_hir(scope_id, value.hir_id);
            }
        }

        self.symbol_stack.shift_remove(&symbol_id);

        if let Some(references) = self.graph.references.get(&symbol_id).cloned() {
            for reference in references {
                if !self
                    .graph
                    .references
                    .get(&reference)
                    .cloned()
                    .unwrap_or_default()
                    .contains(&symbol_id)
                {
                    continue;
                }

                if let Symbol::Function(..) = self.db.symbol(reference) {
                    let error = match self.db.symbol(symbol_id) {
                        Symbol::Const(..) => Some(ErrorKind::RecursiveConstantReference),
                        Symbol::InlineConst(..) => {
                            Some(ErrorKind::RecursiveInlineConstantReference)
                        }
                        Symbol::InlineFunction(..) => Some(ErrorKind::RecursiveInlineFunctionCall),
                        _ => None,
                    };

                    if let Some(error) = error {
                        self.db.error(error, text_range);
                        return;
                    }
                }
            }
        }
    }

    fn propagate_capture(
        &mut self,
        scope_id: ScopeId,
        symbol_id: SymbolId,
        visited_scopes: &mut IndexSet<ScopeId>,
    ) {
        if !visited_scopes.insert(scope_id) {
            return;
        }

        let capturable = self.db.symbol(symbol_id).is_capturable();
        let definable = self.db.symbol(symbol_id).is_definable();
        let is_local = self.db.scope(scope_id).is_local(symbol_id);

        if is_local && definable {
            self.db
                .env_mut(self.graph.environments[&scope_id])
                .define(symbol_id);
        } else if !is_local && capturable {
            self.db
                .env_mut(self.graph.environments[&scope_id])
                .capture(symbol_id);

            for parent_scope_id in self.graph.parent_scopes[&scope_id].clone() {
                self.propagate_capture(parent_scope_id, symbol_id, visited_scopes);
            }
        }
    }

    fn ref_module(&mut self, module: &Module) {
        for &symbol_id in &module.exported_symbols {
            self.ref_symbol(module.scope_id, symbol_id);
        }
    }

    fn ref_symbol(&mut self, scope_id: ScopeId, symbol_id: SymbolId) {
        match self.db.symbol(symbol_id).clone() {
            Symbol::Unknown => unreachable!(),
            Symbol::Module(module) => {
                self.ref_module(&module);
            }
            Symbol::Function(function) | Symbol::InlineFunction(function) => {
                self.ref_hir(function.scope_id, function.hir_id);
            }
            Symbol::Parameter(..) => {}
            Symbol::Let(value) | Symbol::Const(value) | Symbol::InlineConst(value) => {
                self.ref_hir(scope_id, value.hir_id);
            }
        }
    }

    fn ref_hir(&mut self, scope_id: ScopeId, hir_id: HirId) {
        if !self.visited.insert((scope_id, hir_id)) {
            return;
        }

        match self.db.hir(hir_id).clone() {
            Hir::Unknown | Hir::Atom(..) => {}
            Hir::Op(_op, hir_id) => {
                self.ref_hir(scope_id, hir_id);
            }
            Hir::Raise(hir_id) => {
                if let Some(hir_id) = hir_id {
                    self.ref_hir(scope_id, hir_id);
                }
            }
            Hir::Pair(first, rest) => {
                self.ref_hir(scope_id, first);
                self.ref_hir(scope_id, rest);
            }
            Hir::FunctionCall(callee, args, _varargs) => {
                self.ref_hir(scope_id, callee);
                for arg in args {
                    self.ref_hir(scope_id, arg);
                }
            }
            Hir::If(condition, then_block, else_block) => {
                self.ref_hir(scope_id, condition);
                self.ref_hir(scope_id, then_block);
                self.ref_hir(scope_id, else_block);
            }
            Hir::BinaryOp(_op, lhs, rhs) => {
                self.ref_hir(scope_id, lhs);
                self.ref_hir(scope_id, rhs);
            }
            Hir::Substr(value, start, end) => {
                self.ref_hir(scope_id, value);
                self.ref_hir(scope_id, start);
                self.ref_hir(scope_id, end);
            }
            Hir::Definition(scope_id, hir_id) => {
                self.ref_hir(scope_id, hir_id);
            }
            Hir::Reference(symbol_id, ..) => {
                self.resolve_reference(scope_id, symbol_id);
            }
        }
    }

    fn resolve_reference(&mut self, scope_id: ScopeId, symbol_id: SymbolId) {
        let symbol = self.db.symbol(symbol_id).clone();

        if self.db.symbol(symbol_id).is_constant() && !self.symbol_stack.insert(symbol_id) {
            return;
        }

        self.graph
            .symbol_references
            .entry(symbol_id)
            .and_modify(|usages| *usages += 1)
            .or_insert(1);

        self.propagate_capture(scope_id, symbol_id, &mut IndexSet::new());

        match symbol {
            Symbol::Unknown | Symbol::Module(..) => {
                unreachable!()
            }
            Symbol::Let(value) | Symbol::Const(value) | Symbol::InlineConst(value) => {
                self.ref_hir(scope_id, value.hir_id);
            }
            Symbol::Function(function) => {
                self.ref_hir(function.scope_id, function.hir_id);
            }
            Symbol::InlineFunction(function) => {
                self.ref_inline_function(scope_id, &function);
            }
            Symbol::Parameter(..) => {}
        }

        self.symbol_stack.shift_remove(&symbol_id);
    }

    fn ref_inline_function(&mut self, scope_id: ScopeId, function: &Function) {
        let env_id = self.graph.environments[&scope_id];
        self.ref_hir(function.scope_id, function.hir_id);

        let env = self
            .db
            .env(self.graph.environments[&function.scope_id])
            .clone();

        for symbol_id in env.definitions() {
            if matches!(self.db.symbol(symbol_id), Symbol::Parameter(..)) {
                continue;
            }
            self.db.env_mut(env_id).define(symbol_id);
        }

        for symbol_id in env.captures() {
            if matches!(self.db.symbol(symbol_id), Symbol::Parameter(..)) {
                continue;
            }
            self.db.env_mut(env_id).capture(symbol_id);
        }
    }
}
