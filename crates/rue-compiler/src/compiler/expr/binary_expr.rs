use rowan::TextRange;
use rue_parser::{AstNode, BinaryExpr, BinaryOp, Expr};
use rue_typing::{Comparison, Type, TypeId};

use crate::{
    compiler::Compiler,
    hir::{BinOp, Hir, Op},
    value::{Guard, Value},
    ErrorKind, HirId,
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
            BinaryOp::GreaterThan
            | BinaryOp::LessThan
            | BinaryOp::GreaterThanEquals
            | BinaryOp::LessThanEquals => self.op_comparison(&lhs, rhs, op, text_range),
            BinaryOp::And => self.op_and(lhs, rhs, text_range),
            BinaryOp::Or => self.op_or(&lhs, rhs, text_range),
            BinaryOp::BitwiseAnd => self.op_bitwise_and(lhs, rhs, text_range),
            BinaryOp::BitwiseOr => self.op_bitwise_or(&lhs, rhs, text_range),
            BinaryOp::BitwiseXor => self.op_bitwise_xor(&lhs, rhs, text_range),
            BinaryOp::LeftArithShift => self.op_left_arith_shift(&lhs, rhs, text_range),
            BinaryOp::RightArithShift => self.op_right_arith_shift(&lhs, rhs, text_range),
        }
    }

    fn binary_op(&mut self, op: BinOp, lhs: HirId, rhs: HirId, type_id: TypeId) -> Value {
        Value::new(self.db.alloc_hir(Hir::BinaryOp(op, lhs, rhs)), type_id)
    }

    fn op_add(&mut self, lhs: &Value, rhs: Option<&Expr>, text_range: TextRange) -> Value {
        if matches!(self.ty.get(lhs.type_id), Type::Unknown) {
            if let Some(rhs) = rhs {
                self.compile_expr(rhs, None);
            }
            return self.unknown();
        }

        if self.ty.compare(lhs.type_id, self.ty.std().public_key) == Comparison::Equal {
            return self.add_public_key(lhs.hir_id, rhs, text_range);
        }

        if self.ty.compare(lhs.type_id, self.ty.std().bytes) <= Comparison::Assignable {
            return self.add_bytes(lhs.hir_id, rhs, text_range);
        }

        let rhs = rhs
            .map(|rhs| self.compile_expr(rhs, Some(self.ty.std().int)))
            .unwrap_or_else(|| self.unknown());

        self.type_check(lhs.type_id, self.ty.std().int, text_range);
        self.type_check(rhs.type_id, self.ty.std().int, text_range);
        self.binary_op(BinOp::Add, lhs.hir_id, rhs.hir_id, self.ty.std().int)
    }

    fn add_public_key(&mut self, lhs: HirId, rhs: Option<&Expr>, text_range: TextRange) -> Value {
        let rhs = rhs
            .map(|rhs| self.compile_expr(rhs, Some(self.ty.std().public_key)))
            .unwrap_or_else(|| self.unknown());

        self.type_check(rhs.type_id, self.ty.std().public_key, text_range);
        self.binary_op(BinOp::PointAdd, lhs, rhs.hir_id, self.ty.std().public_key)
    }

    fn add_bytes(&mut self, lhs: HirId, rhs: Option<&Expr>, text_range: TextRange) -> Value {
        let rhs = rhs
            .map(|rhs| self.compile_expr(rhs, Some(self.ty.std().bytes)))
            .unwrap_or_else(|| self.unknown());

        self.type_check(rhs.type_id, self.ty.std().bytes, text_range);
        self.binary_op(BinOp::Concat, lhs, rhs.hir_id, self.ty.std().bytes)
    }

    fn op_subtract(&mut self, lhs: &Value, rhs: Option<&Expr>, text_range: TextRange) -> Value {
        let rhs = rhs
            .map(|rhs| self.compile_expr(rhs, Some(self.ty.std().int)))
            .unwrap_or_else(|| self.unknown());

        self.type_check(lhs.type_id, self.ty.std().int, text_range);
        self.type_check(rhs.type_id, self.ty.std().int, text_range);
        self.binary_op(BinOp::Subtract, lhs.hir_id, rhs.hir_id, self.ty.std().int)
    }

    fn op_multiply(&mut self, lhs: &Value, rhs: Option<&Expr>, text_range: TextRange) -> Value {
        let rhs = rhs
            .map(|rhs| self.compile_expr(rhs, Some(self.ty.std().int)))
            .unwrap_or_else(|| self.unknown());

        self.type_check(lhs.type_id, self.ty.std().int, text_range);
        self.type_check(rhs.type_id, self.ty.std().int, text_range);
        self.binary_op(BinOp::Multiply, lhs.hir_id, rhs.hir_id, self.ty.std().int)
    }

    fn op_divide(&mut self, lhs: &Value, rhs: Option<&Expr>, text_range: TextRange) -> Value {
        let rhs = rhs
            .map(|rhs| self.compile_expr(rhs, Some(self.ty.std().int)))
            .unwrap_or_else(|| self.unknown());

        self.type_check(lhs.type_id, self.ty.std().int, text_range);
        self.type_check(rhs.type_id, self.ty.std().int, text_range);
        self.binary_op(BinOp::Divide, lhs.hir_id, rhs.hir_id, self.ty.std().int)
    }

    fn op_remainder(&mut self, lhs: &Value, rhs: Option<&Expr>, text_range: TextRange) -> Value {
        let rhs = rhs
            .map(|rhs| self.compile_expr(rhs, Some(self.ty.std().int)))
            .unwrap_or_else(|| self.unknown());

        self.type_check(lhs.type_id, self.ty.std().int, text_range);
        self.type_check(rhs.type_id, self.ty.std().int, text_range);
        self.binary_op(BinOp::Remainder, lhs.hir_id, rhs.hir_id, self.ty.std().int)
    }

    fn op_equals(&mut self, lhs: &Value, rhs: Option<&Expr>, text_range: TextRange) -> Value {
        if matches!(self.ty.get(lhs.type_id), Type::Unknown) {
            if let Some(rhs) = rhs {
                self.compile_expr(rhs, None);
            }
            return self.unknown();
        }

        let rhs = rhs
            .map(|rhs| self.compile_expr(rhs, Some(lhs.type_id)))
            .unwrap_or_else(|| self.unknown());

        let mut value = self.binary_op(BinOp::Equals, lhs.hir_id, rhs.hir_id, self.ty.std().bool);

        let mut is_atom = true;

        if self.ty.compare(lhs.type_id, self.ty.std().bytes) > Comparison::Castable {
            self.db.error(
                ErrorKind::NonAtomEquality(self.type_name(lhs.type_id)),
                text_range,
            );
            is_atom = false;
        }

        if self.ty.compare(rhs.type_id, self.ty.std().bytes) > Comparison::Castable {
            self.db.error(
                ErrorKind::NonAtomEquality(self.type_name(rhs.type_id)),
                text_range,
            );
            is_atom = false;
        }

        if self.ty.compare(lhs.type_id, self.ty.std().nil) == Comparison::Equal {
            if let Some(guard_path) = rhs.guard_path {
                let then_type = self.ty.std().nil;
                let else_type = self.ty.difference(rhs.type_id, self.ty.std().nil);
                value
                    .guards
                    .insert(guard_path, Guard::new(Some(then_type), Some(else_type)));
            }
        }

        if self.ty.compare(rhs.type_id, self.ty.std().nil) == Comparison::Equal {
            if let Some(guard_path) = lhs.guard_path.clone() {
                let then_type = self.ty.std().nil;
                let else_type = self.ty.difference(lhs.type_id, self.ty.std().nil);
                value
                    .guards
                    .insert(guard_path, Guard::new(Some(then_type), Some(else_type)));
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
            self.db.alloc_hir(Hir::Op(Op::Not, comparison.hir_id)),
            self.ty.std().bool,
        );

        for (symbol_id, guard) in comparison.guards {
            value.guards.insert(symbol_id, !guard);
        }

        value
    }

    fn op_comparison(
        &mut self,
        lhs: &Value,
        rhs: Option<&Expr>,
        op: BinaryOp,
        text_range: TextRange,
    ) -> Value {
        if self.ty.compare(lhs.type_id, self.ty.std().bytes) <= Comparison::Assignable {
            let op = match op {
                BinaryOp::GreaterThan => BinOp::GreaterThanBytes,
                BinaryOp::LessThan => BinOp::LessThanBytes,
                BinaryOp::GreaterThanEquals => BinOp::GreaterThanBytesEquals,
                BinaryOp::LessThanEquals => BinOp::LessThanBytesEquals,
                _ => unreachable!(),
            };

            let rhs = rhs
                .map(|rhs| self.compile_expr(rhs, Some(self.ty.std().bytes)))
                .unwrap_or_else(|| self.unknown());

            self.type_check(rhs.type_id, self.ty.std().bytes, text_range);
            return self.binary_op(op, lhs.hir_id, rhs.hir_id, self.ty.std().bool);
        }

        let op = match op {
            BinaryOp::GreaterThan => BinOp::GreaterThan,
            BinaryOp::LessThan => BinOp::LessThan,
            BinaryOp::GreaterThanEquals => BinOp::GreaterThanBytes,
            BinaryOp::LessThanEquals => BinOp::LessThanBytes,
            _ => unreachable!(),
        };

        let rhs = rhs
            .map(|rhs| self.compile_expr(rhs, Some(self.ty.std().int)))
            .unwrap_or_else(|| self.unknown());

        self.type_check(lhs.type_id, self.ty.std().int, text_range);
        self.type_check(rhs.type_id, self.ty.std().int, text_range);
        self.binary_op(op, lhs.hir_id, rhs.hir_id, self.ty.std().bool)
    }

    fn op_and(&mut self, lhs: Value, rhs: Option<&Expr>, text_range: TextRange) -> Value {
        let overrides = self.build_overrides(lhs.then_guards());
        self.type_overrides.push(overrides);

        let rhs = rhs
            .map(|rhs| self.compile_expr(rhs, Some(self.ty.std().bool)))
            .unwrap_or_else(|| self.unknown());

        self.type_overrides.pop().unwrap();

        self.type_check(lhs.type_id, self.ty.std().bool, text_range);
        self.type_check(rhs.type_id, self.ty.std().bool, text_range);

        let mut value = self.binary_op(
            BinOp::LogicalAnd,
            lhs.hir_id,
            rhs.hir_id,
            self.ty.std().bool,
        );
        value.guards.extend(
            lhs.guards
                .into_iter()
                .map(|(path, guard)| (path, Guard::new(guard.then_type, None))),
        );
        value.guards.extend(
            rhs.guards
                .into_iter()
                .map(|(path, guard)| (path, Guard::new(guard.then_type, None))),
        );
        value
    }

    fn op_or(&mut self, lhs: &Value, rhs: Option<&Expr>, text_range: TextRange) -> Value {
        let overrides = self.build_overrides(lhs.else_guards());
        self.type_overrides.push(overrides);

        let rhs = rhs
            .map(|rhs| self.compile_expr(rhs, Some(self.ty.std().bool)))
            .unwrap_or_else(|| self.unknown());

        self.type_overrides.pop().unwrap();

        self.type_check(lhs.type_id, self.ty.std().bool, text_range);
        self.type_check(rhs.type_id, self.ty.std().bool, text_range);
        self.binary_op(BinOp::LogicalOr, lhs.hir_id, rhs.hir_id, self.ty.std().bool)
    }

    fn op_bitwise_and(&mut self, lhs: Value, rhs: Option<&Expr>, text_range: TextRange) -> Value {
        if self.ty.compare(lhs.type_id, self.ty.std().bool) <= Comparison::Assignable {
            let rhs = rhs
                .map(|rhs| self.compile_expr(rhs, Some(self.ty.std().bool)))
                .unwrap_or_else(|| self.unknown());

            self.type_check(rhs.type_id, self.ty.std().bool, text_range);

            let mut value = self.binary_op(BinOp::All, lhs.hir_id, rhs.hir_id, self.ty.std().bool);
            value.guards.extend(lhs.guards);
            value.guards.extend(rhs.guards);
            return value;
        }

        let rhs = rhs
            .map(|rhs| self.compile_expr(rhs, Some(self.ty.std().int)))
            .unwrap_or_else(|| self.unknown());

        self.type_check(lhs.type_id, self.ty.std().int, text_range);
        self.type_check(rhs.type_id, self.ty.std().int, text_range);
        self.binary_op(BinOp::BitwiseAnd, lhs.hir_id, rhs.hir_id, self.ty.std().int)
    }

    fn op_bitwise_or(&mut self, lhs: &Value, rhs: Option<&Expr>, text_range: TextRange) -> Value {
        if self.ty.compare(lhs.type_id, self.ty.std().bool) <= Comparison::Assignable {
            let rhs = rhs
                .map(|rhs| self.compile_expr(rhs, Some(self.ty.std().bool)))
                .unwrap_or_else(|| self.unknown());

            self.type_check(rhs.type_id, self.ty.std().bool, text_range);
            return self.binary_op(BinOp::Any, lhs.hir_id, rhs.hir_id, self.ty.std().bool);
        }

        let rhs = rhs
            .map(|rhs| self.compile_expr(rhs, Some(self.ty.std().int)))
            .unwrap_or_else(|| self.unknown());

        self.type_check(lhs.type_id, self.ty.std().int, text_range);
        self.type_check(rhs.type_id, self.ty.std().int, text_range);
        self.binary_op(BinOp::BitwiseOr, lhs.hir_id, rhs.hir_id, self.ty.std().int)
    }

    fn op_bitwise_xor(&mut self, lhs: &Value, rhs: Option<&Expr>, text_range: TextRange) -> Value {
        let rhs = rhs
            .map(|rhs| self.compile_expr(rhs, Some(self.ty.std().int)))
            .unwrap_or_else(|| self.unknown());

        self.type_check(lhs.type_id, self.ty.std().int, text_range);
        self.type_check(rhs.type_id, self.ty.std().int, text_range);
        self.binary_op(BinOp::BitwiseXor, lhs.hir_id, rhs.hir_id, self.ty.std().int)
    }

    fn op_left_arith_shift(
        &mut self,
        lhs: &Value,
        rhs: Option<&Expr>,
        text_range: TextRange,
    ) -> Value {
        let rhs = rhs
            .map(|rhs| self.compile_expr(rhs, Some(self.ty.std().int)))
            .unwrap_or_else(|| self.unknown());

        self.type_check(lhs.type_id, self.ty.std().int, text_range);
        self.type_check(rhs.type_id, self.ty.std().int, text_range);
        self.binary_op(
            BinOp::LeftArithShift,
            lhs.hir_id,
            rhs.hir_id,
            self.ty.std().int,
        )
    }

    fn op_right_arith_shift(
        &mut self,
        lhs: &Value,
        rhs: Option<&Expr>,
        text_range: TextRange,
    ) -> Value {
        let rhs = rhs
            .map(|rhs| self.compile_expr(rhs, Some(self.ty.std().int)))
            .unwrap_or_else(|| self.unknown());

        self.type_check(lhs.type_id, self.ty.std().int, text_range);
        self.type_check(rhs.type_id, self.ty.std().int, text_range);
        self.binary_op(
            BinOp::RightArithShift,
            lhs.hir_id,
            rhs.hir_id,
            self.ty.std().int,
        )
    }
}
