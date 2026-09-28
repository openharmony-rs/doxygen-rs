//! Escaping of brackets that are not meant as links.
//!
//! Doxygen comments use brackets in text, like in `.value[0]`, `[in]` or `[0,1]`. rustdoc parses
//! them as links without a definition, tries to resolve them as intra-doc links and warns that they
//! don't resolve. The brackets are found with pulldown-cmark, which rustdoc parses Markdown with,
//! so brackets in code, in HTML blocks and in actual links are kept.

use pulldown_cmark::{BrokenLink, Options, Parser};

/// Escapes the brackets around the label of every link without a definition in `markdown`, like
/// `[in]`, unless the label is a single code span, like in [`Foo`], which is an intra-doc link.
pub(crate) fn escape_non_link_brackets(mut markdown: String) -> String {
    loop {
        let positions = brackets_to_escape(&markdown);
        if positions.is_empty() {
            return markdown;
        }
        for &position in positions.iter().rev() {
            markdown.insert(position, '\\');
        }
    }
}

/// The sorted positions of the brackets to escape with [`escape_non_link_brackets`]. Escaping them
/// can reveal other links, like `[text]` in `[text][label]`, so the result needs to be checked again.
fn brackets_to_escape(markdown: &str) -> Vec<usize> {
    let mut positions = vec![];
    let mut callback = |link: BrokenLink| {
        let close = link.span.end - 1;
        if is_code_span(&link.reference) || markdown.as_bytes()[close] != b']' {
            return None;
        }
        // The label is the last pair of brackets, like in `[text][label]`.
        if let Some(open) = markdown[link.span.start..close].rfind('[') {
            let open = link.span.start + open;
            if !is_escaped(markdown, open) {
                positions.push(open);
            }
        }
        positions.push(close);
        None
    };
    // The options rustdoc parses the documentation with.
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_SMART_PUNCTUATION;
    Parser::new_with_broken_link_callback(markdown, options, Some(&mut callback)).for_each(drop);
    positions.sort_unstable();
    positions
}

fn is_code_span(label: &str) -> bool {
    let label = label.trim();
    label.len() > 1
        && label.starts_with('`')
        && label.ends_with('`')
        && !label[1..label.len() - 1].contains('`')
}

fn is_escaped(text: &str, index: usize) -> bool {
    let backslashes = text[..index]
        .bytes()
        .rev()
        .take_while(|&b| b == b'\\')
        .count();
    backslashes % 2 == 1
}

#[cfg(test)]
mod test {
    use super::*;

    fn escape(markdown: &str) -> String {
        escape_non_link_brackets(markdown.into())
    }

    #[test]
    fn text() {
        assert_eq!(
            escape("Range [0,1], .value[0].i32, [in] and [a b]."),
            "Range \\[0,1\\], .value\\[0\\].i32, \\[in\\] and \\[a b\\]."
        );
    }

    #[test]
    fn links() {
        let links = "[`Foo`], [`Foo`](Bar), [docs](https://example.com), [a][b], [b].\n\n[b]: c";
        assert_eq!(escape(links), links);
        assert_eq!(escape("[a][b] and [a][]"), "\\[a\\]\\[b\\] and \\[a\\][]");
    }

    #[test]
    fn index_after_link() {
        assert_eq!(escape("[`addr`][0]"), "[`addr`]\\[0\\]");
        assert_eq!(escape("[`addr`]\\[0]"), "[`addr`]\\[0\\]");
    }

    #[test]
    fn code() {
        let code = "`a[0]`\n\n```\nint a[2];\n```";
        assert_eq!(escape(code), code);
    }

    #[test]
    fn html_block() {
        let html = "Text.\n<p>See [a].\n\n<ul>\n<li>.value[0].i32</li>\n</ul>";
        assert_eq!(escape(html), html);
    }

    #[test]
    fn escaped() {
        assert_eq!(escape("\\[a] and \\\\[b]"), "\\[a] and \\\\\\[b\\]");
    }
}
