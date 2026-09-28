use crate::lexer::{lex, LexItem};

const OPEN_PAREN: char = '{';
const CLOSED_PAREN: char = '}';

#[derive(Debug, Clone)]
pub enum ParseError {
    UnexpectedEndOfInput,
    UnexpectedInput {
        found: String,
        expected: Vec<String>,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum GrammarItem {
    Notation {
        meta: Vec<String>,
        params: Vec<String>,
        tag: String,
    },
    Text(String),
    GroupStart,
    GroupEnd,
}

enum ParamParser {
    None,
    Whitespace,
    Paren,
    /// The rest of the line, which is dropped.
    Line,
}

pub(crate) fn parse(input: String) -> Result<Vec<GrammarItem>, ParseError> {
    let lexed = lex(input);
    parse_items(lexed)
}

fn parse_items(input: Vec<LexItem>) -> Result<Vec<GrammarItem>, ParseError> {
    let mut grammar_items = vec![];
    let mut param_iter_skip_count = 0;

    for (index, current) in input.iter().enumerate() {
        let rest = &input[index..];
        let next = rest.get(1);

        if param_iter_skip_count > 0 {
            param_iter_skip_count -= 1;
            continue;
        }

        // Do not do any formatting inside of code blocks
        let ends_code = matches!(current, LexItem::At(_))
            && matches!(next, Some(LexItem::Word(v)) if v == "endcode");
        if !ends_code {
            match &mut grammar_items[..] {
                [.., GrammarItem::Notation { tag, .. }] if tag == "code" => {
                    let mut text = String::new();
                    current.push_to(&mut text);

                    grammar_items.push(GrammarItem::Text(text));
                    continue;
                }
                [.., GrammarItem::Notation { tag, .. }, GrammarItem::Text(text)]
                    if tag == "code" =>
                {
                    current.push_to(text);
                    continue;
                }
                _ => {}
            }
        }

        match current {
            LexItem::At(at) => {
                if let Some(next) = next {
                    match next {
                        LexItem::Paren(v) => {
                            match *v {
                                OPEN_PAREN => grammar_items.push(GrammarItem::GroupStart),
                                CLOSED_PAREN => grammar_items.push(GrammarItem::GroupEnd),
                                _ => {
                                    return Err(ParseError::UnexpectedInput {
                                        found: v.to_string(),
                                        expected: vec![OPEN_PAREN.into(), CLOSED_PAREN.into()],
                                    })
                                }
                            }
                            param_iter_skip_count = 1;
                        }
                        LexItem::Word(v) => {
                            // Commands have lowercase names. Something like `@ohos.hilog` or
                            // `@ptrName` within a sentence is text.
                            let is_text = !v.starts_with("param")
                                && !v.chars().all(|c| c.is_ascii_lowercase())
                                && is_mid_line(&input[..index]);
                            if is_text {
                                push_text(&mut grammar_items, &format!("{at}{v}"));
                                param_iter_skip_count = 1;
                                continue;
                            }

                            let mut meta = vec![];
                            let (content, param_parser) = if v.starts_with("param") {
                                let value = v.split('[').collect::<Vec<_>>();
                                match value.get(1) {
                                    Some(&"in]") => meta.push("in".into()),
                                    Some(&"out]") => meta.push("out".into()),
                                    Some(&"in,out]") | Some(&"out,in]") => {
                                        meta.push("in".into());
                                        meta.push("out".into());
                                    }
                                    _ => match value.get(1) {
                                        None => {}
                                        Some(v) => {
                                            return Err(ParseError::UnexpectedInput {
                                                found: v.to_string(),
                                                expected: vec!["in]".into(), "out]".into()],
                                            })
                                        }
                                    },
                                }

                                ("param", ParamParser::Whitespace)
                            } else {
                                let param_parser = match v.as_str() {
                                    "a" | "b" | "c" | "p" | "emoji" | "e" | "em" | "def"
                                    | "class" | "category" | "concept" | "enum" | "example"
                                    | "extends" | "file" | "retval" | "exception" | "throw"
                                    | "throws" => ParamParser::Whitespace,
                                    "code" => ParamParser::Paren,
                                    "ingroup" | "addtogroup" | "defgroup" | "weakgroup" => {
                                        ParamParser::Line
                                    }
                                    _ => ParamParser::None,
                                };
                                (v.as_str(), param_parser)
                            };

                            let param = match param_parser {
                                ParamParser::None | ParamParser::Line => None,
                                ParamParser::Whitespace => rest
                                    .iter()
                                    .enumerate()
                                    .skip(2)
                                    .find(|(_, next)| !matches!(next, LexItem::Whitespace(_)))
                                    .and_then(|(skip, next)| match next {
                                        LexItem::Word(word) => Some((skip, word)),
                                        _ => None,
                                    }),
                                ParamParser::Paren => match &rest {
                                    [_, _, LexItem::Paren('{'), LexItem::Word(word), LexItem::Paren('}'), ..] => {
                                        Some((4, word))
                                    }
                                    [_, _, LexItem::Whitespace(_), LexItem::Paren('{'), LexItem::Word(word), LexItem::Paren('}'), ..] => {
                                        Some((5, word))
                                    }
                                    _ => None,
                                },
                            };

                            let params = if let Some((skip, word)) = param {
                                param_iter_skip_count = skip;
                                vec![word.into()]
                            } else {
                                param_iter_skip_count = 1;
                                vec![]
                            };
                            if let ParamParser::Line = param_parser {
                                param_iter_skip_count = rest
                                    .iter()
                                    .position(|item| *item == LexItem::NewLine)
                                    .unwrap_or(rest.len())
                                    - 1;
                            }

                            grammar_items.push(GrammarItem::Notation {
                                meta,
                                params,
                                tag: content.into(),
                            });

                            if content == "endcode" {
                                grammar_items.push(GrammarItem::Text("".into()));
                            }
                        }
                        _ => {}
                    }
                }
            }
            LexItem::Word(v) => push_text(&mut grammar_items, v),
            LexItem::Whitespace(_) => {
                if let Some(prev) = grammar_items.last_mut() {
                    match prev {
                        GrammarItem::Text(text) if text.ends_with(' ') => {}
                        GrammarItem::Text(text) => *text += " ",
                        GrammarItem::Notation { params, .. } if !params.is_empty() => {
                            grammar_items.push(GrammarItem::Text(" ".into()))
                        }
                        _ => grammar_items.push(GrammarItem::Text("".into())),
                    }
                } else {
                    grammar_items.push(GrammarItem::Text(" ".into()));
                }
            }
            LexItem::NewLine => {
                if let Some(GrammarItem::Text(text)) = grammar_items.last_mut() {
                    *text += "\n"
                }
            }
            LexItem::Paren(v) => {
                if *v == OPEN_PAREN && matches!(next, Some(LexItem::At(_))) {
                    let (end, closed) = inline_command_end(rest);
                    let mut content = String::new();
                    for item in &rest[2..end] {
                        item.push_to(&mut content);
                    }
                    grammar_items.push(GrammarItem::Text(inline_command(&content)));
                    param_iter_skip_count = if closed { end } else { end - 1 };
                    continue;
                }
                push_text(&mut grammar_items, &v.to_string());
            }
        }
    }

    Ok(grammar_items)
}

/// Renders an inline command like `{@link Foo}`, given its content without the braces and the `@`.
fn inline_command(content: &str) -> String {
    let content = content.trim();
    let (command, argument) = content
        .split_once(char::is_whitespace)
        .map_or((content, ""), |(command, argument)| {
            (command, argument.trim())
        });
    match command {
        "code" | "Code" => format!("`{argument}`"),
        "link" | "Link" => link(argument),
        // Like `{@THE_THING}`.
        _ if argument.is_empty() => link(command),
        _ => link(argument),
    }
}

/// The index of the `}` that closes the inline command at the start of `rest`, and whether it is
/// closed. An unclosed command, like `{@link Foo.`, ends with its paragraph or before a line that
/// starts with a command.
fn inline_command_end(rest: &[LexItem]) -> (usize, bool) {
    for (index, item) in rest.iter().enumerate() {
        match item {
            LexItem::Paren(CLOSED_PAREN) => return (index, true),
            LexItem::NewLine => {
                let indent = rest[index + 1..]
                    .iter()
                    .take_while(|item| matches!(item, LexItem::Whitespace(_)))
                    .count();
                if matches!(
                    rest.get(index + 1 + indent),
                    Some(LexItem::NewLine | LexItem::At(_))
                ) {
                    return (index, false);
                }
            }
            _ => {}
        }
    }
    (rest.len(), false)
}

fn link(target: &str) -> String {
    let target = ["enum ", "struct ", "union ", "link "]
        .iter()
        .find_map(|prefix| target.strip_prefix(prefix))
        .unwrap_or(target)
        .trim();
    let identifier = target.trim_end_matches(['.', ',', ';', ':']);
    let punctuation = &target[identifier.len()..];
    if identifier.is_empty() || target.contains(char::is_whitespace) {
        // Not an identifier, but e.g. the title of a document.
        target.to_string()
    } else if identifier.contains('\\') {
        // Something escaped, like `\<path>shape`.
        format!("`{}`{punctuation}", identifier.replace('\\', ""))
    } else {
        format!("[`{identifier}`]{punctuation}")
    }
}

fn push_text(grammar_items: &mut Vec<GrammarItem>, value: &str) {
    match grammar_items.last_mut() {
        Some(GrammarItem::Text(text)) => *text += value,
        _ => grammar_items.push(GrammarItem::Text(value.into())),
    }
}

/// Whether `preceding` ends with a line that contains more than whitespace.
fn is_mid_line(preceding: &[LexItem]) -> bool {
    preceding
        .iter()
        .rev()
        .take_while(|item| **item != LexItem::NewLine)
        .any(|item| !matches!(item, LexItem::Whitespace(_)))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn simple_notation() {
        let result = parse("@name Memory Management".into()).unwrap();
        assert_eq!(
            result,
            vec![
                GrammarItem::Notation {
                    meta: vec![],
                    params: vec![],
                    tag: "name".into(),
                },
                GrammarItem::Text("Memory Management".into())
            ]
        );
    }

    #[test]
    pub fn paren_in_notation() {
        let result = parse("@note hoge_t = {a, b, c}".into()).unwrap();
        assert_eq!(
            result,
            vec![
                GrammarItem::Notation {
                    meta: vec![],
                    params: vec![],
                    tag: "note".into(),
                },
                GrammarItem::Text("hoge_t = {a, b, c}".into())
            ]
        );
    }

    #[test]
    pub fn param() {
        let result =
            parse("@param[in] random This is, without a doubt, a random argument.".into()).unwrap();
        assert_eq!(
            result,
            vec![
                GrammarItem::Notation {
                    meta: vec!["in".into()],
                    params: vec!["random".into()],
                    tag: "param".into(),
                },
                GrammarItem::Text(" This is, without a doubt, a random argument.".into())
            ]
        );
    }

    #[test]
    pub fn param_tabs() {
        let result =
            parse("@param[in]\trandom\t\t\tThis is, without a doubt, a random argument.".into())
                .unwrap();
        assert_eq!(
            result,
            vec![
                GrammarItem::Notation {
                    meta: vec!["in".into()],
                    params: vec!["random".into()],
                    tag: "param".into(),
                },
                GrammarItem::Text(" This is, without a doubt, a random argument.".into())
            ]
        );
    }

    #[test]
    pub fn groups() {
        let result = parse("@{\n* @name Memory Management\n@}".into()).unwrap();
        assert_eq!(
            result,
            vec![
                GrammarItem::GroupStart,
                GrammarItem::Text("* ".into()),
                GrammarItem::Notation {
                    meta: vec![],
                    params: vec![],
                    tag: "name".into(),
                },
                GrammarItem::Text("Memory Management\n".into()),
                GrammarItem::GroupEnd
            ]
        );
    }

    #[test]
    pub fn trims_param_texts() {
        let result = parse(
            "@param[in]           var                                         Example description"
                .into(),
        )
        .unwrap();
        assert_eq!(
            result,
            vec![
                GrammarItem::Notation {
                    meta: vec!["in".into()],
                    params: vec!["var".into()],
                    tag: "param".into(),
                },
                GrammarItem::Text(" Example description".into())
            ]
        )
    }

    #[test]
    pub fn code() {
        let result = parse("@code\nfn main() {}\n@endcode".into()).unwrap();

        assert_eq!(
            result,
            vec![
                GrammarItem::Notation {
                    meta: vec![],
                    params: vec![],
                    tag: "code".into(),
                },
                GrammarItem::Text("\nfn main() {}\n".into()),
                GrammarItem::Notation {
                    meta: vec![],
                    params: vec![],
                    tag: "endcode".into(),
                },
                GrammarItem::Text("".into())
            ]
        )
    }

    #[test]
    pub fn code_with_param() {
        let result = parse("@code{.py}\nfn main() {}\n@endcode".into()).unwrap();

        assert_eq!(
            result,
            vec![
                GrammarItem::Notation {
                    meta: vec![],
                    params: vec![".py".into()],
                    tag: "code".into(),
                },
                GrammarItem::Text("\nfn main() {}\n".into()),
                GrammarItem::Notation {
                    meta: vec![],
                    params: vec![],
                    tag: "endcode".into(),
                },
                GrammarItem::Text("".into())
            ]
        )
    }

    #[test]
    pub fn code_with_args() {
        let result = parse("@code\nfn main() {}\n@endcode\n\n@param[in] a - a".into()).unwrap();

        assert_eq!(
            result,
            vec![
                GrammarItem::Notation {
                    meta: vec![],
                    params: vec![],
                    tag: "code".into(),
                },
                GrammarItem::Text("\nfn main() {}\n".into()),
                GrammarItem::Notation {
                    meta: vec![],
                    params: vec![],
                    tag: "endcode".into(),
                },
                GrammarItem::Text("\n\n".into()),
                GrammarItem::Notation {
                    meta: vec!["in".into()],
                    params: vec!["a".into()],
                    tag: "param".into()
                },
                GrammarItem::Text(" - a".into())
            ]
        )
    }
}
