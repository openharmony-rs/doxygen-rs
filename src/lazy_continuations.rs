//! Indentation of the lines that continue a list item.
//!
//! In Markdown, a line that continues the paragraph of a list item does not need to be indented,
//! like the second line of `* The value,\nor zero.`. Rustdoc renders such a lazy continuation as
//! part of the item, but Clippy's `doc_lazy_continuation` lint warns about it.

/// Indents the lazy continuation lines of the list items in `text` to the text of their item,
/// which keeps the rendered Markdown the same.
///
/// The lines get one space more than Markdown needs, for bindgen output that rustfmt turns into
/// `///` comments: rustfmt puts a space in front of the lines that start without one, like
/// `* item`, and rustdoc removes the space that all lines then share.
pub(crate) fn indent_lazy_continuations(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    // The indentation of the text of the list item whose paragraph continues on the next line.
    let mut item_indent = None;
    let mut fence = None;
    let mut in_html_block = false;
    let mut lines = text.split_inclusive('\n').peekable();
    while let Some(line) = lines.next() {
        let next_line = lines.peek().map_or("", |next| next.trim());
        let content = line.trim_start_matches(' ');
        let block = content.trim_end();
        // The indentation that rustdoc sees.
        let indent = (line.len() - content.len()).saturating_sub(1);
        if let Some(marker) = fence {
            if closes_fence(block, marker) {
                fence = None;
            }
        } else if block.is_empty() {
            item_indent = None;
            in_html_block = false;
        } else if in_html_block {
            // The HTML block ends at a blank line.
        } else if let Some(width) = list_marker_width(block) {
            let item_text = &block[width..];
            item_indent = None;
            // Any HTML, even a `<br>` on its own, can start an HTML block in an item, which the
            // next lines don't continue lazily.
            let starts_paragraph = !item_text.is_empty()
                && !item_text.starts_with('<')
                && !interrupts_paragraph(item_text, next_line);
            if starts_paragraph {
                item_indent = Some(indent + width);
            } else {
                fence = opens_fence(item_text);
                in_html_block = starts_html_block(item_text);
            }
        } else if interrupts_paragraph(block, next_line) {
            item_indent = None;
            fence = opens_fence(block);
            in_html_block = starts_html_block(block);
        } else if let Some(item_indent) = item_indent.filter(|&item_indent| indent < item_indent) {
            result.extend(std::iter::repeat_n(' ', item_indent + 1));
            result.push_str(content);
            continue;
        }
        result.push_str(line);
    }
    result
}

/// The width of the list marker that `line` starts with, like `* ` or `10. `, including the spaces
/// after it. A marker followed by more than four spaces is followed by indented code, so only its
/// first space counts.
fn list_marker_width(line: &str) -> Option<usize> {
    let digits = line.len() - line.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let marker = match digits {
        0 if line.starts_with(['*', '-', '+']) => 1,
        1..=9 if line[digits..].starts_with(['.', ')']) => digits + 1,
        _ => return None,
    };
    let rest = &line[marker..];
    let spaces = rest.len() - rest.trim_start_matches(' ').len();
    match spaces {
        0 => rest.is_empty().then_some(marker),
        1..=4 => Some(marker + spaces),
        _ => Some(marker + 1),
    }
}

/// Whether `line`, followed by `next_line`, starts a Markdown block that ends a paragraph, or that a
/// paragraph turns into a heading or table when the line is indented into it.
fn interrupts_paragraph(line: &str, next_line: &str) -> bool {
    let hashes = line.len() - line.trim_start_matches('#').len();
    let is_heading =
        (1..=6).contains(&hashes) && matches!(line[hashes..].chars().next(), None | Some(' '));
    let is_rule = line
        .chars()
        .all(|c| matches!(c, '-' | '=' | '*' | '_' | ' '));
    is_heading
        || is_rule
        || is_table_delimiter(line)
        || is_table_delimiter(next_line)
        || line.starts_with('>')
        || list_marker_width(line).is_some()
        || opens_fence(line).is_some()
        || starts_html_block(line)
}

/// Whether `line` is the row below the header of a table, like `|---|:-:|`.
fn is_table_delimiter(line: &str) -> bool {
    line.contains('|')
        && line.contains('-')
        && line.chars().all(|c| matches!(c, '-' | ':' | '|' | ' '))
}

/// The marker of the code block that `line` opens, like `` ``` `` or `~~~`.
fn opens_fence(line: &str) -> Option<&'static str> {
    if line.starts_with("```") && !line.trim_start_matches('`').contains('`') {
        Some("```")
    } else if line.starts_with("~~~") {
        Some("~~~")
    } else {
        None
    }
}

fn closes_fence(line: &str, marker: &str) -> bool {
    let fence_char = marker.chars().next().unwrap_or('`');
    line.starts_with(marker) && line.trim_start_matches(fence_char).trim().is_empty()
}

/// Whether `line` starts an HTML block that can end a paragraph, like `<p>` or `<ul>`, as opposed
/// to inline HTML like `<br>`.
fn starts_html_block(line: &str) -> bool {
    // The tags of the HTML blocks that can interrupt a paragraph in CommonMark.
    const BLOCK_TAGS: &str = "address article aside base basefont blockquote body caption center \
        col colgroup dd details dialog dir div dl dt fieldset figcaption figure footer form frame \
        frameset h1 h2 h3 h4 h5 h6 head header hr html iframe legend li link main menu menuitem \
        nav noframes ol optgroup option p param pre script search section style summary table \
        tbody td textarea tfoot th thead title tr track ul";
    let Some(tag) = line.strip_prefix('<') else {
        return false;
    };
    if tag.starts_with(['!', '?']) {
        return true;
    }
    let tag = tag.strip_prefix('/').unwrap_or(tag);
    let name_end = tag
        .find(|c: char| !c.is_ascii_alphanumeric())
        .unwrap_or(tag.len());
    let (name, rest) = tag.split_at(name_end);
    BLOCK_TAGS
        .split_whitespace()
        .any(|block_tag| block_tag.eq_ignore_ascii_case(name))
        && (rest.is_empty() || rest.starts_with([' ', '>']) || rest.starts_with("/>"))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn continuation_lines() {
        assert_eq!(
            indent_lazy_continuations(
                "* `a` - A,\n B\n <br>C.\n\nD\n*  E\nF\n 2. G\nH\n* I:\n| 0 1 |"
            ),
            "* `a` - A,\n   B\n   <br>C.\n\nD\n*  E\n    F\n 2. G\n    H\n* I:\n   | 0 1 |"
        );
    }

    #[test]
    fn blocks_end_the_item() {
        let text =
            "* A\n- B\n* C\n```\nD\n```\n* E\n<p>F\nG\n\n* H\n===\n* I\n1. J\n* K\n| L |\n|---|";
        assert_eq!(indent_lazy_continuations(text), text);
    }

    #[test]
    fn items_that_do_not_start_a_paragraph() {
        let text = "* ```\n- a\nb\n```\n* <br>\nc\n* # D\ne\n*\nf";
        assert_eq!(indent_lazy_continuations(text), text);
    }
}
