use itertools::Itertools;
use rowan::ast::AstNode;
use rue_ast::{BinaryExpr, Block, CallExpr, Expr, FnItem, IfExpr, Item, LiteralExpr, Program};
use rue_error::Error;
use rue_syntax::{SyntaxKind, SyntaxToken};

mod database;
mod hir;
mod scope;
mod symbol;
mod ty;

pub use database::*;
pub use hir::*;
pub use scope::*;
pub use symbol::*;
use ty::Type;

pub use rue_ast::BinaryOp;

pub struct Output {
    pub errors: Vec<Error>,
    pub db: Database,
    pub scope: Option<Scope>,
}

pub fn lower(program: Program) -> Output {
    let mut lowerer = Lowerer::new();
    let scope = lowerer.lower_program(program);
    Output {
        errors: lowerer.errors,
        db: lowerer.db,
        scope,
    }
}

struct Lowerer {
    db: Database,
    scopes: Vec<Scope>,
    errors: Vec<Error>,
}

impl Lowerer {
    fn new() -> Self {
        Self {
            db: Database::new(),
            scopes: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn lower_program(&mut self, program: Program) -> Option<Scope> {
        let mut scope = Scope::default();
        scope.define_type("Int".into(), Type::Int);
        scope.define_type("String".into(), Type::String);
        self.scopes.push(scope);

        let symbol_ids = program
            .items()
            .into_iter()
            .map(|item| self.define_item(item))
            .collect_vec();

        let mut is_valid = true;
        for (i, item) in program.items().into_iter().enumerate() {
            if self.lower_item(item, symbol_ids[i]).is_none() {
                is_valid = false;
            }
        }

        is_valid.then(|| self.scopes.pop().unwrap())
    }

    fn lower_item(&mut self, item: Item, symbol_id: Option<SymbolId>) -> Option<()> {
        match item {
            Item::Fn(item) => self.lower_fn_item(item, symbol_id),
        }
    }

    fn lower_fn_item(&mut self, item: FnItem, symbol_id: Option<SymbolId>) -> Option<()> {
        let mut fn_scope = Scope::default();

        for (index, param) in item
            .param_list()
            .map(|list| list.params())
            .unwrap_or_default()
            .iter()
            .enumerate()
        {
            if let Some(name_token) = param.name() {
                let name = name_token.text().to_string();
                let ty = self.lower_type(param.ty()?)?;
                let symbol_id = self.db.new_symbol(Symbol::Parameter { ty, index });
                fn_scope.define_symbol(name, symbol_id);
            }
        }

        self.scopes.push(fn_scope);
        let block = item.block().and_then(|block| self.lower_block(block));

        symbol_id.and_then(|symbol_id| {
            block.and_then(|(ty, hir)| {
                let mut error = None;

                if let Symbol::Function {
                    return_type,
                    resolved_body,
                    scope,
                    ..
                } = &mut self.db.symbol_mut(symbol_id)
                {
                    if !ty.is_assignable_to(return_type) {
                        error = Some(format!("cannot return value of type `{ty}`, function has return type `{return_type}`"));
                    }
                    *resolved_body = Some(hir);
                    *scope = Some(self.scopes.pop().unwrap());
                }

                if let Some(error) = error {
                    self.errors.push(Error::new(error, item.syntax().text_range().into()));
                    None
                } else {
                    Some(())
                }
            })
        })
    }

    fn lower_block(&mut self, block: Block) -> Option<(Type, Hir)> {
        self.lower_expr(block.expr()?)
    }

    fn lower_expr(&mut self, expr: Expr) -> Option<(Type, Hir)> {
        match expr {
            Expr::Literal(expr) => self.lower_literal_expr(expr),
            Expr::Binary(expr) => self.lower_binary_expr(expr),
            Expr::Prefix(_expr) => todo!(),
            Expr::Call(expr) => self.lower_call_expr(expr),
            Expr::If(expr) => self.lower_if_expr(expr),
        }
    }

    fn lower_literal_expr(&mut self, expr: LiteralExpr) -> Option<(Type, Hir)> {
        let token = expr.token()?;
        match token.kind() {
            SyntaxKind::Integer => self.lower_integer_expr(token),
            SyntaxKind::String => self.lower_string_expr(token),
            SyntaxKind::Ident => self.lower_ident_expr(token),
            _ => None,
        }
    }

    fn lower_integer_expr(&mut self, token: SyntaxToken) -> Option<(Type, Hir)> {
        let text = token.text();
        match text.parse() {
            Ok(value) => Some((Type::Int, Hir::Int(value))),
            Err(error) => {
                self.errors.push(Error::new(
                    format!("invalid integer literal `{text}` ({error})"),
                    token.text_range().into(),
                ));
                None
            }
        }
    }

    fn lower_string_expr(&mut self, token: SyntaxToken) -> Option<(Type, Hir)> {
        let text = token.text();
        let mut chars = text.chars();
        if chars.next() != Some('"') || chars.last() != Some('"') {
            return None;
        }
        Some((Type::String, Hir::String(text.to_string())))
    }

    fn lower_ident_expr(&mut self, token: SyntaxToken) -> Option<(Type, Hir)> {
        let name = token.text();

        let Some(symbol_id) = self.resolve_name(name) else {
            self.errors.push(Error::new(
                format!("undefined variable `{name}`"),
                token.text_range().into(),
            ));
            return None;
        };

        self.scope_mut().mark_used(symbol_id);

        let hir = Hir::Symbol(symbol_id);

        Some(match self.db.symbol(symbol_id) {
            Symbol::Variable { ty, .. } => (ty.clone(), hir),
            Symbol::Parameter { ty, .. } => (ty.clone(), hir),
            Symbol::Function {
                param_types,
                return_type,
                ..
            } => (
                Type::Function {
                    param_types: param_types.clone(),
                    return_type: Box::new(return_type.clone()),
                },
                hir,
            ),
            Symbol::Builtin { .. } => {
                self.errors.push(Error::new(
                    format!("builtin function `{name}` cannot be used as a value"),
                    token.text_range().into(),
                ));
                return None;
            }
        })
    }

    fn lower_binary_expr(&mut self, expr: BinaryExpr) -> Option<(Type, Hir)> {
        let (op, token) = expr.op()?;

        let lhs = self.lower_expr(expr.lhs()?)?;
        let rhs = self.lower_expr(expr.rhs()?)?;

        if lhs.0 != Type::Int || rhs.0 != Type::Int {
            self.errors.push(Error::new(
                format!(
                    "cannot apply operator `{op}` to values of type `{}` and `{}`",
                    lhs.0, rhs.0
                ),
                token.text_range().into(),
            ));
            return None;
        }

        let hir = Hir::BinOp {
            op,
            lhs: Box::new(lhs.1),
            rhs: Box::new(rhs.1),
        };

        Some((Type::Int, hir))
    }

    fn lower_call_expr(&mut self, expr: CallExpr) -> Option<(Type, Hir)> {
        let target = self.lower_expr(expr.target()?)?;

        let args = expr
            .args()
            .into_iter()
            .map(|arg| self.lower_expr(arg))
            .collect::<Option<Vec<_>>>()?;

        let Type::Function {
            param_types,
            return_type,
        } = target.0
        else {
            self.errors.push(Error::new(
                format!(
                    "expected callable function, found value of type `{}`",
                    target.0
                ),
                expr.syntax().text_range().into(),
            ));
            return None;
        };

        if args.len() != param_types.len() {
            self.errors.push(Error::new(
                format!(
                    "expected {} arguments, but was given {}",
                    param_types.len(),
                    args.len()
                ),
                expr.syntax().text_range().into(),
            ));
            return None;
        }

        let mut arg_hirs = Vec::new();

        for (i, arg) in args.iter().enumerate() {
            let ty = &param_types[i];

            if !arg.0.is_assignable_to(ty) {
                self.errors.push(Error::new(
                    format!("expected argument of type `{}`, but found `{}`", ty, arg.0),
                    expr.syntax().text_range().into(),
                ));
                return None;
            }

            arg_hirs.push(arg.1.clone());
        }

        Some((
            return_type.as_ref().clone(),
            Hir::Call {
                value: Box::new(target.1),
                arguments: arg_hirs,
            },
        ))
    }

    fn lower_if_expr(&mut self, expr: IfExpr) -> Option<(Type, Hir)> {
        let condition = self.lower_expr(expr.condition()?)?;
        let then_block = self.lower_block(expr.then_block()?)?;
        let else_block = self.lower_block(expr.else_block()?)?;

        if then_block.0 != else_block.0 {
            self.errors.push(Error::new(
                format!(
                    "then branch has type `{}`, but else branch has differing type `{}`",
                    then_block.0, else_block.0
                ),
                expr.syntax().text_range().into(),
            ));
            return None;
        }

        Some((
            then_block.0,
            Hir::If {
                condition: Box::new(condition.1),
                then_branch: Box::new(then_block.1),
                else_branch: Box::new(else_block.1),
            },
        ))
    }

    fn lower_type(&mut self, token: SyntaxToken) -> Option<Type> {
        match self.resolve_type(token.text()) {
            Some(ty) => Some(ty.clone()),
            None => {
                self.errors.push(Error::new(
                    format!("undefined type `{token}`"),
                    token.text_range().into(),
                ));
                None
            }
        }
    }

    fn define_item(&mut self, item: Item) -> Option<SymbolId> {
        match item {
            Item::Fn(item) => self.define_fn_item(item),
        }
    }

    fn define_fn_item(&mut self, item: FnItem) -> Option<SymbolId> {
        let name_token = item.name()?;
        let name = name_token.text().to_string();

        if self.scope().lookup_symbol(&name).is_some() {
            self.errors.push(Error::new(
                format!("there is already a variable named `{name}`"),
                name_token.text_range().into(),
            ));
            return None;
        }

        let mut param_types = Vec::new();
        for param_type in item
            .param_list()
            .map(|list| list.params())
            .unwrap_or_default()
        {
            param_types.push(self.lower_type(param_type.ty()?)?);
        }

        let return_type = self.lower_type(item.return_type()?)?;

        let symbol = self.db.new_symbol(Symbol::Function {
            param_types,
            return_type,
            resolved_body: None,
            scope: None,
        });

        self.scope_mut().define_symbol(name, symbol);

        Some(symbol)
    }

    fn resolve_name(&self, name: &str) -> Option<SymbolId> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.lookup_symbol(name))
    }

    fn resolve_type(&self, name: &str) -> Option<&Type> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.lookup_type(name))
    }

    fn scope(&self) -> &Scope {
        self.scopes.last().unwrap()
    }

    fn scope_mut(&mut self) -> &mut Scope {
        self.scopes.last_mut().unwrap()
    }
}
