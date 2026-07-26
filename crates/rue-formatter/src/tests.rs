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
fn wrapped_condition_places_open_brace_on_own_line() {
    let options = FormatOptions {
        max_width: 30,
        ..FormatOptions::default()
    };
    let output = format_source(
        "fn main(){if first_condition&&second_condition{return 1;}0}",
        &options,
    )
    .unwrap();
    expect![[r#"
        fn main() {
            if first_condition
                && second_condition
            {
                return 1;
            }
            0
        }
    "#]]
    .assert_eq(&output);
    assert_eq!(format_source(&output, &options).unwrap(), output);
}

#[test]
fn wrapped_function_headers_place_open_brace_on_own_line() {
    let options = FormatOptions {
        max_width: 34,
        ..FormatOptions::default()
    };
    let output = format_source(
        "fn parameters(first: FirstType, second: SecondType){0}\nfn result()->FirstVariant|SecondVariant|ThirdVariant{0}",
        &options,
    )
    .unwrap();
    expect![[r#"
        fn parameters(
            first: FirstType,
            second: SecondType,
        ) {
            0
        }

        fn result() -> FirstVariant
            | SecondVariant
            | ThirdVariant
        {
            0
        }
    "#]]
    .assert_eq(&output);
    assert_eq!(format_source(&output, &options).unwrap(), output);
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

#[test]
fn empty_files_line_endings_and_final_newline() {
    assert_eq!(format_source("", &FormatOptions::default()).unwrap(), "\n");
    assert_eq!(
        format_source(" \t\r\n\r\n", &FormatOptions::default()).unwrap(),
        "\n"
    );
    check(
        "const FIRST:Int=1;\r\nconst SECOND:Int=2;",
        expect![[r#"
            const FIRST: Int = 1;
            const SECOND: Int = 2;
        "#]],
    );
}

#[test]
fn item_modifiers_modules_and_extern_functions() {
    check(
        r#"export mod api{export type Result<T>=T|nil;export struct Wrapper<T>{value:T}export inline const DEFAULT:Int=1;test fn smoke()->Int{DEFAULT}extern fn foreign(value:Int)->Int from "./foreign.hex";}"#,
        expect![[r#"
            export mod api {
                export type Result<T> = T | nil;
                export struct Wrapper<T> {
                    value: T,
                }
                export inline const DEFAULT: Int = 1;
                test fn smoke() -> Int {
                    DEFAULT
                }
                extern fn foreign(value: Int) -> Int from "./foreign.hex";
            }
        "#]],
    );
}

#[test]
fn struct_field_forms() {
    check(
        "struct Example<T>{opcode=42,value:T,optional:T=nil,...rest:Any=nil}",
        expect![[r#"
            struct Example<T> {
                opcode = 42,
                value: T,
                optional: T = nil,
                ...rest: Any = nil,
            }
        "#]],
    );
}

#[test]
fn statement_forms() {
    check(
        r#"fn statements(value:Int){inline let cached:Int=value;let declared:Int;let inferred=value;inline if value>0{debug value;}assert value!=0;raise "problem";raise;return value;return;value;}"#,
        expect![[r#"
            fn statements(value: Int) {
                inline let cached: Int = value;
                let declared: Int;
                let inferred = value;
                inline if value > 0 {
                    debug value;
                }
                assert value != 0;
                raise "problem";
                raise;
                return value;
                return;
                value;
            }
        "#]],
    );
}

#[test]
fn literal_path_and_prefix_expressions() {
    check(
        r#"fn expressions(){42;0xff;0b1010;0o755;"text";true;false;nil;::root::value;super::value;!~+-42;}"#,
        expect![[r#"
            fn expressions() {
                42;
                0xff;
                0b1010;
                0o755;
                "text";
                true;
                false;
                nil;
                ::root::value;
                super::value;
                !~+-42;
            }
        "#]],
    );
}

#[test]
fn collection_and_struct_expressions() {
    check(
        "fn expressions(){(first);(first,second,);[];[first,...rest,];Example{};Example{first,second:value,};{let value=1;value};const{1}}",
        expect![[r#"
            fn expressions() {
                (first);
                (first, second);
                [];
                [first, ...rest];
                Example {};
                Example { first, second: value };
                {
                    let value = 1;
                    value
                };
                const {
                    1
                }
            }
        "#]],
    );
}

#[test]
fn calls_generics_spreads_and_postfix_expressions() {
    check(
        "fn expressions(value:Int,rest:List<Int>){::factory::make::<Int>(value,...rest,);value.field.next;value as Int;value is Int}",
        expect![[r#"
            fn expressions(value: Int, rest: List<Int>) {
                ::factory::make::<Int>(value, ...rest);
                value.field.next;
                value as Int;
                value is Int
            }
        "#]],
    );
}

#[test]
fn lambda_and_conditional_expression_forms() {
    check(
        "fn expressions(value:Int){let identity=fn<T>(item:T):T=>item;let pair=fn(...[first,...rest])=>(first,rest);let result=if value>0{1}else if value<0{-1}else{0};result}",
        expect![[r#"
            fn expressions(value: Int) {
                let identity = fn<T>(item: T): T => item;
                let pair = fn(...[first, ...rest]) => (first, rest);
                let result = if value > 0 {
                    1
                } else if value < 0 {
                    -1
                } else {
                    0
                };
                result
            }
        "#]],
    );
}

#[test]
fn type_forms() {
    check(
        r#"type Paths=::root::Type<super::Value>;type Literals=42|"text"|true|nil;type Group=(Int);type Pair=(Int,String,);type List=[Int,...String,];type Callback=fn<T>(value:T,...rest:[T])->T;"#,
        expect![[r#"
            type Paths = ::root::Type<super::Value>;
            type Literals = 42 | "text" | true | nil;
            type Group = (Int);
            type Pair = (Int, String);
            type List = [Int, ...String];
            type Callback = fn<T>(value: T, ...rest: [T]) -> T;
        "#]],
    );
}

#[test]
fn binding_forms() {
    check(
        "fn bindings((left,right,):(Int,Int),...[first,...rest]:[Int,...Int],{name:alias,...remaining}:Any){let (a,b,)=(left,right);let [head,...tail]=[first,...rest];let {value:renamed,...others}=remaining;renamed}",
        expect![[r#"
            fn bindings(
                (left, right): (Int, Int),
                ...[first, ...rest]: [Int, ...Int],
                { name: alias, ...remaining }: Any,
            ) {
                let (a, b) = (left, right);
                let [head, ...tail] = [first, ...rest];
                let { value: renamed, ...others } = remaining;
                renamed
            }
        "#]],
    );
}

#[test]
fn every_binary_operator_has_canonical_spacing() {
    check(
        "fn operators(a:Int,b:Int){a||b;a&&b;a==b;a!=b;a<b;a>b;a<=b;a>=b;a|b;a^b;a&b;a<<b;a>>b;a>>>b;a+b;a-b;a*b;a/b;a%b}",
        expect![[r#"
            fn operators(a: Int, b: Int) {
                a || b;
                a && b;
                a == b;
                a != b;
                a < b;
                a > b;
                a <= b;
                a >= b;
                a | b;
                a ^ b;
                a & b;
                a << b;
                a >> b;
                a >>> b;
                a + b;
                a - b;
                a * b;
                a / b;
                a % b
            }
        "#]],
    );
}

#[test]
fn flat_trailing_commas_are_removed_everywhere() {
    check(
        "struct Item<T,>{value:T,}fn example<T,>(value:T,)->(T,T,){let pair=(value,value,);let list=[value,];let item=Item{value:value,};consume::<T,>(pair,list,item,)}",
        expect![[r#"
            struct Item<T> {
                value: T,
            }

            fn example<T>(value: T) -> (T, T) {
                let pair = (value, value);
                let list = [value];
                let item = Item { value: value };
                consume::<T>(pair, list, item)
            }
        "#]],
    );
}

#[test]
fn broken_lists_add_trailing_commas_everywhere() {
    let options = FormatOptions {
        max_width: 28,
        ..FormatOptions::default()
    };
    let output = format_source(
        "struct LongStruct<T>{first:FirstLongType,second:SecondLongType}fn call(first:FirstLongType,second:SecondLongType){LongStruct::<FirstLongType>{first:first,second:second};consume(first,second);[first,second]}",
        &options,
    )
    .unwrap();
    expect![[r#"
        struct LongStruct<T> {
            first: FirstLongType,
            second: SecondLongType,
        }

        fn call(
            first: FirstLongType,
            second: SecondLongType,
        ) {
            LongStruct::<
                FirstLongType,
            > {
                first: first,
                second: second,
            };
            consume(first, second);
            [first, second]
        }
    "#]]
    .assert_eq(&output);
    assert_eq!(format_source(&output, &options).unwrap(), output);
}

#[test]
fn comment_and_blank_line_boundaries() {
    check(
        "//! file\n\n/* before */const VALUE:Int=1;/* between */\n\nfn main(){let list=[/* open */1,/* item */2/* close */];let value=1/* before op */+/* after op */2;// trailing\n/* own line */\nvalue}// eof",
        expect![[r#"
            //! file

            /* before */ const VALUE: Int = 1; /* between */

            fn main() {
                let list = [/* open */ 1, /* item */ 2 /* close */ ];
                let value = 1 /* before op */ + /* after op */ 2; // trailing
                /* own line */
                value
            } // eof
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
fn custom_indentation_width_is_applied_consistently() {
    let options = FormatOptions {
        max_width: 24,
        indent_width: 2,
    };
    let output = format_source(
        "fn main(){if first_condition&&second_condition{let values=[first_condition,second_condition];values}}",
        &options,
    )
    .unwrap();
    expect![[r#"
        fn main() {
          if first_condition
            && second_condition
          {
            let values = [
              first_condition,
              second_condition,
            ];
            values
          }
        }
    "#]]
    .assert_eq(&output);
    assert_eq!(format_source(&output, &options).unwrap(), output);
}

#[test]
fn empty_constructs_are_compact() {
    check(
        "mod empty{}struct Unit{}fn noop(){}fn values(){empty();[];Unit{};{}}",
        expect![[r#"
            mod empty {}

            struct Unit {}

            fn noop() {}

            fn values() {
                empty();
                [];
                Unit {};
                {}
            }
        "#]],
    );
}

#[test]
fn operator_precedence_and_grouping_are_preserved() {
    check(
        "fn precedence(a:Int,b:Int,c:Int,d:Int){a+b*c<<d&a^b|c==d&&a||b;(a+b)*(c-d);a-(b-c);a/(b*c)}",
        expect![[r#"
            fn precedence(a: Int, b: Int, c: Int, d: Int) {
                a + b * c << d & a ^ b | c == d && a || b;
                (a + b) * (c - d);
                a - (b - c);
                a / (b * c)
            }
        "#]],
    );
}

#[test]
fn comparison_operator_wrap_matrix() {
    let options = FormatOptions {
        max_width: 27,
        ..FormatOptions::default()
    };
    let output = format_source(
        "fn compare(){first_long_value==second_long_value;first_long_value!=second_long_value;first_long_value<=second_long_value;first_long_value>=second_long_value}",
        &options,
    )
    .unwrap();
    expect![[r#"
        fn compare() {
            first_long_value
                == second_long_value;
            first_long_value
                != second_long_value;
            first_long_value
                <= second_long_value;
            first_long_value
                >= second_long_value
        }
    "#]]
    .assert_eq(&output);
    assert_eq!(format_source(&output, &options).unwrap(), output);
}

#[test]
fn comments_inside_broken_delimiters() {
    let options = FormatOptions {
        max_width: 32,
        ..FormatOptions::default()
    };
    let output = format_source(
        "fn comments(){consume(first_argument,// first\nsecond_argument,/* third */third_argument);[first_argument,// spread\n...remaining_arguments]}",
        &options,
    )
    .unwrap();
    expect![[r#"
        fn comments() {
            consume(
                first_argument, // first
                second_argument, /* third */ third_argument,
            );
            [
                first_argument, // spread
                ...remaining_arguments,
            ]
        }
    "#]]
    .assert_eq(&output);
    assert_eq!(format_source(&output, &options).unwrap(), output);
}

#[test]
fn long_types_and_bindings_break_consistently() {
    let options = FormatOptions {
        max_width: 36,
        ..FormatOptions::default()
    };
    let output = format_source(
        "fn destructure({first:first_alias,second:second_alias,...remaining}:VeryLongContainerType)->fn(first:FirstLongType,second:SecondLongType)->VeryLongResultType{first_alias}",
        &options,
    )
    .unwrap();
    expect![[r#"
        fn destructure(
            {
                first: first_alias,
                second: second_alias,
                ...remaining,
            }: VeryLongContainerType,
        ) -> fn(
            first: FirstLongType,
            second: SecondLongType,
        ) -> VeryLongResultType {
            first_alias
        }
    "#]]
    .assert_eq(&output);
    assert_eq!(format_source(&output, &options).unwrap(), output);
}

#[test]
fn comments_between_if_else_branches_stay_attached() {
    check(
        "fn choose(value:Int)->Int{if value>0{// positive\n1}else if value<0{/* negative */-1}else{// zero\n0}}",
        expect![[r#"
            fn choose(value: Int) -> Int {
                if value > 0 {
                    // positive
                    1
                } else if value < 0 {
                    /* negative */ -1
                } else {
                    // zero
                    0
                }
            }
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
fn excessive_blank_lines_are_normalized_around_comments() {
    check(
        "const FIRST:Int=1;\n\n\n// second group\n\n\nconst SECOND:Int=2;\n\n\n\nfn main(){FIRST+SECOND}",
        expect![[r#"
            const FIRST: Int = 1;

            // second group

            const SECOND: Int = 2;

            fn main() {
                FIRST + SECOND
            }
        "#]],
    );
}

#[test]
fn malformed_input_matrix_is_rejected() {
    for source in [
        "fn",
        "fn main(",
        "fn main(){",
        "const VALUE:",
        "type Value =",
        "struct Value { field: }",
        "import root::{value",
        "fn main(){let = 1;}",
    ] {
        assert!(
            matches!(
                format_source(source, &FormatOptions::default()),
                Err(FormatError::Parse { .. })
            ),
            "expected parse error for {source:?}"
        );
    }
}

#[test]
fn very_narrow_width_still_produces_valid_idempotent_output() {
    let options = FormatOptions {
        max_width: 8,
        indent_width: 2,
    };
    let output = format_source(
        "fn main(value:LongType)->LongType{consume(value,value)}",
        &options,
    )
    .unwrap();
    expect![[r#"
        fn main(
          value: LongType,
        ) -> LongType {
          consume(
            value,
            value,
          )
        }
    "#]]
    .assert_eq(&output);
    assert_eq!(format_source(&output, &options).unwrap(), output);
}

#[test]
fn nested_generics_and_shift_operators_remain_unambiguous() {
    check(
        "type Nested=Outer<Middle<Inner<Int>>>;fn main(value:Int){build::<Outer<Inner<Int>>>(value);value>>2;value>>>3}",
        expect![[r#"
            type Nested = Outer<Middle<Inner<Int>>>;

            fn main(value: Int) {
                build::<Outer<Inner<Int>>>(value);
                value >> 2;
                value >>> 3
            }
        "#]],
    );
}
