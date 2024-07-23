use std::collections::{HashMap, HashSet};

use id_arena::{Arena, Id};

use crate::{
    check_type, compare_type, difference_type, replace_type, simplify_check, stringify_type,
    substitute_type, Check, CheckError, Comparison, ComparisonContext, Semantics, StandardTypes,
    Type, TypePath,
};

pub type TypeId = Id<Type>;

#[derive(Debug, Clone)]
pub struct TypeSystem {
    arena: Arena<Type>,
    types: StandardTypes,
}

impl Default for TypeSystem {
    fn default() -> Self {
        let mut arena = Arena::new();

        let unknown = arena.alloc(Type::Unknown);
        let never = arena.alloc(Type::Never);
        let atom = arena.alloc(Type::Atom);
        let bytes = arena.alloc(Type::Bytes);
        let bytes32 = arena.alloc(Type::Bytes32);
        let public_key = arena.alloc(Type::PublicKey);
        let int = arena.alloc(Type::Int);
        let bool = arena.alloc(Type::Bool);
        let nil = arena.alloc(Type::Nil);

        let any = arena.alloc(Type::Unknown);
        let pair = arena.alloc(Type::Pair(any, any));
        arena[any] = Type::Union(vec![atom, pair]);

        Self {
            arena,
            types: StandardTypes {
                unknown,
                never,
                any,
                atom,
                bytes,
                bytes32,
                public_key,
                int,
                bool,
                nil,
            },
        }
    }
}

impl TypeSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn standard_types(&self) -> StandardTypes {
        self.types
    }

    pub fn alloc(&mut self, ty: Type) -> TypeId {
        self.arena.alloc(ty)
    }

    pub fn get(&self, id: TypeId) -> &Type {
        match &self.arena[id] {
            Type::Ref(id) => self.get(*id),
            ty => ty,
        }
    }

    pub fn get_mut(&mut self, id: TypeId) -> &mut Type {
        match &self.arena[id] {
            Type::Ref(id) => self.get_mut(*id),
            _ => &mut self.arena[id],
        }
    }

    pub fn stringify_named(&self, type_id: TypeId, names: &HashMap<TypeId, String>) -> String {
        stringify_type(self, type_id, names, &mut HashSet::new())
    }

    pub fn stringify(&self, type_id: TypeId) -> String {
        self.stringify_named(type_id, &HashMap::new())
    }

    pub fn compare(&self, lhs: TypeId, rhs: TypeId) -> Comparison {
        self.compare_with_generics(lhs, rhs, &mut Vec::new(), false)
    }

    pub fn compare_with_generics(
        &self,
        lhs: TypeId,
        rhs: TypeId,
        substitution_stack: &mut Vec<HashMap<TypeId, TypeId>>,
        infer_generics: bool,
    ) -> Comparison {
        let generic_stack_frame = if infer_generics {
            Some(substitution_stack.len() - 1)
        } else {
            None
        };
        let initial_substitution_length = substitution_stack.len();

        compare_type(
            self,
            lhs,
            rhs,
            &mut ComparisonContext {
                visited: HashSet::new(),
                substitution_stack,
                initial_substitution_length,
                generic_stack_frame,
            },
        )
    }

    pub fn substitute(
        &mut self,
        type_id: TypeId,
        mut substitutions: HashMap<TypeId, TypeId>,
        semantics: Semantics,
    ) -> TypeId {
        substitute_type(
            self,
            type_id,
            &mut substitutions,
            semantics,
            &mut HashSet::new(),
        )
    }

    pub fn check(&mut self, lhs: TypeId, rhs: TypeId) -> Result<Check, CheckError> {
        check_type(self, lhs, rhs, &mut HashSet::new()).map(simplify_check)
    }

    pub fn difference(&mut self, std: &StandardTypes, lhs: TypeId, rhs: TypeId) -> TypeId {
        difference_type(self, std, lhs, rhs, &mut HashSet::new())
    }

    pub fn replace(
        &mut self,
        type_id: TypeId,
        replace_type_id: TypeId,
        path: &[TypePath],
    ) -> TypeId {
        replace_type(self, type_id, replace_type_id, path)
    }
}
