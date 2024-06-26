use rowan::TextRange;
use rue_parser::{AstNode, BinaryExpr, BinaryOp, Expr};

use crate::{
    compiler::Compiler,
    hir::{BinOp, Hir},
    value::{Guard, Value},
    ErrorKind, HirId, TypeId,
};

impl Compiler<'_> {
    pub fn compile_binary_expr(&mut self, binary: &BinaryExpr) -> Value {
        let Some(op) = binary.op() else {
            return self.unknown();
        };

        let text_range = binary.syntax().text_range();

        let lhs = binary
            .lhs()
            .map(|lhs| self.compile_expr(&lhs, None))
            .unwrap_or_else(|| self.unknown());

        let rhs_expr = binary.rhs();
        let rhs = rhs_expr.as_ref();

        match op {
            BinaryOp::Add => self.op_add(&lhs, rhs, text_range),
            BinaryOp::Subtract => self.op_subtract(&lhs, rhs, text_range),
            BinaryOp::Multiply => self.op_multiply(&lhs, rhs, text_range),
            BinaryOp::Divide => self.op_divide(&lhs, rhs, text_range),
            BinaryOp::Remainder => self.op_remainder(&lhs, rhs, text_range),
            BinaryOp::Equals => self.op_equals(&lhs, rhs, text_range),
            BinaryOp::NotEquals => self.op_not_equals(&lhs, rhs, text_range),
            BinaryOp::GreaterThan => self.op_greater_than(&lhs, rhs, text_range),
            BinaryOp::LessThan => self.op_less_than(&lhs, rhs, text_range),
            BinaryOp::GreaterThanEquals => self.op_greater_than_equals(&lhs, rhs, text_range),
            BinaryOp::LessThanEquals => self.op_less_than_equals(&lhs, rhs, text_range),
            BinaryOp::And => self.op_and(lhs, rhs, text_range),
            BinaryOp::Or => self.op_or(&lhs, rhs, text_range),
        }
    }

    fn binary_op(&mut self, op: BinOp, lhs: HirId, rhs: HirId, type_id: TypeId) -> Value {
        Value::new(self.db.alloc_hir(Hir::BinaryOp { op, lhs, rhs }), type_id)
    }

    fn op_add(&mut self, lhs: &Value, rhs: Option<&Expr>, text_range: TextRange) -> Value {
        if self
            .db
            .compare_type(lhs.type_id, self.builtins.public_key)
            .is_equal()
        {
            return self.add_public_key(lhs.hir_id, rhs, text_range);
        }

        if self
            .db
            .compare_type(lhs.type_id, self.builtins.bytes)
            .is_equal()
        {
            return self.add_bytes(lhs.hir_id, rhs, text_range);
        }

        let rhs = rhs
            .map(|rhs| self.compile_expr(rhs, Some(self.builtins.int)))
            .unwrap_or_else(|| self.unknown());

        self.type_check(lhs.type_id, self.builtins.int, text_range);
        self.type_check(rhs.type_id, self.builtins.int, text_range);
        self.binary_op(BinOp::Add, lhs.hir_id, rhs.hir_id, self.builtins.int)
    }

    fn add_public_key(&mut self, lhs: HirId, rhs: Option<&Expr>, text_range: TextRange) -> Value {
        let rhs = rhs
            .map(|rhs| self.compile_expr(rhs, Some(self.builtins.public_key)))
            .unwrap_or_else(|| self.unknown());

        self.type_check(rhs.type_id, self.builtins.public_key, text_range);
        self.binary_op(BinOp::PointAdd, lhs, rhs.hir_id, self.builtins.public_key)
    }

    fn add_bytes(&mut self, lhs: HirId, rhs: Option<&Expr>, text_range: TextRange) -> Value {
        let rhs = rhs
            .map(|rhs| self.compile_expr(rhs, Some(self.builtins.bytes)))
            .unwrap_or_else(|| self.unknown());

        self.type_check(rhs.type_id, self.builtins.bytes, text_range);
        self.binary_op(BinOp::Concat, lhs, rhs.hir_id, self.builtins.bytes)
    }

    fn op_subtract(&mut self, lhs: &Value, rhs: Option<&Expr>, text_range: TextRange) -> Value {
        let rhs = rhs
            .map(|rhs| self.compile_expr(rhs, Some(self.builtins.int)))
            .unwrap_or_else(|| self.unknown());

        self.type_check(lhs.type_id, self.builtins.int, text_range);
        self.type_check(rhs.type_id, self.builtins.int, text_range);
        self.binary_op(BinOp::Subtract, lhs.hir_id, rhs.hir_id, self.builtins.int)
    }

    fn op_multiply(&mut self, lhs: &Value, rhs: Option<&Expr>, text_range: TextRange) -> Value {
        let rhs = rhs
            .map(|rhs| self.compile_expr(rhs, Some(self.builtins.int)))
            .unwrap_or_else(|| self.unknown());

        self.type_check(lhs.type_id, self.builtins.int, text_range);
        self.type_check(rhs.type_id, self.builtins.int, text_range);
        self.binary_op(BinOp::Multiply, lhs.hir_id, rhs.hir_id, self.builtins.int)
    }

    fn op_divide(&mut self, lhs: &Value, rhs: Option<&Expr>, text_range: TextRange) -> Value {
        let rhs = rhs
            .map(|rhs| self.compile_expr(rhs, Some(self.builtins.int)))
            .unwrap_or_else(|| self.unknown());

        self.type_check(lhs.type_id, self.builtins.int, text_range);
        self.type_check(rhs.type_id, self.builtins.int, text_range);
        self.binary_op(BinOp::Divide, lhs.hir_id, rhs.hir_id, self.builtins.int)
    }

    fn op_remainder(&mut self, lhs: &Value, rhs: Option<&Expr>, text_range: TextRange) -> Value {
        let rhs = rhs
            .map(|rhs| self.compile_expr(rhs, Some(self.builtins.int)))
            .unwrap_or_else(|| self.unknown());

        self.type_check(lhs.type_id, self.builtins.int, text_range);
        self.type_check(rhs.type_id, self.builtins.int, text_range);
        self.binary_op(BinOp::Remainder, lhs.hir_id, rhs.hir_id, self.builtins.int)
    }

    fn op_equals(&mut self, lhs: &Value, rhs: Option<&Expr>, text_range: TextRange) -> Value {
        let rhs = rhs
            .map(|rhs| self.compile_expr(rhs, Some(self.builtins.bytes)))
            .unwrap_or_else(|| self.unknown());

        let mut value = self.binary_op(BinOp::Equals, lhs.hir_id, rhs.hir_id, self.builtins.bool);

        let mut is_atom = true;

        if !self
            .db
            .compare_type(lhs.type_id, self.builtins.bytes)
            .is_castable()
        {
            self.db.error(
                ErrorKind::NonAtomEquality(self.type_name(lhs.type_id)),
                text_range,
            );
            is_atom = false;
        }

        if !self
            .db
            .compare_type(rhs.type_id, self.builtins.bytes)
            .is_castable()
        {
            self.db.error(
                ErrorKind::NonAtomEquality(self.type_name(rhs.type_id)),
                text_range,
            );
            is_atom = false;
        }

        if self
            .db
            .compare_type(lhs.type_id, self.builtins.nil)
            .is_equal()
        {
            if let Some(guard_path) = rhs.guard_path {
                let then_type = self.builtins.nil;
                let else_type = self.db.non_optional(rhs.type_id);
                value
                    .guards
                    .insert(guard_path, Guard::new(then_type, else_type));
            }
        }

        if self
            .db
            .compare_type(rhs.type_id, self.builtins.nil)
            .is_equal()
        {
            if let Some(guard_path) = lhs.guard_path.clone() {
                let then_type = self.builtins.nil;
                let else_type = self.db.non_optional(lhs.type_id);
                value
                    .guards
                    .insert(guard_path, Guard::new(then_type, else_type));
            }
        }

        if is_atom {
            self.type_check(rhs.type_id, lhs.type_id, text_range);
        }

        value
    }

    fn op_not_equals(&mut self, lhs: &Value, rhs: Option<&Expr>, text_range: TextRange) -> Value {
        let comparison = self.op_equals(lhs, rhs, text_range);

        let mut value = Value::new(
            self.db.alloc_hir(Hir::Not(comparison.hir_id)),
            self.builtins.bool,
        );

        for (symbol_id, guard) in comparison.guards {
            value.guards.insert(symbol_id, !guard);
        }

        value
    }

    fn op_greater_than(&mut self, lhs: &Value, rhs: Option<&Expr>, text_range: TextRange) -> Value {
        let rhs = rhs
            .map(|rhs| self.compile_expr(rhs, Some(self.builtins.int)))
            .unwrap_or_else(|| self.unknown());

        self.type_check(lhs.type_id, self.builtins.int, text_range);
        self.type_check(rhs.type_id, self.builtins.int, text_range);
        self.binary_op(
            BinOp::GreaterThan,
            lhs.hir_id,
            rhs.hir_id,
            self.builtins.bool,
        )
    }

    fn op_less_than(&mut self, lhs: &Value, rhs: Option<&Expr>, text_range: TextRange) -> Value {
        let rhs = rhs
            .map(|rhs| self.compile_expr(rhs, Some(self.builtins.int)))
            .unwrap_or_else(|| self.unknown());

        self.type_check(lhs.type_id, self.builtins.int, text_range);
        self.type_check(rhs.type_id, self.builtins.int, text_range);
        self.binary_op(BinOp::LessThan, lhs.hir_id, rhs.hir_id, self.builtins.bool)
    }

    fn op_greater_than_equals(
        &mut self,
        lhs: &Value,
        rhs: Option<&Expr>,
        text_range: TextRange,
    ) -> Value {
        let rhs = rhs
            .map(|rhs| self.compile_expr(rhs, Some(self.builtins.int)))
            .unwrap_or_else(|| self.unknown());

        self.type_check(lhs.type_id, self.builtins.int, text_range);
        self.type_check(rhs.type_id, self.builtins.int, text_range);
        self.binary_op(
            BinOp::GreaterThanEquals,
            lhs.hir_id,
            rhs.hir_id,
            self.builtins.bool,
        )
    }

    fn op_less_than_equals(
        &mut self,
        lhs: &Value,
        rhs: Option<&Expr>,
        text_range: TextRange,
    ) -> Value {
        let rhs = rhs
            .map(|rhs| self.compile_expr(rhs, Some(self.builtins.int)))
            .unwrap_or_else(|| self.unknown());

        self.type_check(lhs.type_id, self.builtins.int, text_range);
        self.type_check(rhs.type_id, self.builtins.int, text_range);
        self.binary_op(
            BinOp::LessThanEquals,
            lhs.hir_id,
            rhs.hir_id,
            self.builtins.bool,
        )
    }

    fn op_and(&mut self, lhs: Value, rhs: Option<&Expr>, text_range: TextRange) -> Value {
        self.type_guard_stack.push(lhs.then_guards());

        let rhs = rhs
            .map(|rhs| self.compile_expr(rhs, Some(self.builtins.bool)))
            .unwrap_or_else(|| self.unknown());

        self.type_guard_stack.pop().unwrap();

        self.type_check(lhs.type_id, self.builtins.bool, text_range);
        self.type_check(rhs.type_id, self.builtins.bool, text_range);

        let mut value = self.binary_op(
            BinOp::LogicalAnd,
            lhs.hir_id,
            rhs.hir_id,
            self.builtins.bool,
        );
        value.guards.extend(lhs.guards);
        value.guards.extend(rhs.guards);
        value
    }

    fn op_or(&mut self, lhs: &Value, rhs: Option<&Expr>, text_range: TextRange) -> Value {
        self.type_guard_stack.push(lhs.then_guards());

        let rhs = rhs
            .map(|rhs| self.compile_expr(rhs, Some(self.builtins.bool)))
            .unwrap_or_else(|| self.unknown());

        self.type_guard_stack.pop().unwrap();

        self.type_check(lhs.type_id, self.builtins.bool, text_range);
        self.type_check(rhs.type_id, self.builtins.bool, text_range);
        self.binary_op(BinOp::LogicalOr, lhs.hir_id, rhs.hir_id, self.builtins.bool)
    }
}
