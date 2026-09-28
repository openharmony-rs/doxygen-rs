//! Conversion of HTML lists to Markdown lists.
//!
//! In Markdown, a line starting with `<li>` or `</ul>` starts an HTML block, which breaks a list
//! spread over several lines out of the surrounding paragraph or list item, and links inside HTML
//! blocks are not resolved. Markdown lists avoid both problems.

/// Converts the `<ul>` lists in `text` to Markdown lists, indented to be nested in the list item
/// that `text` starts with, if `in_list_item` is set. A list that is the only content of a list
/// item, like in `* <ul>...</ul>`, replaces the item.
///
/// `text` must have balanced list tags, see [`has_balanced_lists`].
pub(crate) fn lists_to_markdown(text: &str, in_list_item: bool) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("<ul>") {
        let line_start = rest[..start].rfind('\n').map_or(0, |i| i + 1);
        out.push_str(&rest[..line_start]);
        let line = &rest[line_start..start];
        let leading_ws = &line[..line.len() - line.trim_start().len()];
        let prefix = line.trim();
        let end = start + matching_close(&rest[start..]);
        let items = &rest[start + "<ul>".len()..end - "</ul>".len()];

        let indent = if prefix == "*" || prefix == "-" {
            leading_ws.to_string()
        } else {
            if !prefix.is_empty() {
                out.push_str(leading_ws);
                out.push_str(prefix);
                out.push('\n');
            }
            if in_list_item {
                "  ".to_string()
            } else {
                leading_ws.to_string()
            }
        };
        push_items(&mut out, items, &indent);

        rest = &rest[end..];
        let line_end = rest.find('\n').unwrap_or(rest.len());
        let suffix = rest[..line_end].trim();
        // Separate following text from the list, so it is not a lazy continuation of the last item.
        if !suffix.is_empty() {
            out.push('\n');
            out.push_str(&indent);
            out.push_str(suffix);
            rest = &rest[line_end..];
        } else {
            rest = rest.get(line_end + 1..).unwrap_or("");
            let next_line = rest.lines().next().unwrap_or("").trim_start();
            if !next_line.is_empty() && !starts_block(next_line) {
                out.push('\n');
            }
        }
    }
    out.push_str(rest);
    out
}

/// Whether every `<ul>` in `text` is closed by a `</ul>`.
pub(crate) fn has_balanced_lists(text: &str) -> bool {
    let mut depth = 0i32;
    let mut rest = text;
    while let Some(i) = rest
        .find("<ul>")
        .into_iter()
        .chain(rest.find("</ul>"))
        .min()
    {
        if rest[i..].starts_with("<ul>") {
            depth += 1;
        } else {
            depth -= 1;
            if depth < 0 {
                return false;
            }
        }
        rest = &rest[i + 1..];
    }
    depth == 0
}

/// Whether `line` starts a list item, heading or HTML list, which does not need a preceding blank
/// line.
fn starts_block(line: &str) -> bool {
    ["* ", "- ", "+ ", "#", "<ul>"]
        .iter()
        .any(|prefix| line.starts_with(prefix))
}

/// Byte offset just past the `</ul>` matching the `<ul>` at the start of `s`.
fn matching_close(s: &str) -> usize {
    let mut depth = 0;
    let mut pos = 0;
    loop {
        let next_open = s[pos..].find("<ul>").map(|i| pos + i);
        let next_close = pos + s[pos..].find("</ul>").expect("balanced");
        match next_open {
            Some(open) if open < next_close => {
                depth += 1;
                pos = open + "<ul>".len();
            }
            _ => {
                depth -= 1;
                pos = next_close + "</ul>".len();
                if depth == 0 {
                    return pos;
                }
            }
        }
    }
}

/// Emits each `<li>` in `items` as a Markdown list item, recursing into nested lists.
fn push_items(out: &mut String, items: &str, indent: &str) {
    for item in split_items(items) {
        let (text, nested) = match item.find("<ul>") {
            Some(i) => {
                let end = i + matching_close(&item[i..]);
                (
                    format!("{} {}", &item[..i], &item[end..]),
                    Some(&item[i + "<ul>".len()..end - "</ul>".len()]),
                )
            }
            None => (item.to_string(), None),
        };
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        out.push_str(indent);
        out.push_str("- ");
        out.push_str(&text);
        out.push('\n');
        if let Some(nested) = nested {
            push_items(out, nested, &format!("{indent}  "));
        }
    }
}

/// Splits the contents of a `<ul>` into the contents of its top-level `<li>` elements.
fn split_items(items: &str) -> Vec<&str> {
    let mut result = vec![];
    let mut depth = 0;
    let mut item_start = None;
    let mut pos = 0;
    while pos < items.len() {
        let rest = &items[pos..];
        if rest.starts_with("<ul>") {
            depth += 1;
            pos += "<ul>".len();
        } else if rest.starts_with("</ul>") {
            depth -= 1;
            pos += "</ul>".len();
        } else if depth == 0 && rest.starts_with("<li>") {
            if let Some(start) = item_start {
                result.push(&items[start..pos]);
            }
            pos += "<li>".len();
            item_start = Some(pos);
        } else if depth == 0 && rest.starts_with("</li>") {
            if let Some(start) = item_start.take() {
                result.push(&items[start..pos]);
            }
            pos += "</li>".len();
        } else {
            pos += rest.chars().next().map_or(1, char::len_utf8);
        }
    }
    if let Some(start) = item_start {
        result.push(&items[start..]);
    }
    result
        .into_iter()
        .filter(|item| !item.trim().is_empty())
        .collect()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn nested_lists() {
        assert_eq!(
            lists_to_markdown("<ul><li>a<ul><li>b</li></ul></li><li>c</li></ul>", false),
            "- a\n  - b\n- c\n"
        );
    }

    #[test]
    fn balanced_lists() {
        assert!(has_balanced_lists("<ul><li>a<ul><li>b</li></ul></li></ul>"));
        assert!(!has_balanced_lists("<ul><li>a</li>"));
        assert!(!has_balanced_lists("</ul><ul>"));
    }
}
