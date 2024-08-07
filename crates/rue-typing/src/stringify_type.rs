use crate::{HashMap, HashSet};

use crate::{Callable, Enum, Struct, Type, TypeId, TypeSystem, Variant};

pub(crate) fn stringify_type(
    types: &TypeSystem,
    type_id: TypeId,
    names: &HashMap<TypeId, String>,
    visited: &mut HashSet<TypeId>,
) -> String {
    if let Some(name) = names.get(&type_id) {
        return name.clone();
    }

    if !visited.insert(type_id) {
        return "{recursive}".to_string();
    }

    let result = match types.get(type_id) {
        Type::Ref(..) => unreachable!(),
        Type::Unknown => "{unknown}".to_string(),
        Type::Generic => format!("{{{}}}", type_id.index()),
        Type::Never => "Never".to_string(),
        Type::Any => "Any".to_string(),
        Type::Bytes => "Bytes".to_string(),
        Type::Bytes32 => "Bytes32".to_string(),
        Type::PublicKey => "PublicKey".to_string(),
        Type::Int => "Int".to_string(),
        Type::True => "True".to_string(),
        Type::False => "False".to_string(),
        Type::Nil => "Nil".to_string(),
        Type::Value(value) => format!("{value}"),
        Type::Pair(first, rest) => {
            let first = stringify_type(types, *first, names, visited);
            let rest = stringify_type(types, *rest, names, visited);
            format!("({first}, {rest})")
        }
        Type::Union(items) => {
            let mut result = String::new();

            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    result.push_str(" | ");
                }
                result.push_str(&stringify_type(types, *item, names, visited));
            }

            result
        }
        Type::Lazy(lazy) => {
            let name = stringify_type(types, lazy.type_id, names, visited);
            let mut generics = "<".to_string();

            for (index, (_, generic)) in lazy.substitutions.iter().enumerate() {
                if index > 0 {
                    generics.push_str(", ");
                }
                generics.push_str(&stringify_type(types, *generic, names, visited));
            }

            generics.push('>');
            name + &generics
        }
        Type::Alias(alias) => stringify_type(types, alias.type_id, names, visited),
        Type::Struct(Struct {
            type_id,
            original_type_id,
            ..
        })
        | Type::Variant(Variant {
            type_id,
            original_type_id,
            ..
        }) => {
            if type_id == original_type_id {
                stringify_type(types, *type_id, names, visited)
            } else {
                stringify_type(types, *original_type_id, names, visited)
            }
        }
        Type::Enum(Enum { type_id, .. }) => stringify_type(types, *type_id, names, visited),
        Type::Callable(Callable {
            parameters,
            return_type,
            ..
        }) => {
            let mut result = "fun(".to_string();
            result.push_str(&stringify_type(types, *parameters, names, visited));
            result.push_str(") -> ");
            result.push_str(&stringify_type(types, *return_type, names, visited));
            result
        }
    };

    visited.remove(&type_id);

    result
}

#[cfg(test)]
mod tests {
    use indexmap::indexmap;

    use crate::alloc_callable;

    use super::*;

    #[test]
    fn stringify_atoms() {
        let db = TypeSystem::new();
        let types = db.std();

        assert_eq!(db.stringify(types.unknown), "{unknown}");
        assert_eq!(db.stringify(types.never), "Never");
        assert_eq!(db.stringify(types.bytes), "Bytes");
        assert_eq!(db.stringify(types.bytes32), "Bytes32");
        assert_eq!(db.stringify(types.public_key), "PublicKey");
        assert_eq!(db.stringify(types.int), "Int");
        assert_eq!(db.stringify(types.bool), "Bool");
        assert_eq!(db.stringify(types.nil), "Nil");
        assert_eq!(db.stringify(types.any), "Any");
    }

    #[test]
    fn stringify_named() {
        let db = TypeSystem::new();
        let types = db.std();

        let mut names = HashMap::new();
        names.insert(types.any, "CustomAny".to_string());

        assert_eq!(db.stringify_named(types.any, names), "CustomAny");
    }

    #[test]
    fn test_stringify_callable() {
        let mut db = TypeSystem::new();
        let types = db.std();

        let callable = alloc_callable(
            &mut db,
            &indexmap! {
                "a".to_string() => types.int,
                "b".to_string() => types.bytes,
            },
            types.bool,
            true,
        );

        assert_eq!(
            db.stringify_named(callable, HashMap::new()),
            "fun((Int, (Bytes, Nil))) -> Bool"
        );
    }
}
