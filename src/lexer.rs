#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum LexItem {
    At(String),
    Paren(char),
    Word(String),
    Whitespace(char),
    NewLine,
}

impl LexItem {
    pub(crate) fn push_to(&self, acc: &mut String) {
        match self {
            LexItem::At(s) => acc.push_str(s),
            LexItem::Paren(c) => acc.push(*c),
            LexItem::Word(s) => acc.push_str(s),
            LexItem::Whitespace(c) => acc.push(*c),
            LexItem::NewLine => acc.push('\n'),
        }
    }
}

pub(crate) fn lex(input: String) -> Vec<LexItem> {
    let mut result = vec![];
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        match c {
            '@' | '\\' if starts_command(&chars, i) => {
                result.push(LexItem::At(c.into()));
            }
            // Escapes such as `\\`, `\@` or `\*` stay in the text for Markdown to handle.
            '\\' if chars
                .get(i + 1)
                .is_some_and(|&next| next.is_ascii_punctuation()) =>
            {
                push_char(&mut result, c);
                push_char(&mut result, chars[i + 1]);
                i += 1;
            }
            '{' | '}' => {
                result.push(LexItem::Paren(c));
            }
            ' ' | '\t' => {
                result.push(LexItem::Whitespace(c));
            }
            '\n' => {
                result.push(LexItem::NewLine);
            }
            _ => push_char(&mut result, c),
        }
        i += 1;
    }

    result
}

/// Whether the `@` or `\` at `chars[index]` starts a command, like `@param` or `@{`.
fn starts_command(chars: &[char], index: usize) -> bool {
    let prev = index.checked_sub(1).map(|prev| chars[prev]);
    // `{@` starts an inline command, even with a typo like `{@ link Foo}`.
    if chars[index] == '@' && prev == Some('{') {
        return true;
    }
    let starts_name = chars
        .get(index + 1)
        .is_some_and(|&next| next.is_ascii_alphabetic() || next == '{' || next == '}');
    // An `@` inside a word, like in an e-mail address, is plain text.
    let inside_word = chars[index] == '@' && prev.is_some_and(char::is_alphanumeric);
    starts_name && !inside_word
}

fn push_char(result: &mut Vec<LexItem>, c: char) {
    match result.last_mut() {
        Some(LexItem::Word(word)) => word.push(c),
        _ => result.push(LexItem::Word(String::from(c))),
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn basic_notation() {
        let result = lex("@name Memory Management".into());
        assert_eq!(
            result,
            vec![
                LexItem::At("@".into()),
                LexItem::Word("name".into()),
                LexItem::Whitespace(' '),
                LexItem::Word("Memory".into()),
                LexItem::Whitespace(' '),
                LexItem::Word("Management".into())
            ]
        );

        let result = lex("\\name Memory Management".into());
        assert_eq!(
            result,
            vec![
                LexItem::At("\\".into()),
                LexItem::Word("name".into()),
                LexItem::Whitespace(' '),
                LexItem::Word("Memory".into()),
                LexItem::Whitespace(' '),
                LexItem::Word("Management".into())
            ]
        );

        let result = lex("\\\\name Memory Management".into());
        assert_eq!(
            result,
            vec![
                LexItem::Word("\\\\name".into()),
                LexItem::Whitespace(' '),
                LexItem::Word("Memory".into()),
                LexItem::Whitespace(' '),
                LexItem::Word("Management".into())
            ]
        );
    }

    #[test]
    fn non_commands() {
        let result = lex("('\\0') a@b.c @ 1 \\@x".into());
        assert_eq!(
            result,
            vec![
                LexItem::Word("('\\0')".into()),
                LexItem::Whitespace(' '),
                LexItem::Word("a@b.c".into()),
                LexItem::Whitespace(' '),
                LexItem::Word("@".into()),
                LexItem::Whitespace(' '),
                LexItem::Word("1".into()),
                LexItem::Whitespace(' '),
                LexItem::Word("\\@x".into()),
            ]
        );
    }

    #[test]
    fn basic_groups() {
        let result = lex("@{\n* @name Memory Management\n@}".into());
        assert_eq!(
            result,
            vec![
                LexItem::At("@".into()),
                LexItem::Paren('{'),
                LexItem::NewLine,
                LexItem::Word("*".into()),
                LexItem::Whitespace(' '),
                LexItem::At("@".into()),
                LexItem::Word("name".into()),
                LexItem::Whitespace(' '),
                LexItem::Word("Memory".into()),
                LexItem::Whitespace(' '),
                LexItem::Word("Management".into()),
                LexItem::NewLine,
                LexItem::At("@".into()),
                LexItem::Paren('}')
            ]
        );
    }
}
