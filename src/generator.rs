use crate::emojis;
use crate::html_lists::{has_balanced_lists, lists_to_markdown};
use crate::parser::{parse, GrammarItem, ParseError};

/// Creates a Rustdoc string from a Doxygen string.
///
/// # Errors
///
/// This function can error if there are missing parts of a given Doxygen annotation (like `@param`
/// missing the variable name)
pub fn rustdoc(input: String) -> Result<String, ParseError> {
    // Unbalanced lists can't be converted reliably, so keep them as they are.
    let convert_lists = has_balanced_lists(&input);
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
                    tag.clone(),
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
                    result += &block.render(convert_lists);
                    block = Block {
                        tag,
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
    result += &block.render(convert_lists);

    Ok(result)
}

/// A block command, like `@param`, with the text up to the next block command.
#[derive(Default)]
struct Block {
    /// The command, which is empty for the text before the first block command.
    tag: String,
    /// The rendered command.
    head: String,
    body: String,
}

impl Block {
    fn render(&self, convert_lists: bool) -> String {
        let body = match self.tag.as_str() {
            "sa" | "see" => link_references(self.body.trim_start_matches(' ')),
            _ => self.body.clone(),
        };
        let text = format!("{}{}", self.head, body);
        if convert_lists && text.contains("<ul>") && has_balanced_lists(&text) {
            lists_to_markdown(&text, self.is_list_item())
        } else {
            text
        }
    }

    /// Whether the block is an item of the arguments, returns or throws lists.
    fn is_list_item(&self) -> bool {
        matches!(
            self.tag.as_str(),
            "param" | "retval" | "returns" | "return" | "result" | "throw" | "throws" | "exception"
        )
    }
}

/// Whether `tag` is a command that is part of the surrounding text, rather than starting a block.
fn is_inline_command(tag: &str) -> bool {
    matches!(
        tag,
        "a" | "b" | "c" | "p" | "e" | "em" | "emoji" | "code" | "endcode"
    )
}

/// Links the references in the text of a `@see` command, like `Foo, Bar.`. The first word is
/// linked if it is an identifier, later ones only if they look like code, rather than a
/// description.
fn link_references(text: &str) -> String {
    let mut is_first = true;
    text.split_inclusive(char::is_whitespace)
        .map(|piece| {
            let word = piece.trim_end_matches(char::is_whitespace);
            if word.is_empty() {
                return piece.to_string();
            }
            let target = word.trim_end_matches(['.', ',', ';', ':']);
            let is_reference = is_identifier(target) && (is_first || looks_like_code(target));
            is_first = false;
            if is_reference {
                format!("[`{target}`]{}", &piece[target.len()..])
            } else {
                piece.to_string()
            }
        })
        .collect()
}

fn is_identifier(word: &str) -> bool {
    let word = word.strip_suffix("()").unwrap_or(word);
    let word = word.strip_prefix('#').unwrap_or(word);
    word.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | ':' | '#'))
}

/// Whether `word` has underscores, a `::`, `#` or `()`, or is camel case, unlike most words.
fn looks_like_code(word: &str) -> bool {
    let is_camel_case = word.chars().skip(1).any(|c| c.is_ascii_uppercase())
        && word.chars().any(|c| c.is_ascii_lowercase());
    word.contains(['_', ':', '#', '(']) || is_camel_case
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
            "sa" | "see" => String::from("\n**See also:** "),
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
            "@sa random_thing\n@see  OH_Foo OhBar, #OH_Baz.",
            "\n**See also:** [`random_thing`]\n\n**See also:** [`OH_Foo`] [`OhBar`], [`#OH_Baz`]."
        );
    }

    #[test]
    fn see_also_description() {
        test_rustdoc!(
            "@since 10\n@see {@link OH_Foo} Finishes the operation.",
            "\nAvailable since API-level: 10\n\n**See also:** [`OH_Foo`] Finishes the operation."
        );
        test_rustdoc!(
            "@see <a href=\"https://example.com/#Some_Page\">Some Page</a>",
            "\n**See also:** <a href=\"https://example.com/#Some_Page\">Some Page</a>"
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
        test_rustdoc!("@see {@link Foo}", "\n**See also:** [`Foo`]");
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
    fn html_list_replaces_return_item() {
        test_rustdoc!(
            "@return <ul>\n<li>{@link A} if ok.</li>\n<li>{@link B} if\nnot.</li>\n</ul>\n@since 26",
            "\n# Returns\n\n- [`A`] if ok.\n- [`B`] if not.\n\nAvailable since API-level: 26"
        );
    }

    #[test]
    fn html_list_in_param_item() {
        test_rustdoc!(
            "@param mode The mode: <ul><li>0: off</li><li>1: on</li></ul>\n@param x X",
            "# Arguments\n\n* `mode` - The mode:\n  - 0: off\n  - 1: on\n\n* `x` - X"
        );
    }

    #[test]
    fn html_list_after_return_item() {
        test_rustdoc!(
            "@return The status code.\n<ul>\n<li>A</li>\n</ul>\nMore text.",
            "\n# Returns\n\n* The status code.\n  - A\n\nMore text."
        );
    }

    #[test]
    fn html_list_in_paragraph() {
        test_rustdoc!(
            "Values:\n<ul>\n<li>a</li>\n</ul> Done.\nMore",
            "Values:\n- a\n\nDone.\nMore"
        );
    }

    #[test]
    fn unbalanced_html_list() {
        test_rustdoc!(
            "<ul><li>a</li>\n@since 10",
            "<ul><li>a</li>\n\nAvailable since API-level: 10"
        );
    }

    #[test]
    fn can_parse_example() {
        let example = include_str!("../tests/assets/example-bindgen.rs");
        println!("{}", rustdoc(example.into()).unwrap());
    }
}
