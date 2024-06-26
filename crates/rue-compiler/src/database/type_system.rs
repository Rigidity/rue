use std::collections::{HashMap, HashSet};

use crate::{
    value::{EnumType, EnumVariantType, FunctionType, PairType, StructType, Type},
    Comparison, Database, TypeId,
};

#[derive(Debug)]
struct ComparisonContext<'a> {
    visited: HashSet<(TypeId, TypeId)>,
    generic_type_stack: &'a mut Vec<HashMap<TypeId, TypeId>>,
    infer_generics: bool,
}

impl<'a> ComparisonContext<'a> {
    fn new(generic_type_stack: &'a mut Vec<HashMap<TypeId, TypeId>>, infer_generics: bool) -> Self {
        Self {
            visited: HashSet::new(),
            generic_type_stack,
            infer_generics,
        }
    }
}

impl Database {
    pub fn substitute_type(
        &mut self,
        type_id: TypeId,
        substitutions: &HashMap<TypeId, TypeId>,
    ) -> TypeId {
        self.substitute_type_visitor(type_id, substitutions, &mut HashSet::new())
    }

    pub fn compare_type(&self, lhs: TypeId, rhs: TypeId) -> Comparison {
        self.compare_type_visitor(
            lhs,
            rhs,
            &mut ComparisonContext::new(&mut Vec::new(), false),
        )
    }

    pub fn compare_type_with_generics(
        &self,
        lhs: TypeId,
        rhs: TypeId,
        generic_type_stack: &mut Vec<HashMap<TypeId, TypeId>>,
    ) -> Comparison {
        self.compare_type_visitor(
            lhs,
            rhs,
            &mut ComparisonContext::new(generic_type_stack, true),
        )
    }

    pub fn is_cyclic(&self, type_id: TypeId) -> bool {
        self.is_cyclic_visitor(type_id, &mut HashSet::new())
    }

    pub fn first_type(&self, type_id: TypeId) -> Option<TypeId> {
        let Type::Pair(pair) = self.ty(type_id) else {
            return None;
        };
        Some(pair.first)
    }

    pub fn rest_type(&self, type_id: TypeId) -> Option<TypeId> {
        let Type::Pair(pair) = self.ty(type_id) else {
            return None;
        };
        Some(pair.rest)
    }

    pub fn non_optional(&mut self, ty: TypeId) -> TypeId {
        match self.ty(ty) {
            Type::Optional(inner) => self.non_optional(*inner),
            _ => ty,
        }
    }

    pub fn unwrap_list(&mut self, ty: TypeId) -> Option<TypeId> {
        match self.ty(ty) {
            Type::List(inner) => Some(*inner),
            _ => None,
        }
    }

    pub fn substitute_type_visitor(
        &mut self,
        type_id: TypeId,
        substitutions: &HashMap<TypeId, TypeId>,
        visited: &mut HashSet<TypeId>,
    ) -> TypeId {
        if let Some(type_id) = substitutions.get(&type_id).copied() {
            return type_id;
        }

        if !visited.insert(type_id) {
            return type_id;
        }

        match self.ty(type_id).clone() {
            Type::Alias(..) => unreachable!(),
            Type::Pair(pair) => {
                let new_pair = PairType {
                    first: self.substitute_type_visitor(pair.first, substitutions, visited),
                    rest: self.substitute_type_visitor(pair.rest, substitutions, visited),
                };

                if new_pair == pair {
                    type_id
                } else {
                    self.alloc_type(Type::Pair(new_pair))
                }
            }
            Type::Enum(enum_type) => {
                let new_enum = EnumType {
                    has_fields: enum_type.has_fields,
                    variants: enum_type
                        .variants
                        .iter()
                        .map(|(k, v)| {
                            (
                                k.clone(),
                                self.substitute_type_visitor(*v, substitutions, visited),
                            )
                        })
                        .collect(),
                };

                if new_enum == enum_type {
                    type_id
                } else {
                    self.alloc_type(Type::Enum(new_enum))
                }
            }
            Type::EnumVariant(enum_variant) => {
                let new_variant = EnumVariantType {
                    enum_type: enum_variant.enum_type,
                    original_type_id: enum_variant.original_type_id,
                    fields: enum_variant.fields.as_ref().map(|fields| {
                        fields
                            .iter()
                            .map(|(k, v)| {
                                (
                                    k.clone(),
                                    self.substitute_type_visitor(*v, substitutions, visited),
                                )
                            })
                            .collect()
                    }),
                    rest: enum_variant.rest,
                    discriminant: enum_variant.discriminant,
                };

                if new_variant == enum_variant {
                    type_id
                } else {
                    self.alloc_type(Type::EnumVariant(new_variant))
                }
            }
            Type::List(inner) => {
                let new_inner = self.substitute_type_visitor(inner, substitutions, visited);

                if new_inner == inner {
                    type_id
                } else {
                    self.alloc_type(Type::List(new_inner))
                }
            }
            Type::Struct(struct_type) => {
                let new_struct = StructType {
                    original_type_id: struct_type.original_type_id,
                    fields: struct_type
                        .fields
                        .iter()
                        .map(|(k, v)| {
                            (
                                k.clone(),
                                self.substitute_type_visitor(*v, substitutions, visited),
                            )
                        })
                        .collect(),
                    rest: struct_type.rest,
                };

                if new_struct == struct_type {
                    type_id
                } else {
                    self.alloc_type(Type::Struct(new_struct))
                }
            }
            Type::Function(function) => {
                let new_function = FunctionType {
                    param_types: function
                        .param_types
                        .iter()
                        .map(|ty| self.substitute_type_visitor(*ty, substitutions, visited))
                        .collect(),
                    rest: function.rest,
                    return_type: self.substitute_type_visitor(
                        function.return_type,
                        substitutions,
                        visited,
                    ),
                    generic_types: function
                        .generic_types
                        .iter()
                        .map(|ty| self.substitute_type_visitor(*ty, substitutions, visited))
                        .collect(),
                };

                if new_function == function {
                    type_id
                } else {
                    self.alloc_type(Type::Function(new_function))
                }
            }
            Type::Optional(inner) | Type::PossiblyUndefined(inner) => {
                let new_inner = self.substitute_type_visitor(inner, substitutions, visited);

                if new_inner == inner {
                    type_id
                } else {
                    self.alloc_type(Type::Optional(inner))
                }
            }
            Type::Unknown
            | Type::Generic
            | Type::Any
            | Type::Bool
            | Type::Bytes
            | Type::Bytes32
            | Type::Int
            | Type::Nil
            | Type::PublicKey => type_id,
        }
    }

    #[allow(clippy::match_same_arms, clippy::too_many_lines)]
    fn compare_type_visitor(
        &self,
        lhs: TypeId,
        rhs: TypeId,
        ctx: &mut ComparisonContext<'_>,
    ) -> Comparison {
        let key = (lhs, rhs);
        if lhs == rhs || !ctx.visited.insert(key) {
            return Comparison::Equal;
        }

        let comparison = match (self.ty(lhs), self.ty(rhs)) {
            // Aliases are already resolved at this point.
            (Type::Alias(..), _) | (_, Type::Alias(..)) => unreachable!(),

            // We need to infer `Generic` types, and return `Unrelated` if incompatible.
            (_, Type::Generic) => {
                let mut found = None;

                for generics in ctx.generic_type_stack.iter().rev() {
                    if let Some(&generic) = generics.get(&rhs) {
                        found = Some(generic);
                    }
                }

                if let Some(found) = found {
                    self.compare_type_visitor(lhs, found, ctx)
                } else if ctx.infer_generics {
                    ctx.generic_type_stack.last_mut().unwrap().insert(rhs, lhs);
                    Comparison::Assignable
                } else {
                    Comparison::Unrelated
                }
            }

            // We should have already given a diagnostic for `Unknown`.
            // So we will go ahead and pretend they are equal to everything.
            (Type::Unknown, _) | (_, Type::Unknown) => Comparison::Equal,

            // Possibly undefined is a special type.
            (Type::PossiblyUndefined(..), _) | (_, Type::PossiblyUndefined(..)) => {
                Comparison::Unrelated
            }

            // These are of course equal atomic types.
            (Type::Any, Type::Any) => Comparison::Equal,
            (Type::Int, Type::Int) => Comparison::Equal,
            (Type::Bool, Type::Bool) => Comparison::Equal,
            (Type::Bytes, Type::Bytes) => Comparison::Equal,
            (Type::Bytes32, Type::Bytes32) => Comparison::Equal,
            (Type::PublicKey, Type::PublicKey) => Comparison::Equal,
            (Type::Nil, Type::Nil) => Comparison::Equal,

            // We should treat `Bytes32` as a subtype of `Bytes` and therefore equal.
            // `PublicKey` however, can have different meaning, so is not equal.
            // `Bytes` is also not equal to `Bytes32` since it's unsized.
            (Type::Bytes32, Type::Bytes) => Comparison::Equal,

            // You can cast `Any` to anything, but it's not implicit.
            // So many languages make `Any` implicit and it makes it hard to debug.
            (Type::Any, _) => Comparison::Castable,

            // However, anything can be assigned to `Any` implicitly.
            (_, Type::Any) => Comparison::Assignable,

            // `Generic` types are unrelated to everything else except their specific instance.
            // The only exception is the `Any` type above.
            (Type::Generic, _) => Comparison::Unrelated,

            // You have to explicitly convert between other atom types.
            // This is because the compiled output can change depending on the type.
            (Type::Int, Type::Bytes) => Comparison::Castable,
            (Type::Bool, Type::Bytes) => Comparison::Castable,
            (Type::Bool, Type::Int) => Comparison::Castable,
            (Type::Nil, Type::Int) => Comparison::Castable,
            (Type::Nil, Type::Bool) => Comparison::Castable,
            (Type::PublicKey, Type::Bytes) => Comparison::Castable,
            (Type::PublicKey, Type::Int) => Comparison::Castable,
            (Type::Bytes, Type::Int) => Comparison::Castable,
            (Type::Bytes32, Type::Int) => Comparison::Castable,

            // Let's allow assigning `Nil` to `Bytes` for ease of use.
            // The only alternative without needing a cast is empty strings.
            (Type::Nil, Type::Bytes) => Comparison::Assignable,

            // Size changing conversions are not possible without a type guard.
            (Type::Bytes, Type::Bytes32) => Comparison::Superset,
            (Type::Bytes, Type::PublicKey) => Comparison::Superset,
            (Type::Bytes, Type::Nil) => Comparison::Superset,
            (Type::Bytes, Type::Bool) => Comparison::Superset,
            (Type::Int, Type::Bytes32) => Comparison::Superset,
            (Type::Int, Type::PublicKey) => Comparison::Superset,
            (Type::Int, Type::Nil) => Comparison::Superset,
            (Type::Int, Type::Bool) => Comparison::Superset,
            (Type::Bool, Type::PublicKey) => Comparison::Unrelated,
            (Type::Bool, Type::Bytes32) => Comparison::Unrelated,
            (Type::Bool, Type::Nil) => Comparison::Unrelated,
            (Type::Nil, Type::Bytes32) => Comparison::Unrelated,
            (Type::Nil, Type::PublicKey) => Comparison::Unrelated,
            (Type::PublicKey, Type::Bytes32) => Comparison::Unrelated,
            (Type::PublicKey, Type::Bool) => Comparison::Unrelated,
            (Type::PublicKey, Type::Nil) => Comparison::Unrelated,
            (Type::Bytes32, Type::PublicKey) => Comparison::Unrelated,
            (Type::Bytes32, Type::Bool) => Comparison::Unrelated,
            (Type::Bytes32, Type::Nil) => Comparison::Unrelated,

            // These are the variants of the `Optional` type.
            (Type::Nil, Type::Optional(..)) => Comparison::Assignable,
            (Type::Optional(lhs), Type::Optional(rhs)) => {
                self.compare_type_visitor(*lhs, *rhs, ctx)
            }
            (_, Type::Optional(inner)) => self.compare_type_visitor(lhs, *inner, ctx),
            (Type::Optional(inner), Type::Bytes) => {
                Comparison::Castable & self.compare_type_visitor(*inner, rhs, ctx)
            }

            // TODO: Unions would make this more generalized and useful.
            // I should add unions back.
            (Type::Optional(_inner), _) => Comparison::Unrelated,

            // Compare both sides of a `Pair`.
            (Type::Pair(lhs), Type::Pair(rhs)) => {
                let first = self.compare_type_visitor(lhs.first, rhs.first, ctx);
                let rest = self.compare_type_visitor(lhs.rest, rhs.rest, ctx);
                first & rest
            }

            // A `Pair` is a valid `List` if its first type is the same as the list's inner type
            // and the rest is also a valid `List` of the same type.
            // However, it's not considered equal but rather assignable.
            (Type::Pair(pair), Type::List(inner)) => {
                let inner = self.compare_type_visitor(pair.first, *inner, ctx);
                let rest = self.compare_type_visitor(pair.rest, rhs, ctx);
                Comparison::Assignable & inner & rest
            }

            // A `List` is not a valid pair since `Nil` is also a valid list.
            // It's a `Superset` only if the opposite comparison is not `Unrelated`.
            (Type::List(..), Type::Pair(..)) => {
                Comparison::Superset & self.compare_type_visitor(rhs, lhs, ctx)
            }

            // Nothing else can be assigned to or from a `Pair`.
            (Type::Pair(..), _) | (_, Type::Pair(..)) => Comparison::Unrelated,

            // A `List` just compares with the inner type of another `List`.
            (Type::List(lhs), Type::List(rhs)) => self.compare_type_visitor(*lhs, *rhs, ctx),

            // `Nil` is a valid list.
            (Type::Nil, Type::List(..)) => Comparison::Assignable,
            (Type::List(..), Type::Nil) => Comparison::Superset,

            // `List` is not compatible with atoms.
            (Type::Bytes, Type::List(..)) => Comparison::Unrelated,
            (Type::Bytes32, Type::List(..)) => Comparison::Unrelated,
            (Type::PublicKey, Type::List(..)) => Comparison::Unrelated,
            (Type::Int, Type::List(..)) => Comparison::Unrelated,
            (Type::Bool, Type::List(..)) => Comparison::Unrelated,
            (Type::List(..), Type::Bytes) => Comparison::Unrelated,
            (Type::List(..), Type::Bytes32) => Comparison::Unrelated,
            (Type::List(..), Type::PublicKey) => Comparison::Unrelated,
            (Type::List(..), Type::Int) => Comparison::Unrelated,
            (Type::List(..), Type::Bool) => Comparison::Unrelated,

            // A `Struct` is castable to others only if the fields are identical.
            (Type::Struct(lhs), Type::Struct(rhs)) => {
                if lhs.fields.len() == rhs.fields.len() {
                    let mut result = Comparison::Castable;
                    for i in 0..lhs.fields.len() {
                        result &= self.compare_type_visitor(lhs.fields[i], rhs.fields[i], ctx);
                    }
                    result
                } else {
                    Comparison::Unrelated
                }
            }
            (Type::Struct(..), _) | (_, Type::Struct(..)) => Comparison::Unrelated,

            // Enum variants are assignable only to their respective enum.
            (Type::EnumVariant(variant_type), Type::Enum(..)) => {
                if variant_type.enum_type == rhs {
                    Comparison::Assignable
                } else {
                    Comparison::Unrelated
                }
            }

            // But not the other way around.
            (Type::Enum(..), Type::EnumVariant(variant_type)) => {
                if variant_type.enum_type == lhs {
                    Comparison::Superset
                } else {
                    Comparison::Unrelated
                }
            }

            // Enums and their variants are not assignable to anything except themselves.
            (Type::Enum(..), _) | (_, Type::Enum(..)) => Comparison::Unrelated,
            (Type::EnumVariant(..), _) | (_, Type::EnumVariant(..)) => Comparison::Unrelated,

            // The parameters and return types of functions are checked.
            // This can be made more flexible later.
            (Type::Function(lhs), Type::Function(rhs)) => {
                if lhs.param_types.len() != rhs.param_types.len() || lhs.rest != rhs.rest {
                    Comparison::Unrelated
                } else {
                    let mut result =
                        self.compare_type_visitor(lhs.return_type, rhs.return_type, ctx);

                    for i in 0..lhs.param_types.len() {
                        result &=
                            self.compare_type_visitor(lhs.param_types[i], rhs.param_types[i], ctx);
                    }

                    result
                }
            }

            // There is likely nothing that should relate to a function.
            (Type::Function(..), _) | (_, Type::Function(..)) => Comparison::Unrelated,
        };

        ctx.visited.remove(&key);
        comparison
    }

    fn is_cyclic_visitor(&self, ty: TypeId, visited_aliases: &mut HashSet<TypeId>) -> bool {
        match self.ty_raw(ty).clone() {
            Type::Pair(pair) => {
                self.is_cyclic_visitor(pair.first, visited_aliases)
                    || self.is_cyclic_visitor(pair.rest, visited_aliases)
            }
            Type::Alias(alias) => {
                if !visited_aliases.insert(alias) {
                    return true;
                }
                self.is_cyclic_visitor(alias, visited_aliases)
            }
            Type::List(..)
            | Type::Struct(..)
            | Type::Enum(..)
            | Type::EnumVariant(..)
            | Type::Function(..)
            | Type::Unknown
            | Type::Generic
            | Type::Nil
            | Type::Any
            | Type::Int
            | Type::Bool
            | Type::Bytes
            | Type::Bytes32
            | Type::PublicKey => false,
            Type::Optional(ty) | Type::PossiblyUndefined(ty) => {
                self.is_cyclic_visitor(ty, visited_aliases)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use indexmap::IndexMap;

    use crate::{
        compiler::{builtins, Builtins},
        value::{EnumType, EnumVariantType, FunctionType, PairType, Rest, StructType},
    };

    use super::*;

    fn setup() -> (Database, Builtins) {
        let mut db = Database::new();
        let ty = builtins(&mut db);
        (db, ty)
    }

    fn fields(items: &[TypeId]) -> IndexMap<String, TypeId> {
        let mut fields = IndexMap::new();
        for (i, item) in items.iter().enumerate() {
            fields.insert(format!("field_{i}"), *item);
        }
        fields
    }

    #[test]
    fn test_substitution() {
        let (mut db, ty) = setup();

        let a = db.alloc_type(Type::Generic);
        let b = db.alloc_type(Type::Generic);

        let a_list = db.alloc_type(Type::List(a));

        let function = db.alloc_type(Type::Function(FunctionType {
            param_types: vec![a_list],
            rest: Rest::Nil,
            return_type: b,
            generic_types: vec![a, b],
        }));

        let int_list = db.alloc_type(Type::List(ty.int));

        let substitutions = [(a, ty.int), (b, ty.bool)].into_iter().collect();
        let substituted = db.substitute_type(function, &substitutions);

        let expected = db.alloc_type(Type::Function(FunctionType {
            param_types: vec![int_list],
            rest: Rest::Nil,
            return_type: ty.bool,
            generic_types: vec![a, b],
        }));

        assert_eq!(db.compare_type(substituted, expected), Comparison::Equal);
    }

    #[test]
    fn test_alias_resolution() {
        let (mut db, ty) = setup();

        let int_alias = db.alloc_type(Type::Alias(ty.int));
        assert_eq!(db.compare_type(int_alias, ty.int), Comparison::Equal);
        assert_eq!(db.compare_type(ty.int, int_alias), Comparison::Equal);
        assert_eq!(db.compare_type(int_alias, int_alias), Comparison::Equal);

        let double_alias = db.alloc_type(Type::Alias(int_alias));
        assert_eq!(db.compare_type(double_alias, ty.int), Comparison::Equal);
        assert_eq!(db.compare_type(ty.int, double_alias), Comparison::Equal);
        assert_eq!(
            db.compare_type(double_alias, double_alias),
            Comparison::Equal
        );
        assert_eq!(db.compare_type(double_alias, int_alias), Comparison::Equal);
        assert_eq!(db.compare_type(int_alias, double_alias), Comparison::Equal);
    }

    #[test]
    fn test_generic_type() {
        let (mut db, ty) = setup();
        let a = db.alloc_type(Type::Generic);
        let b = db.alloc_type(Type::Generic);
        assert_eq!(db.compare_type(a, b), Comparison::Unrelated);
        assert_eq!(db.compare_type(a, ty.int), Comparison::Unrelated);
        assert_eq!(db.compare_type(ty.int, a), Comparison::Unrelated);
        assert_eq!(db.compare_type(a, a), Comparison::Equal);
    }

    #[test]
    fn test_any_type() {
        let (db, ty) = setup();
        assert_eq!(db.compare_type(ty.any, ty.int), Comparison::Castable);
        assert_eq!(db.compare_type(ty.any, ty.any), Comparison::Equal);
        assert_eq!(db.compare_type(ty.int, ty.any), Comparison::Assignable);
    }

    #[test]
    fn test_unknown_type() {
        let (db, ty) = setup();
        assert_eq!(db.compare_type(ty.any, ty.unknown), Comparison::Equal);
        assert_eq!(db.compare_type(ty.int, ty.unknown), Comparison::Equal);
        assert_eq!(db.compare_type(ty.unknown, ty.int), Comparison::Equal);
        assert_eq!(db.compare_type(ty.unknown, ty.any), Comparison::Equal);
    }

    #[test]
    fn test_atom_types() {
        let (db, ty) = setup();
        assert_eq!(db.compare_type(ty.bytes, ty.bytes32), Comparison::Superset);
        assert_eq!(db.compare_type(ty.bytes32, ty.bytes), Comparison::Equal);
        assert_eq!(
            db.compare_type(ty.bytes, ty.public_key),
            Comparison::Superset
        );
        assert_eq!(
            db.compare_type(ty.public_key, ty.bytes),
            Comparison::Castable
        );
        assert_eq!(db.compare_type(ty.bytes, ty.int), Comparison::Castable);
        assert_eq!(db.compare_type(ty.int, ty.bytes), Comparison::Castable);
        assert_eq!(db.compare_type(ty.int, ty.int), Comparison::Equal);
    }

    #[test]
    fn test_nil_type() {
        let (mut db, ty) = setup();

        let list = db.alloc_type(Type::List(ty.int));
        assert_eq!(db.compare_type(ty.nil, ty.int), Comparison::Castable);
        assert_eq!(db.compare_type(ty.nil, ty.bool), Comparison::Castable);
        assert_eq!(db.compare_type(ty.nil, ty.bytes), Comparison::Assignable);
        assert_eq!(db.compare_type(ty.nil, ty.bytes32), Comparison::Unrelated);
        assert_eq!(
            db.compare_type(ty.nil, ty.public_key),
            Comparison::Unrelated
        );
        assert_eq!(db.compare_type(ty.nil, list), Comparison::Assignable);
    }

    #[test]
    fn test_pair_type() {
        let (mut db, ty) = setup();

        let int_pair = db.alloc_type(Type::Pair(PairType {
            first: ty.int,
            rest: ty.int,
        }));
        assert_eq!(db.compare_type(int_pair, int_pair), Comparison::Equal);

        let bytes_pair = db.alloc_type(Type::Pair(PairType {
            first: ty.bytes,
            rest: ty.bytes,
        }));
        assert_eq!(db.compare_type(int_pair, bytes_pair), Comparison::Castable);
        assert_eq!(db.compare_type(bytes_pair, int_pair), Comparison::Castable);

        let bytes32_pair = db.alloc_type(Type::Pair(PairType {
            first: ty.bytes32,
            rest: ty.bytes32,
        }));
        assert_eq!(
            db.compare_type(bytes_pair, bytes32_pair),
            Comparison::Superset
        );
        assert_eq!(db.compare_type(bytes32_pair, bytes_pair), Comparison::Equal);

        let bytes32_bytes = db.alloc_type(Type::Pair(PairType {
            first: ty.bytes32,
            rest: ty.bytes,
        }));
        assert_eq!(
            db.compare_type(bytes_pair, bytes32_bytes),
            Comparison::Superset
        );
        assert_eq!(
            db.compare_type(bytes32_bytes, bytes_pair),
            Comparison::Equal
        );
    }

    #[test]
    fn test_list_types() {
        let (mut db, ty) = setup();

        let int_list = db.alloc_type(Type::List(ty.int));
        assert_eq!(db.compare_type(int_list, int_list), Comparison::Equal);

        let pair_list = db.alloc_type(Type::Pair(PairType {
            first: ty.int,
            rest: int_list,
        }));
        assert_eq!(db.compare_type(pair_list, int_list), Comparison::Assignable);
        assert_eq!(db.compare_type(int_list, pair_list), Comparison::Superset);

        let pair_nil = db.alloc_type(Type::Pair(PairType {
            first: ty.int,
            rest: ty.nil,
        }));
        assert_eq!(db.compare_type(pair_nil, int_list), Comparison::Assignable);

        let pair_unrelated_first = db.alloc_type(Type::Pair(PairType {
            first: pair_list,
            rest: ty.nil,
        }));
        assert_eq!(
            db.compare_type(pair_unrelated_first, int_list),
            Comparison::Unrelated
        );

        let pair_unrelated_rest = db.alloc_type(Type::Pair(PairType {
            first: ty.int,
            rest: ty.int,
        }));
        assert_eq!(
            db.compare_type(pair_unrelated_rest, int_list),
            Comparison::Unrelated
        );
    }

    #[test]
    fn test_struct_types() {
        let (mut db, ty) = setup();

        let two_ints = db.alloc_type(Type::Unknown);
        *db.ty_mut(two_ints) = Type::Struct(StructType {
            original_type_id: two_ints,
            fields: fields(&[ty.int, ty.int]),
            rest: Rest::Nil,
        });
        assert_eq!(db.compare_type(two_ints, two_ints), Comparison::Equal);

        let one_int = db.alloc_type(Type::Unknown);
        *db.ty_mut(one_int) = Type::Struct(StructType {
            original_type_id: one_int,
            fields: fields(&[ty.int]),
            rest: Rest::Nil,
        });
        assert_eq!(db.compare_type(one_int, two_ints), Comparison::Unrelated);

        let empty_struct = db.alloc_type(Type::Unknown);
        *db.ty_mut(empty_struct) = Type::Struct(StructType {
            original_type_id: empty_struct,
            fields: fields(&[]),
            rest: Rest::Nil,
        });
        assert_eq!(
            db.compare_type(empty_struct, empty_struct),
            Comparison::Equal
        );
    }

    #[test]
    fn test_enum_types() {
        let (mut db, ty) = setup();

        let enum_type = db.alloc_type(Type::Unknown);

        let variant = db.alloc_type(Type::Unknown);
        *db.ty_mut(variant) = Type::EnumVariant(EnumVariantType {
            enum_type,
            original_type_id: variant,
            fields: Some(fields(&[ty.int])),
            discriminant: ty.unknown_hir,
            rest: Rest::Nil,
        });

        *db.ty_mut(enum_type) = Type::Enum(EnumType {
            has_fields: true,
            variants: fields(&[variant]),
        });

        assert_eq!(db.compare_type(enum_type, variant), Comparison::Superset);
        assert_eq!(db.compare_type(variant, enum_type), Comparison::Assignable);
    }

    #[test]
    fn test_function_types() {
        let (mut db, ty) = setup();

        let int_to_bool = db.alloc_type(Type::Function(FunctionType {
            param_types: vec![ty.int],
            rest: Rest::Nil,
            return_type: ty.bool,
            generic_types: Vec::new(),
        }));
        assert_eq!(db.compare_type(int_to_bool, int_to_bool), Comparison::Equal);

        let int_to_int = db.alloc_type(Type::Function(FunctionType {
            param_types: vec![ty.int],
            rest: Rest::Nil,
            return_type: ty.int,
            generic_types: Vec::new(),
        }));
        assert_eq!(
            db.compare_type(int_to_bool, int_to_int),
            Comparison::Castable
        );
        assert_eq!(
            db.compare_type(int_to_int, int_to_bool),
            Comparison::Superset
        );

        let int_list = db.alloc_type(Type::List(ty.int));
        let int_list_to_bool = db.alloc_type(Type::Function(FunctionType {
            param_types: vec![int_list],
            rest: Rest::Nil,
            return_type: ty.bool,
            generic_types: Vec::new(),
        }));
        assert_eq!(
            db.compare_type(int_to_bool, int_list_to_bool),
            Comparison::Unrelated
        );
        assert_eq!(
            db.compare_type(int_list_to_bool, int_to_bool),
            Comparison::Unrelated
        );
    }
}
