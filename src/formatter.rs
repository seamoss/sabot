// Sabot source code formatter
// Works on raw source text to preserve comments.
// Normalizes indentation and spacing within word definitions.

pub fn format_source(source: &str) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let mut output = Vec::new();
    let mut i = 0;
    let mut prev_blank = false;

    while i < lines.len() {
        let raw = lines[i];
        let trimmed = strip_comment(raw).0.trim();
        let comment = strip_comment(raw).1;

        if trimmed.is_empty() && comment.is_none() {
            // Blank line: preserve one, collapse multiples
            if !prev_blank && !output.is_empty() {
                output.push(String::new());
            }
            prev_blank = true;
            i += 1;
            continue;
        }
        prev_blank = false;

        // Detect word definition start: `: name`
        if is_word_def_start(trimmed) {
            // Collect the entire word definition (until `;`)
            let (formatted, consumed) = format_word_def(&lines[i..]);
            for line in formatted {
                output.push(line);
            }
            i += consumed;
            continue;
        }

        // Let binding
        if trimmed.starts_with("let ") || (comment.is_none() && raw.trim().starts_with("let ")) {
            output.push(format_let_line(raw));
            i += 1;
            continue;
        }

        // Top-level comment or expression: normalize indentation and spacing
        if is_comment_only(raw) {
            output.push(raw.trim().to_string());
        } else {
            let (code, comment) = strip_comment(raw);
            let normalized = normalize_spaces(code.trim());
            match comment {
                Some(c) => output.push(format!("{} {}", normalized, c.trim())),
                None => output.push(normalized),
            }
        }
        i += 1;
    }

    // Remove trailing blank lines
    while output.last().map(|s| s.is_empty()).unwrap_or(false) {
        output.pop();
    }

    let mut result = output.join("\n");
    result.push('\n');
    result
}

/// Check if a line starts a word definition
fn is_word_def_start(trimmed: &str) -> bool {
    if !trimmed.starts_with(':') {
        return false;
    }
    let after_colon = trimmed[1..].trim_start();
    // Must be followed by an identifier
    after_colon.starts_with(|c: char| c.is_alphabetic() || c == '_')
}

/// Check if line is only a comment
fn is_comment_only(line: &str) -> bool {
    line.trim().starts_with("--")
}

/// Split a line into (code, optional comment)
fn strip_comment(line: &str) -> (&str, Option<&str>) {
    let mut in_string = false;
    let mut escape = false;
    let bytes = line.as_bytes();

    for i in 0..bytes.len() {
        if escape {
            escape = false;
            continue;
        }
        if in_string {
            if bytes[i] == b'\\' {
                escape = true;
            } else if bytes[i] == b'"' {
                in_string = false;
            }
            continue;
        }
        if bytes[i] == b'"' {
            in_string = true;
            continue;
        }
        if bytes[i] == b'-' && i + 1 < bytes.len() && bytes[i + 1] == b'-' {
            let code = &line[..i];
            let comment = &line[i..];
            return (code, Some(comment));
        }
    }
    (line, None)
}

/// Format a word definition block. Returns (formatted lines, lines consumed).
fn format_word_def(lines: &[&str]) -> (Vec<String>, usize) {
    // First, collect all lines until we find the closing `;`
    let mut raw_lines = Vec::new();
    let mut found_semi = false;
    let mut consumed = 0;

    for line in lines {
        consumed += 1;
        raw_lines.push(*line);
        let (code, _) = strip_comment(line);
        if code.trim().ends_with(';') || code.trim() == ";" {
            found_semi = true;
            break;
        }
    }

    if !found_semi {
        // Malformed — return as-is
        return (raw_lines.iter().map(|l| l.to_string()).collect(), consumed);
    }

    // Parse the word def into structured parts
    let joined = raw_lines.join("\n");
    let (header, arms, trailing_comment) = parse_word_structure(&joined);

    let mut output = Vec::new();

    if arms.len() == 1 && !arms[0].has_guard {
        // Single-arm: format as one-liner if short enough
        let arm = &arms[0];
        let one_liner = format!(": {} {} -> {} ;", header, arm.pattern, arm.body);
        let one_liner = if let Some(ref c) = trailing_comment {
            format!("{} {}", one_liner.trim_end(), c)
        } else {
            one_liner
        };
        if one_liner.len() <= 80 {
            output.push(one_liner);
        } else {
            // Too long, split across lines
            output.push(format!(": {}", header));
            output.push(format!("  {} -> {}", arm.pattern, arm.body));
            if let Some(ref c) = trailing_comment {
                output.push(format!("; {}", c));
            } else {
                output.push(";".to_string());
            }
        }
    } else {
        // Multi-arm: header on own line, each arm indented
        output.push(format!(": {}", header));
        for arm in &arms {
            if arm.has_guard {
                output.push(format!(
                    "  {} where {} -> {}",
                    arm.pattern, arm.guard_text, arm.body
                ));
            } else {
                let arm_line = format!("  {} -> {}", arm.pattern, arm.body);
                if let Some(ref c) = arm.comment {
                    output.push(format!("{} {}", arm_line.trim_end(), c));
                } else {
                    output.push(arm_line);
                }
            }
        }
        if let Some(ref c) = trailing_comment {
            output.push(format!("; {}", c));
        } else {
            output.push(";".to_string());
        }
    }

    (output, consumed)
}

struct ArmInfo {
    pattern: String,
    has_guard: bool,
    guard_text: String,
    body: String,
    comment: Option<String>,
}

/// Parse a word definition into header name and arms
fn parse_word_structure(source: &str) -> (String, Vec<ArmInfo>, Option<String>) {
    let source = source.trim();

    // Extract the colon and name
    let after_colon = source.strip_prefix(':').unwrap().trim();
    let name_end = after_colon
        .find(|c: char| c.is_whitespace() || c == '[')
        .unwrap_or(after_colon.len());
    let name = after_colon[..name_end].trim().to_string();
    let rest = after_colon[name_end..].trim();

    // Strip trailing semicolon and any comment after it
    let (body, trailing_comment) = {
        // Find the last `;` not inside a string
        let mut last_semi = None;
        let mut in_str = false;
        let mut esc = false;
        for (i, ch) in rest.char_indices() {
            if esc {
                esc = false;
                continue;
            }
            if in_str {
                if ch == '\\' {
                    esc = true;
                }
                if ch == '"' {
                    in_str = false;
                }
                continue;
            }
            if ch == '"' {
                in_str = true;
                continue;
            }
            if ch == ';' {
                last_semi = Some(i);
            }
        }
        match last_semi {
            Some(pos) => {
                let before = rest[..pos].trim();
                let after = rest[pos + 1..].trim();
                let comment = if after.starts_with("--") || !after.is_empty() {
                    if after.starts_with("--") {
                        Some(after.to_string())
                    } else {
                        None
                    }
                } else {
                    None
                };
                (before, comment)
            }
            None => (rest, None),
        }
    };

    // Split into arms by `->` (respecting brackets)
    let arms = parse_arms(body);

    (name, arms, trailing_comment)
}

/// Parse match arms from the body of a word definition
fn parse_arms(body: &str) -> Vec<ArmInfo> {
    // Strategy: find all `[pattern] (where guard)? -> body` sequences
    // An arm starts with `[` at depth 0
    let mut arms = Vec::new();
    let chars: Vec<char> = body.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Skip whitespace/newlines
        while i < chars.len() && (chars[i].is_whitespace()) {
            i += 1;
        }
        if i >= chars.len() {
            break;
        }

        // Expect a pattern starting with `[` or `{[`
        // Find the pattern (could be [stuff] or {[stuff]})
        let pattern_str;
        if chars[i] == '[' || chars[i] == '{' {
            let pat_end = find_pattern_end(&chars, i);
            pattern_str = chars[i..pat_end].iter().collect::<String>();
            i = pat_end;
        } else {
            // Not a pattern start — this shouldn't happen in well-formed code
            // Skip to next `[`
            while i < chars.len() && chars[i] != '[' && chars[i] != '{' {
                i += 1;
            }
            continue;
        }

        // Skip whitespace
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }

        // Check for `where` guard
        let mut has_guard = false;
        let mut guard_text = String::new();
        if i + 5 <= chars.len() {
            let word: String = chars[i..std::cmp::min(i + 5, chars.len())].iter().collect();
            if word == "where" {
                has_guard = true;
                i += 5;
                // Read until `->`
                let arrow_pos = find_arrow(&chars, i);
                guard_text = chars[i..arrow_pos]
                    .iter()
                    .collect::<String>()
                    .trim()
                    .to_string();
                i = arrow_pos;
            }
        }

        // Expect `->`
        if i + 1 < chars.len() && chars[i] == '-' && chars[i + 1] == '>' {
            i += 2;
        }

        // Skip whitespace
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }

        // Read body until next arm pattern `[` at depth 0, or end
        let body_start = i;
        let body_end = find_body_end(&chars, i);
        let body_str = chars[body_start..body_end].iter().collect::<String>();
        i = body_end;

        // Check for inline comment
        let (body_clean, comment) = {
            let (code, cmt) = strip_comment(&body_str);
            (normalize_spaces(code.trim()), cmt.map(|c| c.to_string()))
        };

        arms.push(ArmInfo {
            pattern: normalize_spaces(pattern_str.trim()),
            has_guard,
            guard_text,
            body: body_clean,
            comment,
        });
    }

    arms
}

/// Find the end of a pattern (matching brackets)
fn find_pattern_end(chars: &[char], start: usize) -> usize {
    let mut i = start;
    let mut depth = 0;
    loop {
        if i >= chars.len() {
            return i;
        }
        match chars[i] {
            '[' | '{' => depth += 1,
            ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
}

/// Find `->` not inside brackets
fn find_arrow(chars: &[char], start: usize) -> usize {
    let mut i = start;
    let mut depth = 0;
    while i + 1 < chars.len() {
        match chars[i] {
            '[' | '{' => depth += 1,
            ']' | '}' => depth -= 1,
            '-' if depth == 0 && chars[i + 1] == '>' => return i,
            _ => {}
        }
        i += 1;
    }
    chars.len()
}

/// Find end of arm body: next `[` or `{[` at depth 0 that starts a new pattern
fn find_body_end(chars: &[char], start: usize) -> usize {
    let mut i = start;
    let mut depth = 0;
    let mut in_str = false;
    let mut esc = false;

    while i < chars.len() {
        if esc {
            esc = false;
            i += 1;
            continue;
        }
        if in_str {
            if chars[i] == '\\' {
                esc = true;
            }
            if chars[i] == '"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        if chars[i] == '"' {
            in_str = true;
            i += 1;
            continue;
        }

        match chars[i] {
            '[' if depth == 0 => {
                // Check if this looks like a new pattern (preceded by newline or start)
                // Heuristic: if we're at depth 0 and the [ is preceded by only whitespace
                // on this line, it's a new arm
                if is_arm_start(chars, start, i) {
                    return i;
                }
                depth += 1;
            }
            '{' if depth == 0 && i + 1 < chars.len() && chars[i + 1] == '[' => {
                // List pattern start for new arm
                if is_arm_start(chars, start, i) {
                    return i;
                }
                depth += 1;
            }
            '[' | '{' => depth += 1,
            ']' | '}' => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    i
}

/// Check if position i looks like the start of a new arm pattern.
/// An arm pattern is `[...] ->` or `[...] where ... ->`.
/// A quotation like `[2 *]` is NOT followed by `->` at the same level.
fn is_arm_start(chars: &[char], body_start: usize, pos: usize) -> bool {
    if pos == body_start {
        return false;
    }

    // Must be preceded by whitespace
    if pos > 0 && !chars[pos - 1].is_whitespace() {
        return false;
    }

    // Look ahead: find matching `]`, then check if `->` or `where` follows
    let close = find_pattern_end(chars, pos);
    if close >= chars.len() {
        return false;
    }

    // Skip whitespace after closing bracket
    let mut j = close;
    while j < chars.len() && chars[j].is_whitespace() {
        j += 1;
    }

    // Check for `->` or `where`
    if j + 1 < chars.len() && chars[j] == '-' && chars[j + 1] == '>' {
        return true;
    }
    if j + 5 <= chars.len() {
        let word: String = chars[j..j + 5].iter().collect();
        if word == "where" {
            return true;
        }
    }
    false
}

/// Normalize multiple spaces to single space
fn normalize_spaces(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut prev_space = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                result.push(' ');
            }
            prev_space = true;
        } else {
            result.push(ch);
            prev_space = false;
        }
    }
    result.trim().to_string()
}

/// Format a let binding line
fn format_let_line(line: &str) -> String {
    let trimmed = line.trim();
    let (code, comment) = strip_comment(trimmed);
    let normalized = normalize_spaces(code.trim());
    match comment {
        Some(c) => format!("{} {}", normalized, c.trim()),
        None => normalized,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_word_oneliner() {
        let input = ":   double   [n]   ->   n   2   * ;";
        let output = format_source(input);
        assert_eq!(output.trim(), ": double [n] -> n 2 * ;");
    }

    #[test]
    fn test_multiarm_word() {
        let input = r#"
: factorial
  [0] -> 1
  [n] -> n n 1 - factorial *
;
"#;
        let output = format_source(input);
        assert!(output.contains(": factorial"));
        assert!(output.contains("  [0] -> 1"));
        assert!(output.contains("  [n] -> n n 1 - factorial *"));
        assert!(output.contains(";"));
    }

    #[test]
    fn test_guard_arm() {
        let input = r#"
: classify
  [n] where n > 0 -> "positive"
  [n] where n < 0 -> "negative"
  [n] -> "zero"
;
"#;
        let output = format_source(input);
        assert!(output.contains("  [n] where n > 0 -> \"positive\""));
        assert!(output.contains("  [n] where n < 0 -> \"negative\""));
        assert!(output.contains("  [n] -> \"zero\""));
    }

    #[test]
    fn test_comment_preservation() {
        let input = "-- This is a comment\n42 println\n";
        let output = format_source(input);
        assert!(output.contains("-- This is a comment"));
        assert!(output.contains("42 println"));
    }

    #[test]
    fn test_blank_line_collapse() {
        let input = "42\n\n\n\n43\n";
        let output = format_source(input);
        assert_eq!(output, "42\n\n43\n");
    }

    #[test]
    fn test_let_formatting() {
        let input = "  let   x   =   42   -- value\n";
        let output = format_source(input);
        assert_eq!(output.trim(), "let x = 42 -- value");
    }

    #[test]
    fn test_normalize_spaces() {
        assert_eq!(normalize_spaces("a   b    c"), "a b c");
        assert_eq!(normalize_spaces("  hello  world  "), "hello world");
    }
}
