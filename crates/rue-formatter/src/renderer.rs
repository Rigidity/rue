use crate::{
    FormatOptions,
    document::{Doc, LineKind},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Flat,
    Broken,
}

#[derive(Debug, Clone, Copy)]
struct Command<'a> {
    indent: usize,
    mode: Mode,
    doc: &'a Doc,
}

pub(crate) fn render(doc: &Doc, options: &FormatOptions) -> String {
    let mut output = String::new();
    let mut column = 0;
    let mut commands = vec![Command {
        indent: 0,
        mode: Mode::Broken,
        doc,
    }];

    while let Some(command) = commands.pop() {
        match command.doc {
            Doc::Nil => {}
            Doc::Text(text) => {
                output.push_str(text);
                column += text.chars().count();
            }
            Doc::Concat(docs) => {
                for doc in docs.iter().rev() {
                    commands.push(Command {
                        indent: command.indent,
                        mode: command.mode,
                        doc,
                    });
                }
            }
            Doc::Line(LineKind::Soft) if command.mode == Mode::Flat => {
                output.push(' ');
                column += 1;
            }
            Doc::PreferredBreak {
                doc,
                indent_on_break,
            } => {
                if column < options.max_width
                    && fits_broken_prefix(options.max_width - column - 1, doc, &commands)
                {
                    output.push(' ');
                    column += 1;
                    commands.push(Command {
                        indent: command.indent,
                        mode: command.mode,
                        doc,
                    });
                } else {
                    output.push('\n');
                    let indent = command.indent
                        + if *indent_on_break {
                            options.indent_width
                        } else {
                            0
                        };
                    output.extend(std::iter::repeat_n(' ', indent));
                    column = indent;
                    commands.push(Command {
                        indent,
                        mode: command.mode,
                        doc,
                    });
                }
            }
            Doc::Line(kind) => {
                output.push('\n');
                if *kind == LineKind::Empty && !output.ends_with("\n\n") {
                    output.push('\n');
                }
                output.extend(std::iter::repeat_n(' ', command.indent));
                column = command.indent;
            }
            Doc::Indent(doc) => commands.push(Command {
                indent: command.indent + options.indent_width,
                mode: command.mode,
                doc,
            }),
            Doc::Group(doc) => {
                let mode = if has_forced_line(doc)
                    || !fits(
                        options.max_width.saturating_sub(column),
                        command.indent,
                        doc,
                        &commands,
                    ) {
                    Mode::Broken
                } else {
                    Mode::Flat
                };
                commands.push(Command {
                    indent: command.indent,
                    mode,
                    doc,
                });
            }
            Doc::IfBreak { broken, flat } => commands.push(Command {
                indent: command.indent,
                mode: command.mode,
                doc: if command.mode == Mode::Broken {
                    broken
                } else {
                    flat
                },
            }),
        }
    }

    normalize_output(&output)
}

fn has_forced_line(doc: &Doc) -> bool {
    match doc {
        Doc::Nil | Doc::Text(_) | Doc::Line(LineKind::Soft) => false,
        Doc::Line(LineKind::Hard | LineKind::Empty) => true,
        Doc::Concat(docs) => docs.iter().any(has_forced_line),
        Doc::Indent(doc) | Doc::Group(doc) => has_forced_line(doc),
        Doc::PreferredBreak { doc, .. } => has_forced_line(doc),
        Doc::IfBreak { flat, .. } => has_forced_line(flat),
    }
}

fn fits(
    mut remaining: usize,
    initial_indent: usize,
    doc: &Doc,
    remaining_commands: &[Command<'_>],
) -> bool {
    let mut commands = remaining_commands.to_vec();
    commands.push(Command {
        indent: initial_indent,
        mode: Mode::Flat,
        doc,
    });

    while let Some(command) = commands.pop() {
        match command.doc {
            Doc::Nil => {}
            Doc::Text(text) => {
                let width = text.chars().count();
                if width > remaining {
                    return false;
                }
                remaining -= width;
            }
            Doc::Concat(docs) => {
                for doc in docs.iter().rev() {
                    commands.push(Command {
                        indent: command.indent,
                        mode: command.mode,
                        doc,
                    });
                }
            }
            Doc::Line(LineKind::Soft) if command.mode == Mode::Flat => {
                if remaining == 0 {
                    return false;
                }
                remaining -= 1;
            }
            Doc::Line(LineKind::Soft) => return true,
            Doc::Line(LineKind::Hard | LineKind::Empty) => return true,
            Doc::Indent(doc) | Doc::Group(doc) => commands.push(Command {
                indent: command.indent,
                mode: command.mode,
                doc,
            }),
            Doc::PreferredBreak { doc, .. } => {
                if remaining == 0 {
                    return false;
                }
                remaining -= 1;
                commands.push(Command {
                    indent: command.indent,
                    mode: command.mode,
                    doc,
                });
            }
            Doc::IfBreak { broken, flat } => commands.push(Command {
                indent: command.indent,
                mode: command.mode,
                doc: if command.mode == Mode::Broken {
                    broken
                } else {
                    flat
                },
            }),
        }
    }

    true
}

fn fits_broken_prefix(mut remaining: usize, doc: &Doc, remaining_commands: &[Command<'_>]) -> bool {
    let mut commands = remaining_commands.to_vec();
    commands.push(Command {
        indent: 0,
        mode: Mode::Broken,
        doc,
    });

    while let Some(command) = commands.pop() {
        match command.doc {
            Doc::Nil => {}
            Doc::Text(text) => {
                let width = text.chars().count();
                if width > remaining {
                    return false;
                }
                remaining -= width;
            }
            Doc::Concat(docs) => {
                for doc in docs.iter().rev() {
                    commands.push(Command {
                        indent: command.indent,
                        mode: Mode::Broken,
                        doc,
                    });
                }
            }
            Doc::Line(_) => return true,
            Doc::Indent(doc) | Doc::Group(doc) => commands.push(Command {
                indent: command.indent,
                mode: Mode::Broken,
                doc,
            }),
            Doc::PreferredBreak { .. } => return true,
            Doc::IfBreak { broken, .. } => commands.push(Command {
                indent: command.indent,
                mode: Mode::Broken,
                doc: broken,
            }),
        }
    }

    true
}

fn normalize_output(output: &str) -> String {
    let mut normalized = output.to_string();

    while normalized.ends_with("\n\n\n") {
        normalized.pop();
    }

    if !normalized.ends_with('\n') {
        normalized.push('\n');
    }

    normalized
}
