use super::*;

#[test]
fn imports_are_hoisted_grouped_and_sorted() {
    check(
        "const VALUE:Int=1; import zebra; import beta::{zeta,alpha};\n\nimport delta; import charlie; fn main(){}",
        expect![[r#"
            import beta::{alpha, zeta};
            import zebra;

            import charlie;
            import delta;

            const VALUE: Int = 1;

            fn main() {}
        "#]],
    );
    check(
        "const VALUE:Int=1;\n// zebra\nimport zebra;\nimport alpha;",
        expect![[r#"
            import alpha;
            // zebra
            import zebra;

            const VALUE: Int = 1;
        "#]],
    );
}

#[test]
fn nested_import_paths_are_sorted_without_crossing_groups() {
    check(
        "import root::{zeta::{two,one},alpha,super::thing};\nimport beta::*;\n\nexport zebra::{last,first};\nexport alpha;",
        expect![[r#"
            import beta::*;
            import root::{alpha, super::thing, zeta::{one, two}};

            export alpha;
            export zebra::{first, last};
        "#]],
    );
}

#[test]
fn import_comments_move_with_their_imports() {
    check(
        "// zeta docs\nimport zeta;\n// alpha docs\nimport alpha;\n\n// beta export\nexport beta;",
        expect![[r#"
            // alpha docs
            import alpha;
            // zeta docs
            import zeta;

            // beta export
            export beta;
        "#]],
    );
}

#[test]
fn trailing_comments_stay_with_sorted_imports() {
    check(
        "import zeta; // zeta tail\n// alpha docs\nimport alpha; // alpha tail",
        expect![[r#"
            // alpha docs
            import alpha; // alpha tail
            import zeta; // zeta tail
        "#]],
    );
}

#[test]
fn nested_import_comments_have_stable_owners() {
    check(
        "// root docs\nimport root::{zeta, // zeta tail\n// alpha docs\nalpha};",
        expect![[r#"
            // root docs
            import root::{
                // alpha docs
                alpha,
                zeta, // zeta tail
            };
        "#]],
    );
}

#[test]
fn nested_group_banners_stay_with_the_opener() {
    check(
        "import root::{\n// group banner\n// group banner\n\n// zeta docs\nzeta,\n// alpha docs\nalpha};",
        expect![[r#"
            import root::{
                // group banner
                // group banner

                // alpha docs
                alpha,
                // zeta docs
                zeta,
            };
        "#]],
    );
}

#[test]
fn duplicate_identical_comments_are_preserved_per_import() {
    check(
        "// docs\nimport zeta;\n// docs\nimport alpha;",
        expect![[r#"
            // docs
            import alpha;
            // docs
            import zeta;
        "#]],
    );
}

#[test]
fn file_header_is_not_first_import_documentation() {
    check(
        "// file header\n\n// zeta docs\nimport zeta;\n// alpha docs\nimport alpha;",
        expect![[r#"
            // file header

            // alpha docs
            import alpha;
            // zeta docs
            import zeta;
        "#]],
    );
}
