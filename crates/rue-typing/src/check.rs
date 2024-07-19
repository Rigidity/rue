use std::{
    collections::{HashSet, VecDeque},
    fmt,
    hash::BuildHasher,
};

use crate::{Type, TypeId, TypePath, TypeSystem};

#[derive(Debug, Clone, Copy)]
pub enum CheckError {
    Recursive(TypeId, TypeId),
    Impossible(TypeId, TypeId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Check {
    None,
    IsPair,
    IsAtom,
    IsBool,
    IsNil,
    Length(usize),
    And(Vec<Check>),
    Or(Vec<Check>),
    If(Box<Check>, Box<Check>, Box<Check>),
    Pair(Box<Check>, Box<Check>),
}

impl fmt::Display for Check {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_check(self, f, &mut Vec::new())
    }
}

/// Returns [`None`] for recursive checks.
pub(crate) fn check_type<S>(
    types: &TypeSystem,
    lhs: TypeId,
    rhs: TypeId,
    visited: &mut HashSet<(TypeId, TypeId), S>,
) -> Result<Check, CheckError>
where
    S: BuildHasher,
{
    if !visited.insert((lhs, rhs)) {
        return Err(CheckError::Recursive(lhs, rhs));
    }

    let check = match (types.get(lhs), types.get(rhs)) {
        (Type::Ref(..), _) | (_, Type::Ref(..)) => unreachable!(),

        (Type::Unknown, _) | (_, Type::Unknown) => Check::None,

        (Type::Never, _) => Check::None,
        (_, Type::Never) => return Err(CheckError::Impossible(lhs, rhs)),

        (Type::Bytes, Type::Bytes) => Check::None,
        (Type::Bytes32, Type::Bytes32) => Check::None,
        (Type::PublicKey, Type::PublicKey) => Check::None,
        (Type::Int, Type::Int) => Check::None,
        (Type::Bool, Type::Bool) => Check::None,
        (Type::Nil, Type::Nil) => Check::None,

        (Type::Bytes32, Type::Bytes) => Check::None,
        (Type::PublicKey, Type::Bytes) => Check::None,
        (Type::Int, Type::Bytes) => Check::None,
        (Type::Bool, Type::Bytes) => Check::None,
        (Type::Nil, Type::Bytes) => Check::None,

        (Type::Bytes32, Type::Int) => Check::None,
        (Type::PublicKey, Type::Int) => Check::None,
        (Type::Bytes, Type::Int) => Check::None,
        (Type::Bool, Type::Int) => Check::None,
        (Type::Nil, Type::Int) => Check::None,

        (Type::Nil, Type::Bool) => Check::None,

        (Type::Bytes, Type::Bool) => Check::IsBool,
        (Type::Bytes, Type::Nil) => Check::IsNil,
        (Type::Bytes, Type::PublicKey) => Check::Length(48),
        (Type::Bytes, Type::Bytes32) => Check::Length(32),

        (Type::Int, Type::Bool) => Check::IsBool,
        (Type::Int, Type::Nil) => Check::IsNil,
        (Type::Int, Type::PublicKey) => Check::Length(48),
        (Type::Int, Type::Bytes32) => Check::Length(32),

        (Type::Bool, Type::Nil) => Check::IsNil,

        (_, Type::Union(items)) => {
            let mut result = Vec::new();
            for item in items.clone() {
                result.push(check_type(types, lhs, item, visited)?);
            }
            Check::Or(result)
        }

        (Type::Union(items), _) => check_union_against_rhs(types, lhs, items, rhs, visited)?,

        (Type::PublicKey, Type::Bytes32) => return Err(CheckError::Impossible(lhs, rhs)),
        (Type::Bytes32, Type::PublicKey) => return Err(CheckError::Impossible(lhs, rhs)),
        (Type::Nil, Type::PublicKey) => return Err(CheckError::Impossible(lhs, rhs)),
        (Type::Nil, Type::Bytes32) => return Err(CheckError::Impossible(lhs, rhs)),
        (Type::PublicKey, Type::Nil) => return Err(CheckError::Impossible(lhs, rhs)),
        (Type::Bytes32, Type::Nil) => return Err(CheckError::Impossible(lhs, rhs)),
        (Type::Bool, Type::PublicKey) => return Err(CheckError::Impossible(lhs, rhs)),
        (Type::Bool, Type::Bytes32) => return Err(CheckError::Impossible(lhs, rhs)),
        (Type::PublicKey, Type::Bool) => return Err(CheckError::Impossible(lhs, rhs)),
        (Type::Bytes32, Type::Bool) => return Err(CheckError::Impossible(lhs, rhs)),

        (Type::Bytes, Type::Pair(..)) => return Err(CheckError::Impossible(lhs, rhs)),
        (Type::Bytes32, Type::Pair(..)) => return Err(CheckError::Impossible(lhs, rhs)),
        (Type::PublicKey, Type::Pair(..)) => return Err(CheckError::Impossible(lhs, rhs)),
        (Type::Int, Type::Pair(..)) => return Err(CheckError::Impossible(lhs, rhs)),
        (Type::Bool, Type::Pair(..)) => return Err(CheckError::Impossible(lhs, rhs)),
        (Type::Nil, Type::Pair(..)) => return Err(CheckError::Impossible(lhs, rhs)),

        (Type::Pair(..), Type::Bytes) => return Err(CheckError::Impossible(lhs, rhs)),
        (Type::Pair(..), Type::Bytes32) => return Err(CheckError::Impossible(lhs, rhs)),
        (Type::Pair(..), Type::PublicKey) => return Err(CheckError::Impossible(lhs, rhs)),
        (Type::Pair(..), Type::Int) => return Err(CheckError::Impossible(lhs, rhs)),
        (Type::Pair(..), Type::Bool) => return Err(CheckError::Impossible(lhs, rhs)),
        (Type::Pair(..), Type::Nil) => return Err(CheckError::Impossible(lhs, rhs)),

        (Type::Pair(lhs_first, lhs_rest), Type::Pair(rhs_first, rhs_rest)) => {
            let (lhs_first, lhs_rest) = (*lhs_first, *lhs_rest);
            let (rhs_first, rhs_rest) = (*rhs_first, *rhs_rest);
            let first = check_type(types, lhs_first, rhs_first, visited)?;
            let rest = check_type(types, lhs_rest, rhs_rest, visited)?;
            Check::Pair(Box::new(first), Box::new(rest))
        }
    };

    visited.remove(&(lhs, rhs));

    Ok(check)
}

fn check_union_against_rhs<S>(
    types: &TypeSystem,
    original_type_id: TypeId,
    items: &[TypeId],
    rhs: TypeId,
    visited: &mut HashSet<(TypeId, TypeId), S>,
) -> Result<Check, CheckError>
where
    S: BuildHasher,
{
    let mut atom_count = 0;
    let mut bool_count = 0;
    let mut nil_count = 0;
    let mut bytes32_count = 0;
    let mut public_key_count = 0;
    let mut pairs = Vec::new();

    let mut items: VecDeque<_> = items.iter().copied().collect::<VecDeque<_>>();
    let mut length = 0;

    while !items.is_empty() {
        let item = items.remove(0).unwrap();
        length += 1;

        if !visited.insert((item, rhs)) {
            return Err(CheckError::Recursive(item, rhs));
        }

        match types.get(item) {
            Type::Ref(..) => unreachable!(),
            Type::Union(child_items) => {
                items.extend(child_items);
            }
            Type::Unknown => {}
            Type::Never => {
                length -= 1;
            }
            Type::Bytes | Type::Int => {
                atom_count += 1;
            }
            Type::Bytes32 => {
                atom_count += 1;
                bytes32_count += 1;
            }
            Type::PublicKey => {
                atom_count += 1;
                public_key_count += 1;
            }
            Type::Bool => {
                atom_count += 1;
                bool_count += 1;
            }
            Type::Nil => {
                atom_count += 1;
                nil_count += 1;
                bool_count += 1;
            }
            Type::Pair(first, rest) => {
                pairs.push((*first, *rest));
            }
        }

        visited.remove(&(item, rhs));
    }

    let always_atom = atom_count == length;
    let always_pair = pairs.len() == length;
    let always_bool = bool_count == length;
    let always_nil = nil_count == length;
    let always_bytes32 = bytes32_count == length;
    let always_public_key = public_key_count == length;

    Ok(match types.get(rhs) {
        Type::Unknown => Check::None,
        Type::Never => return Err(CheckError::Impossible(original_type_id, rhs)),
        Type::Ref(..) => unreachable!(),
        Type::Union(..) => unreachable!(),
        Type::Bytes if always_atom => Check::None,
        Type::Int if always_atom => Check::None,
        Type::Bool if always_bool => Check::None,
        Type::Nil if always_nil => Check::None,
        Type::Bytes32 if always_bytes32 => Check::None,
        Type::PublicKey if always_public_key => Check::None,
        Type::Bytes32 if always_atom => Check::Length(32),
        Type::PublicKey if always_atom => Check::Length(48),
        Type::Bool if always_atom => Check::IsBool,
        Type::Nil if always_atom => Check::IsNil,
        Type::Bytes => Check::IsAtom,
        Type::Int => Check::IsAtom,
        Type::Bytes32 => Check::And(vec![Check::IsAtom, Check::Length(32)]),
        Type::PublicKey => Check::And(vec![Check::IsAtom, Check::Length(48)]),
        Type::Bool => Check::And(vec![Check::IsAtom, Check::IsBool]),
        Type::Nil => Check::And(vec![Check::IsAtom, Check::IsNil]),
        Type::Pair(..) if always_atom => return Err(CheckError::Impossible(original_type_id, rhs)),
        Type::Pair(first, rest) => {
            let (first, rest) = (*first, *rest);

            let first_items: Vec<_> = pairs.iter().map(|(first, _)| *first).collect();
            let rest_items: Vec<_> = pairs.iter().map(|(_, rest)| *rest).collect();

            let first =
                check_union_against_rhs(types, original_type_id, &first_items, first, visited)?;
            let rest =
                check_union_against_rhs(types, original_type_id, &rest_items, rest, visited)?;

            let pair_check = Check::Pair(Box::new(first), Box::new(rest));

            if always_pair {
                pair_check
            } else {
                Check::And(vec![Check::IsPair, pair_check])
            }
        }
    })
}

pub(crate) fn simplify_check(check: Check) -> Check {
    match check {
        Check::None => Check::None,
        Check::IsAtom => Check::IsAtom,
        Check::IsPair => Check::IsPair,
        Check::IsBool => Check::IsBool,
        Check::IsNil => Check::IsNil,
        Check::Length(len) => {
            if len == 0 {
                Check::IsNil
            } else {
                Check::Length(len)
            }
        }
        Check::And(items) => {
            let mut result = Vec::new();

            let mut is_atom = false;
            let mut is_pair = false;
            let mut is_bool = false;
            let mut is_nil = false;
            let mut length = false;

            let mut items: VecDeque<_> = items.into();

            while !items.is_empty() {
                let item = simplify_check(items.pop_front().unwrap());

                match item {
                    Check::None => continue,
                    Check::IsAtom => {
                        if is_atom {
                            continue;
                        }
                        is_atom = true;
                    }
                    Check::IsPair => {
                        if is_pair {
                            continue;
                        }
                        is_pair = true;
                    }
                    Check::IsBool => {
                        if is_bool {
                            continue;
                        }
                        is_bool = true;
                    }
                    Check::IsNil => {
                        if is_nil {
                            continue;
                        }
                        is_nil = true;
                    }
                    Check::Length(..) => {
                        if length {
                            continue;
                        }
                        length = false;
                    }
                    Check::And(children) => {
                        items.extend(children);
                        continue;
                    }
                    _ => {}
                }

                result.push(item);
            }

            if result.is_empty() {
                Check::None
            } else if result.len() == 1 {
                result.remove(0)
            } else {
                Check::And(result)
            }
        }
        Check::Or(items) => {
            let mut result = Vec::new();
            let mut atoms: Vec<Check> = Vec::new();
            let mut pairs: Vec<Check> = Vec::new();

            let mut items: VecDeque<_> = items.into();

            while !items.is_empty() {
                let item = simplify_check(items.pop_front().unwrap());

                match item {
                    Check::And(children) => {
                        match children
                            .iter()
                            .find(|child| matches!(child, Check::IsAtom | Check::IsPair))
                        {
                            Some(Check::IsAtom) => {
                                atoms.push(Check::And(
                                    children
                                        .into_iter()
                                        .filter(|child| *child != Check::IsAtom)
                                        .collect(),
                                ));
                                continue;
                            }
                            Some(Check::IsPair) => {
                                pairs.push(Check::And(
                                    children
                                        .into_iter()
                                        .filter(|child| *child != Check::IsPair)
                                        .collect(),
                                ));
                                continue;
                            }
                            _ => {}
                        }
                    }
                    Check::Or(children) => {
                        items.extend(children);
                        continue;
                    }
                    _ => {}
                }
            }

            if !atoms.is_empty() && !pairs.is_empty() {
                if atoms.len() > pairs.len() {
                    result.push(Check::If(
                        Box::new(Check::IsAtom),
                        Box::new(Check::Or(atoms)),
                        Box::new(Check::Or(pairs)),
                    ));
                } else {
                    result.push(Check::If(
                        Box::new(Check::IsPair),
                        Box::new(Check::Or(pairs)),
                        Box::new(Check::Or(atoms)),
                    ));
                }
            } else if atoms.is_empty() {
                result.push(Check::And(vec![Check::IsPair, Check::Or(pairs)]));
            } else if pairs.is_empty() {
                result.push(Check::And(vec![Check::IsAtom, Check::Or(atoms)]));
            }

            if result.len() == 1 {
                result.remove(0)
            } else {
                Check::Or(result)
            }
        }
        Check::If(cond, then, else_) => {
            let cond = simplify_check(*cond);
            let then = simplify_check(*then);
            let else_ = simplify_check(*else_);
            Check::If(Box::new(cond), Box::new(then), Box::new(else_))
        }
        Check::Pair(first, rest) => {
            let first = simplify_check(*first);
            let rest = simplify_check(*rest);
            Check::Pair(Box::new(first), Box::new(rest))
        }
    }
}

fn fmt_val(f: &mut fmt::Formatter<'_>, path: &[TypePath]) -> fmt::Result {
    for path in path.iter().rev() {
        match path {
            TypePath::First => write!(f, "(f ")?,
            TypePath::Rest => write!(f, "(r ")?,
        }
    }
    write!(f, "val")?;
    for _ in 0..path.len() {
        write!(f, ")")?;
    }
    Ok(())
}

fn fmt_check(check: &Check, f: &mut fmt::Formatter<'_>, path: &mut Vec<TypePath>) -> fmt::Result {
    match check {
        Check::None => write!(f, "1"),
        Check::IsPair => {
            write!(f, "(l ")?;
            fmt_val(f, path)?;
            write!(f, ")")
        }
        Check::IsAtom => {
            write!(f, "(not (l ")?;
            fmt_val(f, path)?;
            write!(f, "))")
        }
        Check::IsBool => {
            write!(f, "(any (= ")?;
            fmt_val(f, path)?;
            write!(f, " ()) (= ")?;
            fmt_val(f, path)?;
            write!(f, " 1))")
        }
        Check::IsNil => {
            write!(f, "(= ")?;
            fmt_val(f, path)?;
            write!(f, " ())")
        }
        Check::Length(len) => {
            write!(f, "(= (strlen ")?;
            fmt_val(f, path)?;
            write!(f, ") {len})")
        }
        Check::And(checks) => {
            write!(f, "(and")?;
            for check in checks {
                write!(f, " ")?;
                fmt_check(check, f, path)?;
            }
            write!(f, ")")
        }
        Check::Or(checks) => {
            write!(f, "(or")?;
            for check in checks {
                write!(f, " ")?;
                fmt_check(check, f, path)?;
            }
            write!(f, ")")
        }
        Check::If(cond, then, else_) => {
            write!(f, "(if ")?;
            fmt_check(cond, f, path)?;
            write!(f, " ")?;
            fmt_check(then, f, path)?;
            write!(f, " ")?;
            fmt_check(else_, f, path)?;
            write!(f, ")")
        }
        Check::Pair(first, rest) => {
            let has_first = first.as_ref() != &Check::None;
            let has_rest = rest.as_ref() != &Check::None;

            if has_first && has_rest {
                write!(f, "(all ")?;
                path.push(TypePath::First);
                fmt_check(first, f, path)?;
                path.pop().unwrap();
                write!(f, " ")?;
                path.push(TypePath::Rest);
                fmt_check(rest, f, path)?;
                path.pop().unwrap();
                write!(f, ")")
            } else if has_first {
                path.push(TypePath::First);
                fmt_check(first, f, path)?;
                path.pop().unwrap();
                Ok(())
            } else if has_rest {
                path.push(TypePath::Rest);
                fmt_check(rest, f, path)?;
                path.pop().unwrap();
                Ok(())
            } else {
                write!(f, "1")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Comparison, StandardTypes};

    use super::*;

    #[test]
    fn check_incompatible() {
        let mut db = TypeSystem::new();
        let types = StandardTypes::alloc(&mut db);

        assert!(matches!(
            db.check(types.bytes32, types.public_key).unwrap_err(),
            CheckError::Impossible(..)
        ));

        assert_eq!(
            db.compare(types.bytes32, types.public_key),
            Comparison::Incompatible
        );

        let difference = db.difference(&types, types.bytes32, types.public_key);
        assert_eq!(db.compare(difference, types.bytes32), Comparison::Equal);
    }

    #[test]
    fn check_any_bytes32() {
        let mut db = TypeSystem::new();
        let types = StandardTypes::alloc(&mut db);

        assert_eq!(
            format!("{}", db.check(types.any, types.bytes32).unwrap()),
            "(and (not (l val)) (= (strlen val) 32))"
        );

        assert_eq!(db.compare(types.any, types.bytes32), Comparison::Superset);

        let difference = db.difference(&types, types.any, types.bytes32);
        assert_eq!(db.compare(difference, types.any), Comparison::Assignable);
    }

    #[test]
    fn check_any_int() {
        let mut db = TypeSystem::new();
        let types = StandardTypes::alloc(&mut db);

        assert_eq!(
            format!("{}", db.check(types.any, types.int).unwrap()),
            "(not (l val))"
        );

        assert_eq!(db.compare(types.any, types.int), Comparison::Superset);

        let difference = db.difference(&types, types.any, types.int);
        assert_eq!(db.compare(difference, types.any), Comparison::Assignable);
    }

    #[test]
    fn check_any_any() {
        let mut db = TypeSystem::new();
        let types = StandardTypes::alloc(&mut db);

        assert!(matches!(
            db.check(types.any, types.any).unwrap_err(),
            CheckError::Recursive(..)
        ));

        assert_eq!(db.compare(types.any, types.any), Comparison::Assignable);

        let difference = db.difference(&types, types.any, types.any);
        assert_eq!(db.compare(difference, types.any), Comparison::Assignable);
    }

    #[test]
    fn check_optional_int() {
        let mut db = TypeSystem::new();
        let types = StandardTypes::alloc(&mut db);
        let pair = db.alloc(Type::Pair(types.int, types.int));
        let optional_pair = db.alloc(Type::Union(vec![pair, types.nil]));

        assert_eq!(
            format!("{}", db.check(optional_pair, types.nil).unwrap()),
            "(and (not (l val)) (= val ()))"
        );

        assert_eq!(db.compare(optional_pair, types.nil), Comparison::Superset);

        let difference = db.difference(&types, optional_pair, types.nil);
        assert_eq!(db.compare(difference, pair), Comparison::Equal);

        assert_eq!(
            format!("{}", db.check(optional_pair, pair).unwrap()),
            "(and (l val) 1)"
        );

        assert_eq!(db.compare(optional_pair, pair), Comparison::Superset);

        let difference = db.difference(&types, optional_pair, pair);
        assert_eq!(db.compare(difference, types.nil), Comparison::Equal);
    }

    #[test]
    fn check_simple_union() {
        let mut db = TypeSystem::new();
        let types = StandardTypes::alloc(&mut db);
        let union = db.alloc(Type::Union(vec![types.bytes32, types.public_key]));

        assert_eq!(
            format!("{}", db.check(union, types.public_key).unwrap()),
            "(= (strlen val) 48)"
        );

        assert_eq!(db.compare(union, types.public_key), Comparison::Superset);

        let difference = db.difference(&types, union, types.public_key);
        assert_eq!(db.compare(difference, types.bytes32), Comparison::Equal);
    }

    #[test]
    fn check_any_pair_bytes32_pair_int_nil() {
        let mut db = TypeSystem::new();
        let types = StandardTypes::alloc(&mut db);

        let int_nil_pair = db.alloc(Type::Pair(types.int, types.nil));
        let ty = db.alloc(Type::Pair(types.bytes32, int_nil_pair));

        assert_eq!(
            format!("{}", db.check(types.any, ty).unwrap()),
            "(and (l val) (all (and (not (l (f val))) (= (strlen (f val)) 32)) (and (l (r val)) (all (not (l (f (r val)))) (and (not (l (r (r val)))) (= (r (r val)) ()))))))"
        );

        assert_eq!(db.compare(types.any, ty), Comparison::Superset);

        let difference = db.difference(&types, types.any, ty);
        assert_eq!(db.compare(difference, types.any), Comparison::Assignable);
    }

    #[test]
    fn check_complex_type_unions() {
        let mut db = TypeSystem::new();
        let types = StandardTypes::alloc(&mut db);

        let int_nil_pair = db.alloc(Type::Pair(types.int, types.nil));
        let bytes32_pair = db.alloc(Type::Pair(types.bytes32, types.bytes32));
        let complex_pair = db.alloc(Type::Pair(int_nil_pair, bytes32_pair));

        let lhs = db.alloc(Type::Union(vec![
            types.bytes32,
            types.public_key,
            types.nil,
            complex_pair,
            int_nil_pair,
            bytes32_pair,
            types.bool,
        ]));

        let rhs = db.alloc(Type::Union(vec![
            types.bytes32,
            bytes32_pair,
            types.nil,
            complex_pair,
        ]));

        let expected_diff = db.alloc(Type::Union(vec![
            int_nil_pair,
            types.public_key,
            types.bool,
        ]));

        assert_eq!(
            format!("{}", db.check(lhs, rhs).unwrap()),
            "(if (l val) (or (and (all (and (not (l (f val))) (= (strlen (f val)) 32)) (and (not (l (r val))) (= (strlen (r val)) 32)))) (and (all (and (l (f val)) 1) (and (l (r val)) 1)))) (or (and (= (strlen val) 32)) (and (= val ()))))"
        );

        assert_eq!(db.compare(lhs, rhs), Comparison::Superset);

        let difference = db.difference(&types, lhs, rhs);
        assert_eq!(
            db.compare(difference, expected_diff),
            Comparison::Assignable
        );

        assert_eq!(format!("{}", db.check(types.any, rhs).unwrap()),
            "(if (l val) (or (and (all (and (not (l (f val))) (= (strlen (f val)) 32)) (and (not (l (r val))) (= (strlen (r val)) 32)))) (and (all (and (l (f val)) (all (not (l (f (f val)))) (and (not (l (r (f val)))) (= (r (f val)) ())))) (and (l (r val)) (all (and (not (l (f (r val)))) (= (strlen (f (r val))) 32)) (and (not (l (r (r val)))) (= (strlen (r (r val))) 32))))))) (or (and (= (strlen val) 32)) (and (= val ()))))"
        );

        assert_eq!(db.compare(types.any, rhs), Comparison::Superset);

        let difference = db.difference(&types, types.any, rhs);
        assert_eq!(db.compare(difference, types.any), Comparison::Assignable);
    }
}
