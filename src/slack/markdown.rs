use regex::Regex;

/// Convert markdown text to Slack mrkdwn format
///
/// Slack's mrkdwn format differences from standard markdown:
/// - *bold* (Slack) vs **bold** (Markdown)
/// - _italic_ (Slack) vs *italic* or _italic_ (Markdown)
/// - Headers (##) -> Bold text with spacing
/// - Lists, code blocks, and links work similarly
///
/// This function converts standard markdown to Slack-compatible format.
pub fn markdown_to_slack(text: &str) -> String {
    let mut result = text.to_string();

    // Convert headers to bold (## Header -> *Header*)
    result = convert_headers(&result);

    // Convert **bold** to *bold* (avoid code blocks)
    result = convert_bold(&result);

    // Clean up extra newlines
    result = clean_newlines(&result);

    result
}

fn convert_headers(text: &str) -> String {
    // Use regex to convert headers, preserving content
    let h1_re = Regex::new(r"(?m)^#\s+(.+)$").unwrap();
    let h2_re = Regex::new(r"(?m)^##\s+(.+)$").unwrap();
    let h3_re = Regex::new(r"(?m)^###\s+(.+)$").unwrap();

    let result = h3_re.replace_all(text, "*$1*");
    let result = h2_re.replace_all(&result, "\n*$1*");
    let result = h1_re.replace_all(&result, "\n*$1*");

    result.to_string()
}

fn convert_bold(text: &str) -> String {
    // Convert **text** to *text* but not inside code blocks
    let code_block_re = Regex::new(r"```[\s\S]*?```").unwrap();

    // Extract code blocks
    let mut code_blocks = Vec::new();
    let mut text_without_code = text.to_string();

    for cap in code_block_re.find_iter(text) {
        code_blocks.push(cap.as_str().to_string());
        text_without_code = text_without_code.replace(
            cap.as_str(),
            &format!("__CODE_BLOCK_{}__", code_blocks.len() - 1),
        );
    }

    // Convert **text** to *text* in non-code parts
    let bold_re = Regex::new(r"\*\*([^\*]+)\*\*").unwrap();
    text_without_code = bold_re.replace_all(&text_without_code, "*$1*").to_string();

    // Restore code blocks
    for (i, block) in code_blocks.iter().enumerate() {
        text_without_code = text_without_code.replace(&format!("__CODE_BLOCK_{}__", i), block);
    }

    text_without_code
}

fn clean_newlines(text: &str) -> String {
    // Remove excessive newlines (more than 2 consecutive)
    let multi_newline_re = Regex::new(r"\n{3,}").unwrap();
    multi_newline_re.replace_all(text, "\n\n").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_bold() {
        let result = markdown_to_slack("**bold**");
        assert!(result.contains("*bold*"));

        let result = markdown_to_slack("This is **bold** text");
        assert!(result.contains("*bold*"));
    }

    #[test]
    fn test_convert_headers() {
        let input = "## Header\nSome text";
        let output = markdown_to_slack(input);
        assert!(output.contains("*Header*"));
    }

    #[test]
    fn test_preserve_code() {
        let result = markdown_to_slack("`code`");
        assert!(result.contains("`code`"));

        let result = markdown_to_slack("```rust\ncode\n```");
        assert!(result.contains("```"));
        assert!(result.contains("code"));
    }

    #[test]
    fn test_example_text() {
        let input = "This repository implements **http-tunnel**\n## What it does:\nSome text";
        let output = markdown_to_slack(input);
        assert!(output.contains("*http-tunnel*"));
        assert!(output.contains("*What it does:*"));
    }
}
