use rue_parser::{
    BinaryExpr, Block, Expr, FunctionCall, FunctionItem, FunctionType, IfExpr, LiteralExpr,
    LiteralType, Root, SyntaxKind, SyntaxToken,
};

use crate::{
    database::{Database, ScopeId, SymbolId, TypeId},
    scope::Scope,
    symbol::Symbol,
    ty::{Type, Typed},
    value::Value,
};

pub struct LowerOutput {
    pub db: Database,
    pub errors: Vec<String>,
    pub main_scope_id: ScopeId,
}

pub struct Lowerer {
    db: Database,
    scope_stack: Vec<ScopeId>,
    errors: Vec<String>,
}

impl Lowerer {
    pub fn new() -> Self {
        Self {
            db: Database::default(),
            scope_stack: Vec::new(),
            errors: Vec::new(),
        }
    }

    pub fn compile_root(mut self, root: Root) -> LowerOutput {
        let scope_id = self.db.alloc_scope(Scope::default());
        self.scope_stack.push(scope_id);

        let symbol_ids: Vec<SymbolId> = root
            .function_items()
            .into_iter()
            .map(|function| self.declare_function(function))
            .collect();

        for (i, function) in root.function_items().into_iter().enumerate() {
            self.compile_function(function, symbol_ids[i]);
        }

        LowerOutput {
            db: self.db,
            errors: self.errors,
            main_scope_id: scope_id,
        }
    }

    fn declare_function(&mut self, function: FunctionItem) -> SymbolId {
        let mut scope = Scope::default();

        let ret_ty = function
            .return_ty()
            .map(|ty| self.compile_ty(ty))
            .unwrap_or_else(|| {
                self.error("expected return type".to_string());
                self.db.alloc_type(Type::Unknown)
            });

        let mut param_types = Vec::new();

        for param in function
            .param_list()
            .map(|list| list.params())
            .unwrap_or_default()
        {
            let Some(name) = param.name() else {
                self.error("expected parameter name".to_string());
                continue;
            };

            let ty = param.ty().map(|ty| self.compile_ty(ty)).unwrap_or_else(|| {
                self.error("expected parameter type".to_string());
                self.db.alloc_type(Type::Unknown)
            });

            param_types.push(ty);

            let symbol_id = self.db.alloc_symbol(Symbol::Parameter { ty });
            scope.define_symbol(name.to_string(), symbol_id);
        }

        let scope_id = self.db.alloc_scope(scope);

        let symbol_id = self.db.alloc_symbol(Symbol::Function {
            scope_id,
            value: Value::Nil,
            ret_type: ret_ty,
            param_types,
        });

        if let Some(name) = function.name() {
            self.scope_mut().define_symbol(name.to_string(), symbol_id);
        }

        symbol_id
    }

    fn compile_function(&mut self, function: FunctionItem, symbol_id: SymbolId) {
        if let Some(body) = function.body() {
            let body_scope_id = match self.db.symbol(symbol_id) {
                Symbol::Function { scope_id, .. } => *scope_id,
                _ => unreachable!(),
            };

            self.scope_stack.push(body_scope_id);
            let ret = self.compile_block(body);
            self.scope_stack.pop().expect("function not in scope stack");

            match &mut self.db.symbol_mut(symbol_id) {
                Symbol::Function { value, .. } => {
                    *value = ret.value;
                }
                _ => unreachable!(),
            };
        }
    }

    fn compile_block(&mut self, block: Block) -> Typed {
        let Some(expr) = block.expr() else {
            self.error("expected expr".to_string());
            return Typed {
                value: Value::Nil,
                ty: self.db.alloc_type(Type::Unknown),
            };
        };
        self.compile_expr(expr)
    }

    fn compile_expr(&mut self, expr: Expr) -> Typed {
        match expr {
            Expr::LiteralExpr(literal) => self.compile_literal_expr(literal),
            Expr::BinaryExpr(binary) => self.compile_binary_expr(binary),
            Expr::IfExpr(if_expr) => self.compile_if_expr(if_expr),
            Expr::FunctionCall(call) => self.compile_function_call(call),
        }
    }

    fn compile_binary_expr(&mut self, binary: BinaryExpr) -> Typed {
        let Some(lhs) = binary.lhs() else {
            self.error("expected lhs".to_string());
            return Typed {
                value: Value::Nil,
                ty: self.db.alloc_type(Type::Unknown),
            };
        };

        let Some(rhs) = binary.rhs() else {
            self.error("expected rhs".to_string());
            return Typed {
                value: Value::Nil,
                ty: self.db.alloc_type(Type::Unknown),
            };
        };

        let Some(op) = binary.op() else {
            self.error("expected op".to_string());
            return Typed {
                value: Value::Nil,
                ty: self.db.alloc_type(Type::Unknown),
            };
        };

        let lhs = self.compile_expr(lhs);
        let rhs = self.compile_expr(rhs);

        if !self.is_assignable_to(self.db.ty(lhs.ty), &Type::Int) {
            self.error(format!("expected int, found {:?}", self.db.ty(lhs.ty)));
        }

        if !self.is_assignable_to(self.db.ty(rhs.ty), &Type::Int) {
            self.error(format!("expected int, found {:?}", self.db.ty(rhs.ty)));
        }

        let lhs = lhs.value;
        let rhs = rhs.value;

        let mut ty = Type::Int;

        let value = match op.kind() {
            SyntaxKind::Plus => Value::Add(vec![lhs, rhs]),
            SyntaxKind::Minus => Value::Subtract(vec![lhs, rhs]),
            SyntaxKind::Star => Value::Multiply(vec![lhs, rhs]),
            SyntaxKind::Slash => Value::Divide(vec![lhs, rhs]),
            SyntaxKind::LessThan => {
                ty = Type::Bool;
                Value::LessThan(Box::new(lhs), Box::new(rhs))
            }
            SyntaxKind::GreaterThan => {
                ty = Type::Bool;
                Value::GreaterThan(Box::new(lhs), Box::new(rhs))
            }
            _ => {
                self.error(format!("unexpected binary operator `{}`", op.text()));
                Value::Nil
            }
        };

        Typed {
            value,
            ty: self.db.alloc_type(ty),
        }
    }

    fn compile_literal_expr(&mut self, literal: LiteralExpr) -> Typed {
        let Some(value) = literal.value() else {
            self.error("expected value".to_string());
            return Typed {
                value: Value::Nil,
                ty: self.db.alloc_type(Type::Unknown),
            };
        };

        match value.kind() {
            SyntaxKind::Int => self.compile_int(value),
            SyntaxKind::Ident => self.compile_ident(value),
            _ => {
                self.error(format!("unexpected literal: {:?}", value));
                Typed {
                    value: Value::Nil,
                    ty: self.db.alloc_type(Type::Unknown),
                }
            }
        }
    }

    fn compile_if_expr(&mut self, if_expr: IfExpr) -> Typed {
        let Some(condition) = if_expr.condition() else {
            self.error("expected condition".to_string());
            return Typed {
                value: Value::Nil,
                ty: self.db.alloc_type(Type::Unknown),
            };
        };

        let Some(then_block) = if_expr.then_block() else {
            self.error("expected then block".to_string());
            return Typed {
                value: Value::Nil,
                ty: self.db.alloc_type(Type::Unknown),
            };
        };

        let Some(else_block) = if_expr.else_block() else {
            self.error("expected else block".to_string());
            return Typed {
                value: Value::Nil,
                ty: self.db.alloc_type(Type::Unknown),
            };
        };

        let condition = self.compile_expr(condition);
        let then_block = self.compile_block(then_block);
        let else_block = self.compile_block(else_block);

        if !self.is_assignable_to(self.db.ty(condition.ty), &Type::Bool) {
            self.error(format!(
                "expected bool, found {:?}",
                self.db.ty(condition.ty)
            ));
        }

        if !self.is_assignable_to(self.db.ty(then_block.ty), self.db.ty(else_block.ty)) {
            self.error(format!(
                "expected {:?}, found {:?}",
                self.db.ty(then_block.ty),
                self.db.ty(else_block.ty)
            ));
        }

        Typed {
            value: Value::If {
                condition: Box::new(condition.value),
                then_block: Box::new(then_block.value),
                else_block: Box::new(else_block.value),
            },
            ty: then_block.ty,
        }
    }

    fn compile_int(&mut self, int: SyntaxToken) -> Typed {
        Typed {
            value: Value::Int(int.text().parse().expect("failed to parse into BigInt")),
            ty: self.db.alloc_type(Type::Int),
        }
    }

    fn compile_ident(&mut self, ident: SyntaxToken) -> Typed {
        let name = ident.text();

        let Some(symbol_id) = self
            .scope_stack
            .iter()
            .rev()
            .find_map(|&scope_id| self.db.scope(scope_id).get_symbol(name))
        else {
            self.error(format!("undefined symbol: {}", name));
            return Typed {
                value: Value::Nil,
                ty: self.db.alloc_type(Type::Unknown),
            };
        };

        self.scope_mut().use_symbol(symbol_id);

        match self.db.symbol(symbol_id) {
            Symbol::Function {
                param_types,
                ret_type,
                ..
            } => Typed {
                value: Value::Reference(symbol_id),
                ty: self.db.alloc_type(Type::Function {
                    params: param_types.clone(),
                    ret: *ret_type,
                }),
            },
            Symbol::Parameter { ty } => Typed {
                value: Value::Reference(symbol_id),
                ty: *ty,
            },
        }
    }

    fn compile_function_call(&mut self, call: FunctionCall) -> Typed {
        let Some(callee) = call.callee() else {
            self.error("expected callee".to_string());
            return Typed {
                value: Value::Nil,
                ty: self.db.alloc_type(Type::Unknown),
            };
        };

        let Some(args) = call.args() else {
            self.error("expected args".to_string());
            return Typed {
                value: Value::Nil,
                ty: self.db.alloc_type(Type::Unknown),
            };
        };

        let callee = self.compile_expr(callee);
        let args: Vec<Typed> = args
            .exprs()
            .into_iter()
            .map(|arg| self.compile_expr(arg))
            .collect();

        let arg_types: Vec<TypeId> = args.iter().map(|arg| arg.ty).collect();
        let arg_values: Vec<Value> = args.iter().map(|arg| arg.value.clone()).collect();

        match self.db.ty(callee.ty).clone() {
            Type::Function { params, ret } => {
                if params.len() != arg_types.len() {
                    self.error(format!(
                        "expected {} arguments, found {}",
                        params.len(),
                        arg_types.len()
                    ));
                    return Typed {
                        value: Value::Nil,
                        ty: self.db.alloc_type(Type::Unknown),
                    };
                }

                for (param, arg) in params.clone().into_iter().zip(arg_types.iter()) {
                    let error = if !self.is_assignable_to(self.db.ty(param), self.db.ty(*arg)) {
                        Some(format!(
                            "expected argument of type {:?}, found {:?}",
                            self.db.ty(param),
                            self.db.ty(*arg)
                        ))
                    } else {
                        None
                    };

                    if let Some(error) = error {
                        self.error(error);
                    }
                }

                Typed {
                    value: Value::FunctionCall {
                        callee: Box::new(callee.value),
                        args: arg_values,
                    },
                    ty: ret,
                }
            }
            Type::Unknown => Typed {
                value: Value::FunctionCall {
                    callee: Box::new(callee.value),
                    args: arg_values,
                },
                ty: self.db.alloc_type(Type::Unknown),
            },
            ty => {
                self.error(format!("expected function, found {ty:?}"));
                Typed {
                    value: Value::Nil,
                    ty: self.db.alloc_type(Type::Unknown),
                }
            }
        }
    }

    fn compile_ty(&mut self, ty: rue_parser::Type) -> TypeId {
        match ty {
            rue_parser::Type::LiteralType(literal) => self.compile_literal_ty(literal),
            rue_parser::Type::FunctionType(function) => self.compile_function_ty(function),
        }
    }

    fn compile_literal_ty(&mut self, literal: LiteralType) -> TypeId {
        let Some(value) = literal.value() else {
            self.error("expected value".to_string());
            return self.db.alloc_type(Type::Unknown);
        };

        match value.text() {
            "Int" => self.db.alloc_type(Type::Int),
            "Bool" => self.db.alloc_type(Type::Bool),
            _ => {
                self.error(format!("unexpected type: {}", value.text()));
                self.db.alloc_type(Type::Unknown)
            }
        }
    }

    fn compile_function_ty(&mut self, function: FunctionType) -> TypeId {
        let params = function
            .params()
            .map(|params| params.types())
            .unwrap_or_default()
            .into_iter()
            .map(|ty| self.compile_ty(ty))
            .collect();

        let ret = function
            .ret()
            .map(|ty| self.compile_ty(ty))
            .unwrap_or_else(|| {
                self.error("expected return type".to_string());
                self.db.alloc_type(Type::Unknown)
            });

        self.db.alloc_type(Type::Function { params, ret })
    }

    fn is_assignable_to(&self, a: &Type, b: &Type) -> bool {
        match (a, b) {
            (Type::Unknown, _) | (_, Type::Unknown) => true,
            (Type::Int, Type::Int) => true,
            (Type::Bool, Type::Bool) => true,
            (
                Type::Function {
                    params: params_a,
                    ret: ret_a,
                },
                Type::Function {
                    params: params_b,
                    ret: ret_b,
                },
            ) => {
                if params_a.len() != params_b.len() {
                    return false;
                }

                for (&a, &b) in params_a.iter().zip(params_b.iter()) {
                    if !self.is_assignable_to(self.db.ty(a), self.db.ty(b)) {
                        return false;
                    }
                }

                self.is_assignable_to(self.db.ty(*ret_a), self.db.ty(*ret_b))
            }
            _ => false,
        }
    }

    fn scope_mut(&mut self) -> &mut Scope {
        self.db
            .scope_mut(self.scope_stack.last().copied().expect("no scope found"))
    }

    fn error(&mut self, message: String) {
        self.errors.push(message);
    }
}