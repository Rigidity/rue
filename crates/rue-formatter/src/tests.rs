use expect_test::{Expect, expect};

use crate::{FormatError, FormatOptions, format_source};

#[allow(clippy::needless_pass_by_value)]
fn check(input: &str, expected: Expect) {
    let output = format_source(input, &FormatOptions::default()).expect("source should format");
    expected.assert_eq(&output);
    assert_eq!(
        format_source(&output, &FormatOptions::default()).unwrap(),
        output
    );
}

#[test]
fn items_blocks_and_expressions() {
    check(
        "inline   const VALUE:Int=1+2*3;\nfn main( x:Int)->Int{let y=x+VALUE;y}",
        expect![[r#"
            inline const VALUE: Int = 1 + 2 * 3;

            fn main(x: Int) -> Int {
                let y = x + VALUE;
                y
            }
        "#]],
    );
}

#[test]
fn types_bindings_generics_and_imports() {
    check(
        "import foo::*; import foo::{bar,baz,}; type Pair<T>=(T,T,); fn take<T>(...[a,{x:y}]:[T,...T])->fn(a:T)->T{}",
        expect![[r#"
            import foo::*;
            import foo::{bar, baz};

            type Pair<T> = (T, T);

            fn take<T>(...[a, { x: y }]: [T, ...T]) -> fn(a: T) -> T {}
        "#]],
    );
}

#[test]
fn comments_are_preserved_once() {
    check(
        "// lead\nfn main(/* a */x:Int){// before\nlet y=x/* op */+1; // trailing\n// tail\ny}",
        expect![[r#"
            // lead
            fn main(/* a */ x: Int) {
                // before
                let y = x /* op */ + 1; // trailing
                // tail
                y
            }
        "#]],
    );
}

#[test]
fn comment_position_matrix() {
    check(
        "/* file */\nfn main(/* parameter */x:Int)->Int{let y=[/* dangling */];x/* left */+/* right */1}// eof",
        expect![[r#"
            /* file */
            fn main(/* parameter */ x: Int) -> Int {
                let y = [ /* dangling */ ];
                x /* left */ + /* right */ 1
            } // eof
        "#]],
    );
}

#[test]
fn multiline_comment_text_is_exact() {
    let output = format_source(
        "fn main(){/* first  \nsecond */1}",
        &FormatOptions::default(),
    )
    .unwrap();
    assert!(output.contains("/* first  \nsecond */"));
}

#[test]
fn preserves_one_intentional_blank_line() {
    check(
        "fn main(){let a=1;\n\n\nlet b=2;\na+b}",
        expect![[r#"
            fn main() {
                let a = 1;

                let b = 2;
                a + b
            }
        "#]],
    );
}

#[test]
fn width_breaks_delimited_groups() {
    let options = FormatOptions {
        max_width: 30,
        ..FormatOptions::default()
    };
    let output = format_source(
        "fn main(first:VeryLongType,second:VeryLongType)->VeryLongType{first(second,second)}",
        &options,
    )
    .unwrap();
    expect![[r#"
        fn main(
            first: VeryLongType,
            second: VeryLongType,
        ) -> VeryLongType {
            first(second, second)
        }
    "#]]
    .assert_eq(&output);
    assert_eq!(format_source(&output, &options).unwrap(), output);
}

#[test]
fn malformed_input_is_rejected() {
    let error = format_source("fn main( {", &FormatOptions::default()).unwrap_err();
    assert!(matches!(error, FormatError::Parse { .. }));
}

#[test]
fn all_expression_families() {
    check(
        "fn f(x:Int)->Int{debug fn<T>(a:T):T=>a;assert x is Int;return if x>0{{x}.field as Int}else{const{x}};}",
        expect![[r#"
            fn f(x: Int) -> Int {
                debug fn<T>(a: T): T => a;
                assert x is Int;
                return if x > 0 {
                    {
                        x
                    }.field as Int
                } else {
                    const {
                        x
                    }
                };
            }
        "#]],
    );
}

#[test]
fn standalone_comment_inside_broken_group() {
    check(
        "fn main(){[// explain\n1]}",
        expect![[r#"
            fn main() {
                [
                    // explain
                    1,
                ]
            }
        "#]],
    );
}

#[test]
fn short_binary_expressions_stay_flat() {
    check(
        "fn main(){let num=42;fn()=>num+num}",
        expect![[r#"
            fn main() {
                let num = 42;
                fn() => num + num
            }
        "#]],
    );
}

#[test]
fn nested_delimiters_use_single_indent() {
    let options = FormatOptions {
        max_width: 40,
        ..FormatOptions::default()
    };
    let output = format_source(
        "fn main(){assert tree_hash(fizz_buzz(1,15))==tree_hash([1,2,3,4,5,6]);}",
        &options,
    )
    .unwrap();
    expect![[r#"
        fn main() {
            assert tree_hash(
                fizz_buzz(1, 15),
            ) == tree_hash([1, 2, 3, 4, 5, 6]);
        }
    "#]]
    .assert_eq(&output);
    assert_eq!(format_source(&output, &options).unwrap(), output);
}

#[test]
fn binary_chain_uses_one_continuation_indent() {
    let options = FormatOptions {
        max_width: 24,
        ..FormatOptions::default()
    };
    let output =
        format_source("fn main(){first_value+second_value+third_value}", &options).unwrap();
    expect![[r#"
        fn main() {
            first_value
                + second_value
                + third_value
        }
    "#]]
    .assert_eq(&output);
    assert_eq!(format_source(&output, &options).unwrap(), output);
}

#[test]
fn binary_operators_inside_if_are_not_chain_members() {
    check(
        r#"fn main(){1+if char=="["{2}else if char=="]"{3}else{4}}"#,
        expect![[r#"
            fn main() {
                1
                    + if char == "[" {
                        2
                    } else if char == "]" {
                        3
                    } else {
                        4
                    }
            }
        "#]],
    );
}

#[test]
fn comparison_wraps_rhs_inside_logical_chain() {
    let options = FormatOptions {
        max_width: 35,
        ..FormatOptions::default()
    };
    let output = format_source(
        "fn main(){let x=first_condition&&second_value==call(first,second,third);}",
        &options,
    )
    .unwrap();
    expect![[r#"
        fn main() {
            let x = first_condition
                && second_value == call(
                    first,
                    second,
                    third,
                );
        }
    "#]]
    .assert_eq(&output);
    assert_eq!(format_source(&output, &options).unwrap(), output);
}

#[test]
fn generic_trailing_commas_follow_layout() {
    check(
        "fn flat<T,>(){}",
        expect![[r#"
            fn flat<T>() {}
        "#]],
    );

    let options = FormatOptions {
        max_width: 30,
        ..FormatOptions::default()
    };
    let output = format_source(
        "fn example<FirstLongParameter,SecondLongParameter>(){}",
        &options,
    )
    .unwrap();
    expect![[r#"
        fn example<
            FirstLongParameter,
            SecondLongParameter,
        >() {}
    "#]]
    .assert_eq(&output);
    assert_eq!(format_source(&output, &options).unwrap(), output);
}

#[test]
fn union_type_operators_lead_continuation_lines() {
    let options = FormatOptions {
        max_width: 32,
        ..FormatOptions::default()
    };
    let output = format_source(
        "type Condition = FirstVariant | SecondVariant | ThirdVariant;",
        &options,
    )
    .unwrap();
    expect![[r#"
        type Condition = FirstVariant
            | SecondVariant
            | ThirdVariant;
    "#]]
    .assert_eq(&output);
    assert_eq!(format_source(&output, &options).unwrap(), output);
}

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
fn compact_items_are_grouped() {
    check(
        "const FIRST:Int=1;\nconst SECOND:Int=2;\n\nconst THIRD:Int=3;\nconst FOURTH:Int=4;\n\ntype A=Int;\ntype B=Int;\n\nfn main(){}",
        expect![[r#"
            const FIRST: Int = 1;
            const SECOND: Int = 2;

            const THIRD: Int = 3;
            const FOURTH: Int = 4;

            type A = Int;
            type B = Int;

            fn main() {}
        "#]],
    );
}

#[test]
fn block_items_separate_implicit_return_expression() {
    check(
        "fn recurse(items: List<Int>) -> Int { if items is nil { return 0; } recurse(items.rest) }",
        expect![[r#"
            fn recurse(items: List<Int>) -> Int {
                if items is nil {
                    return 0;
                }
                recurse(items.rest)
            }
        "#]],
    );
}
