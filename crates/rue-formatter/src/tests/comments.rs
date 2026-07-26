use super::*;

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
fn separated_file_banner_stays_detached_from_first_item() {
    check(
        "// This puzzle has not been audited.\n\n// Item documentation.\nstruct Example{value:Int}",
        expect![[r#"
            // This puzzle has not been audited.

            // Item documentation.
            struct Example {
                value: Int,
            }
        "#]],
    );
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
