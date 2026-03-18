/// Join hard-wrapped prose lines within paragraphs, preserving intentional
/// Markdown structure (fenced code blocks, lists, headers, blockquotes, etc.).
pub fn fill_lines(body: &str) -> String {
    // Collect output as a sequence of logical lines (including empty strings for
    // blank lines). Joining with '\n' at the end avoids prefix/suffix \n confusion.
    let mut out_lines: Vec<String> = Vec::new();
    let mut prose_buf: Vec<&str> = Vec::new();
    let mut in_fence = false;

    let flush_prose = |buf: &mut Vec<&str>, out: &mut Vec<String>| {
        if !buf.is_empty() {
            out.push(buf.join(" "));
            buf.clear();
        }
    };

    for line in body.lines() {
        // Track fenced code blocks (``` or ~~~)
        if is_fence(line) {
            flush_prose(&mut prose_buf, &mut out_lines);
            out_lines.push(line.to_owned());
            in_fence = !in_fence;
            continue;
        }

        if in_fence {
            out_lines.push(line.to_owned());
            continue;
        }

        if line.is_empty() {
            flush_prose(&mut prose_buf, &mut out_lines);
            out_lines.push(String::new());
            continue;
        }

        if is_structural(line) {
            flush_prose(&mut prose_buf, &mut out_lines);
            out_lines.push(line.to_owned());
        } else {
            prose_buf.push(line);
        }
    }

    flush_prose(&mut prose_buf, &mut out_lines);

    // Remove leading/trailing blank lines that may have been added
    while out_lines.first().map(|s| s.is_empty()).unwrap_or(false) {
        out_lines.remove(0);
    }
    while out_lines.last().map(|s| s.is_empty()).unwrap_or(false) {
        out_lines.pop();
    }

    out_lines.join("\n")
}

fn is_fence(line: &str) -> bool {
    line.starts_with("```") || line.starts_with("~~~")
}

fn is_structural(line: &str) -> bool {
    // ATX headers
    if line.starts_with('#') {
        return true;
    }
    // Blockquote
    if line.starts_with('>') {
        return true;
    }
    // Unordered list (-, *, +) — must be followed by a space
    if let Some(rest) = line.strip_prefix('-') {
        if rest.starts_with(' ') || rest.is_empty() {
            return true;
        }
    }
    if let Some(rest) = line.strip_prefix('*') {
        if rest.starts_with(' ') || rest.is_empty() {
            return true;
        }
    }
    if let Some(rest) = line.strip_prefix('+') {
        if rest.starts_with(' ') || rest.is_empty() {
            return true;
        }
    }
    // Ordered list: digits followed by ". "
    {
        let trimmed = line;
        let digit_end = trimmed
            .char_indices()
            .take_while(|(_, c)| c.is_ascii_digit())
            .map(|(i, c)| i + c.len_utf8())
            .last();
        if let Some(end) = digit_end {
            if end > 0 && trimmed[end..].starts_with(". ") {
                return true;
            }
        }
    }
    // Indented code block (4+ spaces or tab)
    if line.starts_with("    ") || line.starts_with('\t') {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_paragraph_reflow() {
        assert_eq!(fill_lines("foo\nbar\nbaz"), "foo bar baz");
    }

    #[test]
    fn paragraph_break_preserved() {
        assert_eq!(fill_lines("foo\nbar\n\nbaz\nquux"), "foo bar\n\nbaz quux");
    }

    #[test]
    fn unordered_list_preserved() {
        let input = "- item 1\n- item 2";
        assert_eq!(fill_lines(input), input);
    }

    #[test]
    fn ordered_list_preserved() {
        let input = "1. first\n2. second";
        assert_eq!(fill_lines(input), input);
    }

    #[test]
    fn header_preserved() {
        assert_eq!(fill_lines("# Title\nparagraph"), "# Title\nparagraph");
    }

    #[test]
    fn fenced_code_block_preserved() {
        let input = "before\n```\nfoo\n  bar\n```\nafter";
        assert_eq!(fill_lines(input), input);
    }

    #[test]
    fn tilde_fenced_code_block_preserved() {
        let input = "before\n~~~\nfoo\n  bar\n~~~\nafter";
        assert_eq!(fill_lines(input), input);
    }

    #[test]
    fn indented_code_preserved() {
        let input = "    code line\n    another";
        assert_eq!(fill_lines(input), input);
    }

    #[test]
    fn tab_indented_preserved() {
        let input = "\tcode line";
        assert_eq!(fill_lines(input), input);
    }

    #[test]
    fn blockquote_preserved() {
        let input = "> quote line";
        assert_eq!(fill_lines(input), input);
    }

    #[test]
    fn mixed_prose_and_list() {
        let input = "This is a long\nsentence here.\n\n- item 1\n- item 2";
        let expected = "This is a long sentence here.\n\n- item 1\n- item 2";
        assert_eq!(fill_lines(input), expected);
    }

    #[test]
    fn empty_input() {
        assert_eq!(fill_lines(""), "");
    }

    #[test]
    fn single_line_unchanged() {
        assert_eq!(fill_lines("just one line"), "just one line");
    }

    #[test]
    fn realistic_commit_message() {
        let input = "\
This commit adds support for the new\n\
frob configuration option, which\n\
controls the frob rate.\n\
\n\
- Updated config parser\n\
- Added validation\n\
- Tests included\n\
\n\
Fixes #42";
        let expected = "\
This commit adds support for the new frob configuration option, which controls the frob rate.\n\
\n\
- Updated config parser\n\
- Added validation\n\
- Tests included\n\
\n\
Fixes #42";
        assert_eq!(fill_lines(input), expected);
    }

    #[test]
    fn idempotent_prose() {
        let input = "foo\nbar\nbaz";
        assert_eq!(fill_lines(&fill_lines(input)), fill_lines(input));
    }

    #[test]
    fn idempotent_mixed() {
        let input = "prose line one\nprose line two\n\n- list item\n- another";
        assert_eq!(fill_lines(&fill_lines(input)), fill_lines(input));
    }

    #[test]
    fn idempotent_code_block() {
        let input = "intro\n```\ncode\n```\noutro";
        assert_eq!(fill_lines(&fill_lines(input)), fill_lines(input));
    }

    #[test]
    fn lines_with_1_to_3_leading_spaces_are_prose() {
        // Lines with 1-3 leading spaces are prose (not structural indented code),
        // so they are joined together. Leading spaces on each line are preserved in
        // the joined output.
        assert_eq!(
            fill_lines(" one space\n two spaces\n   three spaces"),
            " one space  two spaces    three spaces"
        );
    }
}
