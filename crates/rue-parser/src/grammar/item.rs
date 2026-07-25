use rowan::Checkpoint;
use rue_diagnostic::DiagnosticKind;

use crate::{
    Parser, SyntaxKind, T,
    grammar::{binding::binding, block::block, expr::expr, generics::generic_parameters, ty::ty},
};

pub fn item(p: &mut Parser) {
    let cp = p.checkpoint();
    let export = p.try_eat(T![export]);
    let inline = p.try_eat(T![inline]);
    let extern_kw = if inline { false } else { p.try_eat(T![extern]) };
    let test = p.try_eat(T![test]);

    if p.at(T![mod]) && !inline && !extern_kw && !test {
        module_item(p, cp);
    } else if p.at(T![fn]) {
        function_item(p, cp, extern_kw);
    } else if p.at(T![const]) && !extern_kw && !test {
        constant_item(p, cp);
    } else if p.at(T![type]) && !inline && !extern_kw && !test {
        type_alias_item(p, cp);
    } else if p.at(T![struct]) && !inline && !extern_kw && !test {
        struct_item(p, cp);
    } else if p.at(T![import]) || export {
        import_item(p, cp, export);
    } else {
        p.skip();
    }
}

fn module_item(p: &mut Parser, cp: Checkpoint) {
    p.start_at(cp, SyntaxKind::ModuleItem);
    p.expect(T![mod]);
    p.expect(SyntaxKind::Ident);
    p.expect(T!['{']);
    while !p.at(T!['}']) && !p.at(SyntaxKind::Eof) {
        item(p);
    }
    p.expect(T!['}']);
    p.finish();
}

fn function_item(p: &mut Parser, cp: Checkpoint, extern_kw: bool) {
    p.start_at(cp, SyntaxKind::FunctionItem);
    p.expect(T![fn]);
    p.expect(SyntaxKind::Ident);
    if p.at(T![<]) {
        generic_parameters(p);
    }
    p.expect(T!['(']);
    while !p.at(T![')']) {
        function_parameter(p);
        if !p.try_eat(T![,]) {
            break;
        }
    }
    p.expect(T![')']);
    if p.try_eat(T![->]) {
        ty(p);
    }
    if extern_kw && p.at(T![from]) {
        p.expect(T![from]);
        p.expect(SyntaxKind::String);
        p.expect(T![;]);
    } else if p.at(T!['{']) {
        block(p);
    } else {
        p.error(DiagnosticKind::MissingFunctionBody);
    }
    p.finish();
}

fn function_parameter(p: &mut Parser) {
    p.start(SyntaxKind::FunctionParameter);
    p.try_eat(T![...]);
    binding(p);
    p.expect(T![:]);
    ty(p);
    p.finish();
}

fn constant_item(p: &mut Parser, cp: Checkpoint) {
    p.start_at(cp, SyntaxKind::ConstantItem);
    p.expect(T![const]);
    p.expect(SyntaxKind::Ident);
    p.expect(T![:]);
    ty(p);
    p.expect(T![=]);
    expr(p);
    p.expect(T![;]);
    p.finish();
}

fn type_alias_item(p: &mut Parser, cp: Checkpoint) {
    p.start_at(cp, SyntaxKind::TypeAliasItem);
    p.expect(T![type]);
    p.expect(SyntaxKind::Ident);
    if p.at(T![<]) {
        generic_parameters(p);
    }
    p.expect(T![=]);
    ty(p);
    p.expect(T![;]);
    p.finish();
}

fn struct_item(p: &mut Parser, cp: Checkpoint) {
    p.start_at(cp, SyntaxKind::StructItem);
    p.expect(T![struct]);
    p.expect(SyntaxKind::Ident);
    if p.at(T![<]) {
        generic_parameters(p);
    }
    p.expect(T!['{']);
    while !p.at(T!['}']) {
        struct_field(p);
        if !p.try_eat(T![,]) {
            break;
        }
    }
    p.expect(T!['}']);
    p.finish();
}

fn struct_field(p: &mut Parser) {
    p.start(SyntaxKind::StructField);
    p.try_eat(T![...]);
    p.expect(SyntaxKind::Ident);
    if p.try_eat(T![:]) {
        ty(p);
        if p.try_eat(T![=]) {
            expr(p);
        }
    } else {
        p.expect(T![=]);
        expr(p);
    }
    p.finish();
}

fn import_item(p: &mut Parser, cp: Checkpoint, export: bool) {
    p.start_at(cp, SyntaxKind::ImportItem);
    if !export {
        p.expect(T![import]);
    }
    import_path(p, true);
    p.expect(T![;]);
    p.finish();
}

fn import_path(p: &mut Parser, mut first: bool) {
    p.start(SyntaxKind::ImportPath);
    let mut required = true;
    while p.at(T![::]) || required {
        if import_path_segment(p, first) {
            break;
        }
        first = false;
        required = false;
    }
    p.finish();
}

fn import_path_segment(p: &mut Parser, first: bool) -> bool {
    p.start(SyntaxKind::ImportPathSegment);
    p.try_eat(T![::]);
    let last = if !first && p.at(T![*]) {
        p.expect(T![*]);
        true
    } else if !first && p.at(T!['{']) {
        p.expect(T!['{']);
        while !p.at(T!['}']) {
            import_path(p, false);
            if !p.try_eat(T![,]) {
                break;
            }
        }
        p.expect(T!['}']);
        true
    } else if p.at(T![super]) {
        p.expect(T![super]);
        false
    } else {
        p.expect(SyntaxKind::Ident);
        false
    };
    p.finish();
    last
}

#[cfg(test)]
mod tests {
    use expect_test::expect;

    use crate::grammar::tests::check;

    use super::*;

    #[test]
    fn test_function_item() {
        check(
            item,
            "fn main() -> Int {}",
            expect![[r#"
                FunctionItem@0..19
                  Fn@0..2 "fn"
                  Whitespace@2..3 " "
                  Ident@3..7 "main"
                  OpenParen@7..8 "("
                  CloseParen@8..9 ")"
                  Whitespace@9..10 " "
                  Arrow@10..12 "->"
                  Whitespace@12..13 " "
                  PathType@13..17
                    PathSegment@13..17
                      Ident@13..16 "Int"
                      Whitespace@16..17 " "
                  Block@17..19
                    OpenBrace@17..18 "{"
                    CloseBrace@18..19 "}"
            "#]],
            expect![""],
        );

        check(
            item,
            "fn main<T>() -> T {}",
            expect![[r#"
                FunctionItem@0..20
                  Fn@0..2 "fn"
                  Whitespace@2..3 " "
                  Ident@3..7 "main"
                  GenericParameters@7..10
                    LessThan@7..8 "<"
                    Ident@8..9 "T"
                    GreaterThan@9..10 ">"
                  OpenParen@10..11 "("
                  CloseParen@11..12 ")"
                  Whitespace@12..13 " "
                  Arrow@13..15 "->"
                  Whitespace@15..16 " "
                  PathType@16..18
                    PathSegment@16..18
                      Ident@16..17 "T"
                      Whitespace@17..18 " "
                  Block@18..20
                    OpenBrace@18..19 "{"
                    CloseBrace@19..20 "}"
            "#]],
            expect![""],
        );

        check(
            item,
            "fn main(value: Int) -> Int { value + 42 }",
            expect![[r#"
                FunctionItem@0..41
                  Fn@0..2 "fn"
                  Whitespace@2..3 " "
                  Ident@3..7 "main"
                  OpenParen@7..8 "("
                  FunctionParameter@8..18
                    NamedBinding@8..13
                      Ident@8..13 "value"
                    Colon@13..14 ":"
                    Whitespace@14..15 " "
                    PathType@15..18
                      PathSegment@15..18
                        Ident@15..18 "Int"
                  CloseParen@18..19 ")"
                  Whitespace@19..20 " "
                  Arrow@20..22 "->"
                  Whitespace@22..23 " "
                  PathType@23..27
                    PathSegment@23..27
                      Ident@23..26 "Int"
                      Whitespace@26..27 " "
                  Block@27..41
                    OpenBrace@27..28 "{"
                    Whitespace@28..29 " "
                    BinaryExpr@29..40
                      PathExpr@29..35
                        PathSegment@29..35
                          Ident@29..34 "value"
                          Whitespace@34..35 " "
                      Plus@35..36 "+"
                      Whitespace@36..37 " "
                      LiteralExpr@37..39
                        Integer@37..39 "42"
                      Whitespace@39..40 " "
                    CloseBrace@40..41 "}"
            "#]],
            expect![""],
        );
    }

    #[test]
    fn test_external_function_item() {
        check(
            item,
            "extern fn add(a: Int, b: Int) -> Int\n    from \"./add.hex\";",
            expect![[r#"
                FunctionItem@0..58
                  Extern@0..6 "extern"
                  Whitespace@6..7 " "
                  Fn@7..9 "fn"
                  Whitespace@9..10 " "
                  Ident@10..13 "add"
                  OpenParen@13..14 "("
                  FunctionParameter@14..20
                    NamedBinding@14..15
                      Ident@14..15 "a"
                    Colon@15..16 ":"
                    Whitespace@16..17 " "
                    PathType@17..20
                      PathSegment@17..20
                        Ident@17..20 "Int"
                  Comma@20..21 ","
                  Whitespace@21..22 " "
                  FunctionParameter@22..28
                    NamedBinding@22..23
                      Ident@22..23 "b"
                    Colon@23..24 ":"
                    Whitespace@24..25 " "
                    PathType@25..28
                      PathSegment@25..28
                        Ident@25..28 "Int"
                  CloseParen@28..29 ")"
                  Whitespace@29..30 " "
                  Arrow@30..32 "->"
                  Whitespace@32..33 " "
                  PathType@33..41
                    PathSegment@33..41
                      Ident@33..36 "Int"
                      Whitespace@36..41 "\n    "
                  From@41..45 "from"
                  Whitespace@45..46 " "
                  String@46..57 "\"./add.hex\""
                  Semicolon@57..58 ";"
            "#]],
            expect![""],
        );

        check(
            item,
            "extern fn add(a: Int, b: Int) -> Int { a + b }",
            expect![[r#"
                FunctionItem@0..46
                  Extern@0..6 "extern"
                  Whitespace@6..7 " "
                  Fn@7..9 "fn"
                  Whitespace@9..10 " "
                  Ident@10..13 "add"
                  OpenParen@13..14 "("
                  FunctionParameter@14..20
                    NamedBinding@14..15
                      Ident@14..15 "a"
                    Colon@15..16 ":"
                    Whitespace@16..17 " "
                    PathType@17..20
                      PathSegment@17..20
                        Ident@17..20 "Int"
                  Comma@20..21 ","
                  Whitespace@21..22 " "
                  FunctionParameter@22..28
                    NamedBinding@22..23
                      Ident@22..23 "b"
                    Colon@23..24 ":"
                    Whitespace@24..25 " "
                    PathType@25..28
                      PathSegment@25..28
                        Ident@25..28 "Int"
                  CloseParen@28..29 ")"
                  Whitespace@29..30 " "
                  Arrow@30..32 "->"
                  Whitespace@32..33 " "
                  PathType@33..37
                    PathSegment@33..37
                      Ident@33..36 "Int"
                      Whitespace@36..37 " "
                  Block@37..46
                    OpenBrace@37..38 "{"
                    Whitespace@38..39 " "
                    BinaryExpr@39..45
                      PathExpr@39..41
                        PathSegment@39..41
                          Ident@39..40 "a"
                          Whitespace@40..41 " "
                      Plus@41..42 "+"
                      Whitespace@42..43 " "
                      PathExpr@43..45
                        PathSegment@43..45
                          Ident@43..44 "b"
                          Whitespace@44..45 " "
                    CloseBrace@45..46 "}"
            "#]],
            expect![""],
        );
    }

    #[test]
    fn test_external_source_requires_extern() {
        check(
            item,
            "fn add() from \"./add.hex\";",
            expect![[r#"
                FunctionItem@0..9
                  Fn@0..2 "fn"
                  Whitespace@2..3 " "
                  Ident@3..6 "add"
                  OpenParen@6..7 "("
                  CloseParen@7..8 ")"
                  Whitespace@8..9 " "
            "#]],
            expect!["Missing function body at main.rue:1:10"],
        );
    }

    #[test]
    fn test_constant_item() {
        check(
            item,
            "inline const VALUE: Int = 42;",
            expect![[r#"
                ConstantItem@0..29
                  Inline@0..6 "inline"
                  Whitespace@6..7 " "
                  Const@7..12 "const"
                  Whitespace@12..13 " "
                  Ident@13..18 "VALUE"
                  Colon@18..19 ":"
                  Whitespace@19..20 " "
                  PathType@20..24
                    PathSegment@20..24
                      Ident@20..23 "Int"
                      Whitespace@23..24 " "
                  Assign@24..25 "="
                  Whitespace@25..26 " "
                  LiteralExpr@26..28
                    Integer@26..28 "42"
                  Semicolon@28..29 ";"
            "#]],
            expect![""],
        );
    }

    #[test]
    fn test_type_alias_item() {
        check(
            item,
            "type Hello = World;",
            expect![[r#"
                TypeAliasItem@0..19
                  Type@0..4 "type"
                  Whitespace@4..5 " "
                  Ident@5..10 "Hello"
                  Whitespace@10..11 " "
                  Assign@11..12 "="
                  Whitespace@12..13 " "
                  PathType@13..18
                    PathSegment@13..18
                      Ident@13..18 "World"
                  Semicolon@18..19 ";"
            "#]],
            expect![""],
        );
    }

    #[test]
    fn test_import_item() {
        check(
            item,
            "import clvm::quote;",
            expect![[r#"
                ImportItem@0..19
                  Import@0..6 "import"
                  Whitespace@6..7 " "
                  ImportPath@7..18
                    ImportPathSegment@7..11
                      Ident@7..11 "clvm"
                    ImportPathSegment@11..18
                      PathSeparator@11..13 "::"
                      Ident@13..18 "quote"
                  Semicolon@18..19 ";"
            "#]],
            expect![""],
        );

        check(
            item,
            "import hello_world;",
            expect![[r#"
                ImportItem@0..19
                  Import@0..6 "import"
                  Whitespace@6..7 " "
                  ImportPath@7..18
                    ImportPathSegment@7..18
                      Ident@7..18 "hello_world"
                  Semicolon@18..19 ";"
            "#]],
            expect![""],
        );

        check(
            item,
            "import;",
            expect![[r#"
                ImportItem@0..6
                  Import@0..6 "import"
                  ImportPath@6..6
                    ImportPathSegment@6..6
            "#]],
            expect![[r#"
                Expected one of `::`, `super`, identifier, found `;` at main.rue:1:7
                Expected one of `::`, `;`, found eof at main.rue:1:8"#]],
        );

        check(
            item,
            "import *;",
            expect![[r#"
                ImportItem@0..8
                  Import@0..6 "import"
                  Whitespace@6..7 " "
                  ImportPath@7..7
                    ImportPathSegment@7..7
                  Semicolon@7..8 ";"
            "#]],
            expect!["Expected one of `::`, `super`, identifier, found `*` at main.rue:1:8"],
        );

        check(
            item,
            "import {};",
            expect![[r#"
                ImportItem@0..7
                  Import@0..6 "import"
                  Whitespace@6..7 " "
                  ImportPath@7..7
                    ImportPathSegment@7..7
            "#]],
            expect![[r#"
                Expected one of `::`, `super`, identifier, found `{` at main.rue:1:8
                Expected one of `::`, `;`, found `}` at main.rue:1:9"#]],
        );

        check(
            item,
            "import {x, y};",
            expect![[r#"
                ImportItem@0..7
                  Import@0..6 "import"
                  Whitespace@6..7 " "
                  ImportPath@7..7
                    ImportPathSegment@7..7
            "#]],
            expect![[r#"
                Expected one of `::`, `super`, identifier, found `{` at main.rue:1:8
                Expected one of `::`, `;`, found identifier at main.rue:1:9"#]],
        );

        check(
            item,
            "import hello::{};",
            expect![[r#"
                ImportItem@0..17
                  Import@0..6 "import"
                  Whitespace@6..7 " "
                  ImportPath@7..16
                    ImportPathSegment@7..12
                      Ident@7..12 "hello"
                    ImportPathSegment@12..16
                      PathSeparator@12..14 "::"
                      OpenBrace@14..15 "{"
                      CloseBrace@15..16 "}"
                  Semicolon@16..17 ";"
            "#]],
            expect![""],
        );

        check(
            item,
            "import hello::{world, there,};",
            expect![[r#"
                ImportItem@0..30
                  Import@0..6 "import"
                  Whitespace@6..7 " "
                  ImportPath@7..29
                    ImportPathSegment@7..12
                      Ident@7..12 "hello"
                    ImportPathSegment@12..29
                      PathSeparator@12..14 "::"
                      OpenBrace@14..15 "{"
                      ImportPath@15..20
                        ImportPathSegment@15..20
                          Ident@15..20 "world"
                      Comma@20..21 ","
                      Whitespace@21..22 " "
                      ImportPath@22..27
                        ImportPathSegment@22..27
                          Ident@22..27 "there"
                      Comma@27..28 ","
                      CloseBrace@28..29 "}"
                  Semicolon@29..30 ";"
            "#]],
            expect![""],
        );

        check(
            item,
            "import hello::*;",
            expect![[r#"
                ImportItem@0..16
                  Import@0..6 "import"
                  Whitespace@6..7 " "
                  ImportPath@7..15
                    ImportPathSegment@7..12
                      Ident@7..12 "hello"
                    ImportPathSegment@12..15
                      PathSeparator@12..14 "::"
                      Star@14..15 "*"
                  Semicolon@15..16 ";"
            "#]],
            expect![""],
        );

        check(
            item,
            "export clvm::quote;",
            expect![[r#"
                ImportItem@0..19
                  Export@0..6 "export"
                  Whitespace@6..7 " "
                  ImportPath@7..18
                    ImportPathSegment@7..11
                      Ident@7..11 "clvm"
                    ImportPathSegment@11..18
                      PathSeparator@11..13 "::"
                      Ident@13..18 "quote"
                  Semicolon@18..19 ";"
            "#]],
            expect![""],
        );
    }
}
