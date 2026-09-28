use crate::emojis;
use crate::parser::{parse, GrammarItem, ParseError};

/// Creates a Rustdoc string from a Doxygen string.
///
/// # Errors
///
/// This function can error if there are missing parts of a given Doxygen annotation (like `@param`
/// missing the variable name)
pub fn rustdoc(input: String) -> Result<String, ParseError> {
    let parsed = parse(input)?;
    let mut result = String::new();
    let mut block = Block::default();
    let mut already_added_params = false;
    let mut already_added_returns = false;
    let mut already_added_throws = false;
    let mut group_started = false;

    for item in parsed {
        match item {
            GrammarItem::Notation { meta, params, tag } => {
                let is_inline = is_inline_command(&tag);
                let (str, (added_param, added_return, added_throws)) = generate_notation(
                    tag,
                    meta,
                    params,
                    (
                        already_added_params,
                        already_added_returns,
                        already_added_throws,
                    ),
                );
                if added_param {
                    already_added_params = true;
                }

                if added_return {
                    already_added_returns = true;
                }

                if added_throws {
                    already_added_throws = true;
                }

                if is_inline {
                    block.body += &str;
                } else {
                    result += &block.render();
                    block = Block {
                        head: str,
                        body: String::new(),
                    };
                }
            }
            GrammarItem::Text(v) => {
                if group_started {
                    block.body += &v.replacen("*", "", 1)
                } else {
                    block.body += &v
                }
            }
            // See <https://stackoverflow.com/a/40354789>
            GrammarItem::GroupStart => {
                group_started = true;
                block.body += "# ";
            }
            GrammarItem::GroupEnd => {
                group_started = false;
            }
        }
    }
    result += &block.render();

    Ok(result)
}

/// A block command, like `@param`, with the text up to the next block command.
#[derive(Default)]
struct Block {
    /// The rendered command.
    head: String,
    body: String,
}

impl Block {
    fn render(&self) -> String {
        format!("{}{}", self.head, self.body)
    }
}

/// Whether `tag` is a command that is part of the surrounding text, rather than starting a block.
fn is_inline_command(tag: &str) -> bool {
    matches!(
        tag,
        "a" | "b" | "c" | "p" | "e" | "em" | "emoji" | "sa" | "see" | "code" | "endcode"
    )
}

fn generate_notation(
    tag: String,
    meta: Vec<String>,
    params: Vec<String>,
    (already_params, already_returns, already_throws): (bool, bool, bool),
) -> (String, (bool, bool, bool)) {
    let mut new_param = false;
    let mut new_return = false;
    let mut new_throw = false;

    (
        match tag.as_str() {
            "param" => {
                let param = params.get(0);
                new_param = true;
                let mut str = if !already_params {
                    "# Arguments\n\n".into()
                } else {
                    // In Markdown we need 2 newlines to get a newline in
                    // the rendered form, so we inject another newline here!
                    String::from("\n")
                };

                str += &if let Some(param) = param {
                    if meta.is_empty() {
                        format!("* `{param}` -")
                    } else {
                        if let Some(second) = meta.get(1) {
                            format!(
                                "* `{}` (direction {}, {}) -",
                                param,
                                meta.get(0).unwrap(),
                                second
                            )
                        } else {
                            format!("* `{}` (direction {}) -", param, meta.get(0).unwrap())
                        }
                    }
                } else {
                    String::new()
                };

                str
            }
            "a" | "e" | "em" => params
                .first()
                .map(|word| format!("_{word}_"))
                .unwrap_or_default(),
            "b" => params
                .first()
                .map(|word| format!("**{word}**"))
                .unwrap_or_default(),
            "c" | "p" => params
                .first()
                .map(|word| format!("`{word}`"))
                .unwrap_or_default(),
            "emoji" => params
                .first()
                .map(|word| {
                    emojis::EMOJIS
                        .get(&word.replace(':', ""))
                        .map_or_else(|| word.clone(), |emoji| emoji.to_string())
                })
                .unwrap_or_default(),
            // Without a word, the reference is in the following text, like in `@see {@link Foo}`.
            "sa" | "see" => params
                .first()
                .map(|code_ref| format!("[`{code_ref}`]"))
                .unwrap_or_default(),
            "retval" => {
                new_return = true;
                let mut str = if !already_returns {
                    "# Returns\n\n".into()
                } else {
                    String::from("\n")
                };

                str += &match params.first() {
                    Some(var) => format!("* `{var}` -"),
                    None => String::from("* "),
                };
                str
            }
            "returns" | "return" | "result" => {
                new_return = true;
                let mut str = if !already_returns {
                    "\n# Returns\n\n".into()
                } else {
                    String::from("\n")
                };

                str += "* ";
                str
            }
            "throw" | "throws" | "exception" => {
                new_throw = true;
                let mut str = if !already_throws {
                    "# Throws\n\n".into()
                } else {
                    String::from("\n")
                };

                str += &match params.first() {
                    Some(exception) => format!("* [`{exception}`] -"),
                    None => String::from("* "),
                };
                str
            }
            "note" => String::from("\n**Note:** "),
            "since" => String::from("\nAvailable since API-level: "),
            "syscap" => String::from("\nRequired System Capabilities: "),
            "version" => String::from("\nVersion: "),
            "deprecated" => String::from("\n**Deprecated** "),
            "useinstead" => String::from("\n**Use instead:** "),
            "remark" | "remarks" => String::from("> "),
            "par" => String::from("# "),
            "details" | "pre" | "post" => String::from("\n\n"),
            "brief" | "short" => String::new(),
            "code" => {
                let lang = params.first().map(|p| p.as_str()).unwrap_or_default();
                let lang = lang.strip_prefix('.').unwrap_or(lang);
                format!("```{lang}")
            }
            "endcode" => String::from("```"),
            _ => String::new(),
        },
        (new_param, new_return, new_throw),
    )
}

#[cfg(test)]
mod test {
    use super::*;

    macro_rules! test_rustdoc {
        ($input:literal, $expected:literal) => {
            let result = $crate::generator::rustdoc($input.into()).unwrap();
            assert_eq!(result, $expected);
        };
    }

    #[test]
    fn unknown_annotation() {
        test_rustdoc!("@thisdoesntexist Example doc", "Example doc");
    }

    #[test]
    fn param_with_direction() {
        test_rustdoc!(
            "@param[in] example This insane thing.",
            "# Arguments\n\n* `example` (direction in) - This insane thing."
        );

        test_rustdoc!(
            "@param[in,out] example This insane thing.",
            "# Arguments\n\n* `example` (direction in, out) - This insane thing."
        );

        test_rustdoc!(
            "@param[out,in] example This insane thing.",
            "# Arguments\n\n* `example` (direction in, out) - This insane thing."
        );
    }

    #[test]
    fn param_without_direction() {
        test_rustdoc!(
            "@param example This is definitively an example!",
            "# Arguments\n\n* `example` - This is definitively an example!"
        );
    }

    #[test]
    fn multiple_params() {
        test_rustdoc!(
            "@param example1 This is the first example\n@param[out] example2 This is the second example\n@param[in] example3 This is the third example.",
            "# Arguments\n\n* `example1` - This is the first example\n\n* `example2` (direction out) - This is the second example\n\n* `example3` (direction in) - This is the third example."
        );
    }

    #[test]
    fn italics() {
        test_rustdoc!(
            "This @a thing is without a doubt @e great. @em And you won't tell me otherwise.",
            "This _thing_ is without a doubt _great._ _And_ you won't tell me otherwise."
        );
    }

    #[test]
    fn bold() {
        test_rustdoc!("This is a @b bold claim.", "This is a **bold** claim.");
    }

    #[test]
    fn code_inline() {
        test_rustdoc!(
            "@c u8 is not the same as @p u32",
            "`u8` is not the same as `u32`"
        );
    }

    #[test]
    fn emoji() {
        test_rustdoc!("@emoji :relieved: @emoji :ok_hand:", "😌 👌");
    }

    #[test]
    fn text_styling() {
        test_rustdoc!(
            "This is from @a Italy. ( @b I @c hope @emoji :pray: )",
            "This is from _Italy._ ( **I** `hope` 🙏 )"
        );
    }

    #[test]
    fn brief() {
        test_rustdoc!(
            "@brief This function does things.\n@short This function also does things.",
            "This function does things.\nThis function also does things."
        );
    }

    #[test]
    fn see_also() {
        test_rustdoc!(
            "@sa random_thing @see random_thing_2",
            "[`random_thing`] [`random_thing_2`]"
        );
    }

    #[test]
    fn deprecated() {
        test_rustdoc!(
            "@deprecated This function is deprecated!\n@param example_1 Example 1.",
            "\n**Deprecated** This function is deprecated!\n# Arguments\n\n* `example_1` - Example 1."
        );
    }

    #[test]
    fn useinstead() {
        test_rustdoc!(
            "@deprecated since 20\n@useinstead OH_Foo\n@since 10",
            "\n**Deprecated** since 20\n\n**Use instead:** OH_Foo\n\nAvailable since API-level: 10"
        );
    }

    #[test]
    fn useinstead_multiline() {
        test_rustdoc!(
            "@deprecated since 20\n@useinstead Use OH_Foo,\nOH_Bar separately.\n@since 10",
            "\n**Deprecated** since 20\n\n**Use instead:** Use OH_Foo,\nOH_Bar separately.\n\nAvailable since API-level: 10"
        );
    }

    #[test]
    fn details() {
        test_rustdoc!(
            "@brief This function is insane!\n@details This is an insane function because its functionality and performance is quite astonishing.",
            "This function is insane!\n\n\nThis is an insane function because its functionality and performance is quite astonishing."
        );
    }

    #[test]
    fn paragraph() {
        test_rustdoc!(
            "@par Interesting fact about this function\nThis is a function.",
            "# Interesting fact about this function\nThis is a function."
        );
    }

    #[test]
    fn remark() {
        test_rustdoc!(
            "@remark This things needs to be\n@remark remarked.",
            "> This things needs to be\n> remarked."
        );
    }

    #[test]
    fn returns() {
        test_rustdoc!(
            "@returns A value that should be\n@return used with caution.\n@result And if it's @c -1 ... run.",
            "\n# Returns\n\n* A value that should be\n\n* used with caution.\n\n* And if it's `-1` ... run."
        );
    }

    #[test]
    fn return_value() {
        test_rustdoc!(
            "@retval example1 This return value is great!",
            "# Returns\n\n* `example1` - This return value is great!"
        );
    }

    #[test]
    fn returns_and_return_value() {
        test_rustdoc!(
            "@returns Great values!\n@retval example1 Is this an example?\n@return Also maybe more things (?)",
            "\n# Returns\n\n* Great values!\n\n* `example1` - Is this an example?\n\n* Also maybe more things (?)"
        );

        test_rustdoc!(
            "@returns Great values!\n@return Also maybe more things (?)\n@retval example1 Is this an example?",
            "\n# Returns\n\n* Great values!\n\n* Also maybe more things (?)\n\n* `example1` - Is this an example?"
        );

        test_rustdoc!(
            "@retval example1 Is this an example?\n@returns Great values!\n@return Also maybe more things (?)",
            "# Returns\n\n* `example1` - Is this an example?\n\n* Great values!\n\n* Also maybe more things (?)"
        );
    }

    #[test]
    fn since() {
        test_rustdoc!(
            "@since The bite of '87",
            "\nAvailable since API-level: The bite of '87"
        );
    }

    #[test]
    fn throws() {
        test_rustdoc!(
            "@throw std::io::bonk This is thrown when INSANE things happen.\n@throws std::net::meow This is thrown when BAD things happen.\n@exception std::fs::no This is thrown when NEFARIOUS things happen.",
            "# Throws\n\n* [`std::io::bonk`] - This is thrown when INSANE things happen.\n\n* [`std::net::meow`] - This is thrown when BAD things happen.\n\n* [`std::fs::no`] - This is thrown when NEFARIOUS things happen."
        );
    }

    #[test]
    fn code() {
        test_rustdoc!(
            "@code\nfn main() {\n        test( [1] ); // @code @throw\n@endcode",
            "```\nfn main() {\n        test( [1] ); // @code @throw\n```"
        );
    }

    #[test]
    fn code_with_lang() {
        test_rustdoc!(
            "@code{.rs}\nfn main() {\n        test( [1] ); // @code @throw\n@endcode",
            "```rs\nfn main() {\n        test( [1] ); // @code @throw\n```"
        );
    }

    #[test]
    fn backslashes() {
        test_rustdoc!(
            "The terminating character ('\\0'), \\\\0, \\<b\\>, \\[0, 900\\] and uint8_t\\*.",
            "The terminating character ('\\0'), \\\\0, \\<b\\>, \\[0, 900\\] and uint8_t\\*."
        );
    }

    #[test]
    fn at_sign_in_word() {
        test_rustdoc!(
            "Mail to user@example.com, see de_DE@collation.",
            "Mail to user@example.com, see de_DE@collation."
        );
    }

    #[test]
    fn code_span() {
        test_rustdoc!(
            "Loads `@ohos.app.ability.childProcessManager` with `\\n`.",
            "Loads `@ohos.app.ability.childProcessManager` with `\\n`."
        );
    }

    #[test]
    fn unknown_annotation_in_text() {
        test_rustdoc!(
            "@Custom Dropped\nLoads a module like @ohos.hilog, if @ptrName is NULL. @stable ICU 2.0",
            "Dropped\nLoads a module like @ohos.hilog, if @ptrName is NULL. ICU 2.0"
        );
    }

    #[test]
    fn inline_links() {
        test_rustdoc!(
            "See {@link Foo}, {@link enum Bar}, {@link link Baz}, {@link napi_ok }, {@ link Qux} and {@THE_THING}.",
            "See [`Foo`], [`Bar`], [`Baz`], [`napi_ok`], [`Qux`] and [`THE_THING`]."
        );
    }

    #[test]
    fn inline_link_without_identifier() {
        test_rustdoc!(
            "See {@link water flow items} for {@link \\<path>shape}.",
            "See water flow items for `<path>shape`."
        );
    }

    #[test]
    fn unclosed_inline_link() {
        test_rustdoc!(
            "Copy option {@link ArkUI_CopyOptions.\n\n@since 21",
            "Copy option [`ArkUI_CopyOptions`].\n\n\nAvailable since API-level: 21"
        );
    }

    #[test]
    fn inline_code() {
        test_rustdoc!("Pass {@code a | b}.", "Pass `a | b`.");
    }

    #[test]
    fn missing_arguments() {
        test_rustdoc!("@see {@link Foo}", "[`Foo`]");
        test_rustdoc!("Matches \\p{graph} or @c", "Matches {graph} or ");
        test_rustdoc!("@retval", "# Returns\n\n* ");
        test_rustdoc!("@throw", "# Throws\n\n* ");
    }

    #[test]
    fn groups() {
        test_rustdoc!(
            "@brief Does things.\n@ingroup Background Display\n@since 10",
            "Does things.\n\nAvailable since API-level: 10"
        );
    }

    #[test]
    fn can_parse_example() {
        let example = include_str!("../tests/assets/example-bindgen.rs");
        println!("{}", rustdoc(example.into()).unwrap());
    }
}
