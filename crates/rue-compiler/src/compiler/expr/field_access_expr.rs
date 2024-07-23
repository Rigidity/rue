use rue_parser::FieldAccessExpr;

use crate::{
    compiler::Compiler,
    hir::{Hir, Op},
    value::{GuardPathItem, PairType, Rest, Type, Value},
    ErrorKind,
};

impl Compiler<'_> {
    /// Compiles a field access expression, or special properties for certain types.
    pub fn compile_field_access_expr(&mut self, field_access: &FieldAccessExpr) -> Value {
        let Some(old_value) = field_access
            .expr()
            .map(|expr| self.compile_expr(&expr, None))
        else {
            return self.unknown();
        };

        let Some(field_name) = field_access.field() else {
            return self.unknown();
        };

        let mut new_value = match self.db.ty(old_value.type_id).clone() {
            Type::Struct(struct_type) => {
                if let Some((index, _, &field_type)) =
                    struct_type.fields.get_full(field_name.text())
                {
                    let mut type_id = field_type;

                    if index == struct_type.fields.len() - 1 && struct_type.rest == Rest::Optional {
                        type_id = self.ty.alloc(Type::Optional(type_id));
                    }

                    Value::new(
                        self.compile_index(
                            old_value.hir_id,
                            index,
                            index == struct_type.fields.len() - 1 && struct_type.rest != Rest::Nil,
                        ),
                        type_id,
                    )
                    .extend_guard_path(old_value, GuardPathItem::Field(field_name.to_string()))
                } else {
                    self.db.error(
                        ErrorKind::UnknownField(field_name.to_string()),
                        field_name.text_range(),
                    );
                    return self.unknown();
                }
            }
            Type::EnumVariant(variant_type) => {
                let fields = variant_type.fields.unwrap_or_default();

                if let Some((index, _, &field_type)) = fields.get_full(field_name.text()) {
                    let mut type_id = field_type;

                    if index == fields.len() - 1 && variant_type.rest == Rest::Optional {
                        type_id = self.ty.alloc(Type::Optional(type_id));
                    }

                    let fields_hir_id = self.db.alloc_hir(Hir::Op(Op::Rest, old_value.hir_id));

                    Value::new(
                        self.compile_index(
                            fields_hir_id,
                            index,
                            index == fields.len() - 1 && variant_type.rest != Rest::Nil,
                        ),
                        type_id,
                    )
                    .extend_guard_path(old_value, GuardPathItem::Field(field_name.to_string()))
                } else {
                    self.db.error(
                        ErrorKind::UnknownField(field_name.to_string()),
                        field_name.text_range(),
                    );
                    return self.unknown();
                }
            }
            Type::Pair(PairType { first, rest }) => match field_name.text() {
                "first" => Value::new(
                    self.db.alloc_hir(Hir::Op(Op::First, old_value.hir_id)),
                    first,
                )
                .extend_guard_path(old_value, GuardPathItem::First),
                "rest" => Value::new(self.db.alloc_hir(Hir::Op(Op::Rest, old_value.hir_id)), rest)
                    .extend_guard_path(old_value, GuardPathItem::Rest),
                _ => {
                    self.db.error(
                        ErrorKind::InvalidFieldAccess(
                            field_name.to_string(),
                            self.type_name(old_value.type_id),
                        ),
                        field_name.text_range(),
                    );
                    return self.unknown();
                }
            },
            Type::Bytes | Type::Bytes32 if field_name.text() == "length" => Value::new(
                self.db.alloc_hir(Hir::Op(Op::Strlen, old_value.hir_id)),
                self.ty.std().int,
            ),
            _ => {
                self.db.error(
                    ErrorKind::InvalidFieldAccess(
                        field_name.to_string(),
                        self.type_name(old_value.type_id),
                    ),
                    field_name.text_range(),
                );
                return self.unknown();
            }
        };

        if let Some(guard_path) = new_value.guard_path.as_ref() {
            if let Some(type_override) = self.symbol_type(guard_path) {
                new_value.type_id = type_override.type_id;
                new_value.hir_id = self.apply_mutation(new_value.hir_id, type_override.mutation);
            }
        }

        new_value
    }
}
