use regex::Regex;

/// Convert markdown text to Slack mrkdwn format
///
/// Slack's mrkdwn format differences from standard markdown:
/// - *bold* (Slack) vs **bold** (Markdown)
/// - _italic_ (Slack) vs *italic* or _italic_ (Markdown)
/// - Headers (##) -> Bold text with spacing
/// - Tables -> Formatted with proper alignment
/// - URLs -> Wrapped in <URL> for auto-linking
/// - Lists, code blocks work similarly
///
/// This function converts standard markdown to Slack-compatible format.
pub fn markdown_to_slack(text: &str) -> String {
    let mut result = text.to_string();

    // Convert tables to formatted text
    result = convert_tables(&result);

    // Convert headers to bold (## Header -> *Header*)
    result = convert_headers(&result);

    // Convert **bold** to *bold* (avoid code blocks and URLs)
    result = convert_bold(&result);

    // Format URLs for Slack (must be done after bold to avoid conflicts)
    result = format_urls(&result);

    // Clean up extra newlines
    result = clean_newlines(&result);

    result
}

fn convert_tables(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut result = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        // Check if this line looks like a table header (contains |)
        if line.contains('|') && i + 1 < lines.len() {
            let next_line = lines[i + 1];

            // Check if next line is a separator (|---|---|)
            if next_line.contains('|') && next_line.contains('-') {
                // This is a table! Process it
                let mut table_lines = vec![line];
                let mut j = i + 1;

                // Collect all table rows
                while j < lines.len() && lines[j].contains('|') {
                    table_lines.push(lines[j]);
                    j += 1;
                }

                // Format the table
                result.push(format_table(&table_lines));
                i = j;
                continue;
            }
        }

        result.push(line.to_string());
        i += 1;
    }

    result.join("\n")
}

fn format_table(lines: &[&str]) -> String {
    if lines.len() < 2 {
        return lines.join("\n");
    }

    // Parse table rows
    let parse_row = |line: &str| -> Vec<String> {
        line.split('|')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    };

    let header = parse_row(lines[0]);
    let rows: Vec<Vec<String>> = lines
        .iter()
        .skip(2) // Skip header and separator
        .map(|line| parse_row(line))
        .collect();

    // Calculate column widths
    let mut widths = header.iter().map(|h| h.len()).collect::<Vec<_>>();
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(cell.len());
            }
        }
    }

    // Format header
    let mut formatted = Vec::new();
    let header_line = header
        .iter()
        .enumerate()
        .map(|(i, h)| format!("{:width$}", h, width = widths.get(i).unwrap_or(&h.len())))
        .collect::<Vec<_>>()
        .join(" │ ");
    formatted.push(format!("```\n{}", header_line));

    // Add separator
    let separator = widths
        .iter()
        .map(|w| "─".repeat(*w))
        .collect::<Vec<_>>()
        .join("─┼─");
    formatted.push(separator);

    // Format rows
    for row in rows {
        let row_line = row
            .iter()
            .enumerate()
            .map(|(i, cell)| {
                format!(
                    "{:width$}",
                    cell,
                    width = widths.get(i).unwrap_or(&cell.len())
                )
            })
            .collect::<Vec<_>>()
            .join(" │ ");
        formatted.push(row_line);
    }

    formatted.push("```".to_string());
    formatted.join("\n")
}

fn convert_headers(text: &str) -> String {
    // Use regex to convert headers, preserving content
    // Process from most specific (h6) to least specific (h1) to avoid incorrect matches
    let h6_re = Regex::new(r"(?m)^######\s+(.+)$").unwrap();
    let h5_re = Regex::new(r"(?m)^#####\s+(.+)$").unwrap();
    let h4_re = Regex::new(r"(?m)^####\s+(.+)$").unwrap();
    let h3_re = Regex::new(r"(?m)^###\s+(.+)$").unwrap();
    let h2_re = Regex::new(r"(?m)^##\s+(.+)$").unwrap();
    let h1_re = Regex::new(r"(?m)^#\s+(.+)$").unwrap();

    // Note: We use closures for replacement instead of "$1" syntax
    // because the regex crate requires it for proper capture group substitution

    // H6: Small emphasis
    let result = h6_re.replace_all(text, |caps: &regex::Captures| format!("_{}_", &caps[1]));
    // H5: Small emphasis
    let result = h5_re.replace_all(&result, |caps: &regex::Captures| format!("_{}_", &caps[1]));
    // H4: Bold
    let result = h4_re.replace_all(&result, |caps: &regex::Captures| format!("*{}*", &caps[1]));
    // H3: Bold
    let result = h3_re.replace_all(&result, |caps: &regex::Captures| format!("*{}*", &caps[1]));
    // H2: Bold with spacing
    let result = h2_re.replace_all(&result, |caps: &regex::Captures| {
        format!("\n*{}*", &caps[1])
    });
    // H1: Bold with extra spacing
    let result = h1_re.replace_all(&result, |caps: &regex::Captures| {
        format!("\n\n*{}*", &caps[1])
    });

    result.to_string()
}

fn convert_bold(text: &str) -> String {
    // Convert **text** to *text* but not inside code blocks
    // Also handle URLs specially to avoid breaking them
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

    // Convert **text** to *text* but handle URLs specially
    // First, handle **URL** pattern - just remove the ** without adding *
    let bold_url_re = Regex::new(r"\*\*(https?://[^\s\*]+)\*\*").unwrap();
    text_without_code = bold_url_re
        .replace_all(&text_without_code, |caps: &regex::Captures| {
            // Just return the URL without any markdown
            caps[1].to_string()
        })
        .to_string();

    // Then convert remaining **text** to *text*
    let bold_re = Regex::new(r"\*\*([^\*]+)\*\*").unwrap();
    text_without_code = bold_re
        .replace_all(&text_without_code, |caps: &regex::Captures| {
            let content = &caps[1];
            // Check if content contains URL - if so, don't bold it
            if content.contains("http://") || content.contains("https://") {
                content.to_string()
            } else {
                format!("*{}*", content)
            }
        })
        .to_string();

    // Restore code blocks
    for (i, block) in code_blocks.iter().enumerate() {
        text_without_code = text_without_code.replace(&format!("__CODE_BLOCK_{}__", i), block);
    }

    text_without_code
}

fn format_urls(text: &str) -> String {
    // Format URLs for Slack
    // 1. Convert markdown links [text](url) to Slack format <url|text>
    // 2. Wrap standalone URLs in <URL> for auto-linking
    // 3. Don't wrap URLs already in angle brackets or code blocks

    let code_block_re = Regex::new(r"```[\s\S]*?```").unwrap();
    let inline_code_re = Regex::new(r"`[^`]+`").unwrap();

    // Extract code blocks
    let mut code_blocks = Vec::new();
    let mut result = text.to_string();

    for cap in code_block_re.find_iter(text) {
        code_blocks.push(cap.as_str().to_string());
        result = result.replace(
            cap.as_str(),
            &format!("__CODE_BLOCK_{}__", code_blocks.len() - 1),
        );
    }

    // Extract inline code
    let mut inline_codes = Vec::new();
    let inline_code_matches: Vec<String> = inline_code_re
        .find_iter(&result)
        .map(|cap| cap.as_str().to_string())
        .collect();

    for code in inline_code_matches {
        inline_codes.push(code.clone());
        result = result.replace(
            &code,
            &format!("__INLINE_CODE_{}__", inline_codes.len() - 1),
        );
    }

    // Convert markdown links [text](url) to Slack format <url|text>
    // Must be done BEFORE wrapping standalone URLs
    let markdown_link_re = Regex::new(r"\[([^\]]+)\]\((https?://[^\)]+)\)").unwrap();
    let markdown_links: Vec<(String, String)> = markdown_link_re
        .captures_iter(&result)
        .map(|caps| (caps[0].to_string(), format!("<{}|{}>", &caps[2], &caps[1])))
        .collect();

    // Replace markdown links with Slack format
    for (original, replacement) in markdown_links {
        result = result.replace(&original, &replacement);
    }

    // Wrap standalone URLs in <URL> (skip URLs already in Slack link format)
    // We need to avoid wrapping URLs that are already inside < >
    // Use a placeholder approach
    let slack_link_re = Regex::new(r"<https?://[^>]+>").unwrap();
    let mut slack_links = Vec::new();

    // Extract existing Slack links (from markdown conversion)
    let slack_link_matches: Vec<String> = slack_link_re
        .find_iter(&result)
        .map(|cap| cap.as_str().to_string())
        .collect();

    for link in slack_link_matches {
        slack_links.push(link.clone());
        result = result.replace(&link, &format!("__SLACK_LINK_{}__", slack_links.len() - 1));
    }

    // Now wrap remaining standalone URLs
    let standalone_url_re = Regex::new(r"(https?://[^\s<>]+)").unwrap();
    result = standalone_url_re.replace_all(&result, "<$1>").to_string();

    // Restore Slack links
    for (i, link) in slack_links.iter().enumerate() {
        result = result.replace(&format!("__SLACK_LINK_{}__", i), link);
    }

    // Restore inline code
    for (i, code) in inline_codes.iter().enumerate() {
        result = result.replace(&format!("__INLINE_CODE_{}__", i), code);
    }

    // Restore code blocks
    for (i, block) in code_blocks.iter().enumerate() {
        result = result.replace(&format!("__CODE_BLOCK_{}__", i), block);
    }

    result
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

    #[test]
    fn test_convert_h4_headers() {
        let input = "#### Section Title\nContent here";
        let output = markdown_to_slack(input);
        assert!(output.contains("*Section Title*"));
    }

    #[test]
    fn test_convert_h5_h6_headers() {
        // Test each header level separately since multiline processing is complex
        let h5_input = "##### Subsection";
        let h5_output = markdown_to_slack(h5_input);
        assert!(h5_output.contains("_Subsection_"));

        let h6_input = "###### Minor heading";
        let h6_output = markdown_to_slack(h6_input);
        assert!(h6_output.contains("_Minor heading_"));
    }

    #[test]
    fn test_table_formatting() {
        let input = "| Header 1 | Header 2 |\n|----------|----------|\n| Cell 1   | Cell 2   |\n| Cell 3   | Cell 4   |";
        let output = markdown_to_slack(input);
        // Should contain code block formatting
        assert!(output.contains("```"));
        // Should contain headers
        assert!(output.contains("Header 1"));
        assert!(output.contains("Header 2"));
        // Should contain cells
        assert!(output.contains("Cell 1"));
        assert!(output.contains("Cell 4"));
    }

    #[test]
    fn test_table_with_emojis() {
        let input = "| Feature | Status |\n|---------|--------|\n| Auth | :white_check_mark: |\n| Cache | :x: |";
        let output = markdown_to_slack(input);
        assert!(output.contains("Feature"));
        assert!(output.contains("Status"));
        assert!(output.contains(":white_check_mark:"));
        assert!(output.contains(":x:"));
    }

    #[test]
    fn test_url_wrapped_in_angles() {
        let input = "https://github.com/user/repo/pull/1";
        let output = markdown_to_slack(input);
        // URL should be wrapped in angle brackets for Slack
        assert!(output.contains("<https://github.com/user/repo/pull/1>"));
    }

    #[test]
    fn test_bold_url_not_broken() {
        let input = "**https://github.com/user/repo/pull/1**";
        let output = markdown_to_slack(input);
        // URL should be wrapped but not have * inside the angle brackets
        assert!(output.contains("<https://github.com/user/repo/pull/1>"));
        assert!(!output.contains("*https://"));
    }

    #[test]
    fn test_markdown_link_conversion() {
        let input = "[Pull Request](https://github.com/user/repo/pull/1)";
        let output = markdown_to_slack(input);
        eprintln!("Input: {}", input);
        eprintln!("Output: {}", output);
        // Should convert to Slack link format <url|text>
        assert!(output.contains("<https://github.com/user/repo/pull/1|Pull Request>"));
    }

    #[test]
    fn test_bold_text_with_url() {
        let input = "Check **this link** at https://example.com for details";
        let output = markdown_to_slack(input);
        // Bold should be converted
        assert!(output.contains("*this link*"));
        // URL should be wrapped
        assert!(output.contains("<https://example.com>"));
    }

    #[test]
    fn test_url_in_code_not_wrapped() {
        let input = "Use `https://example.com` in your code";
        let output = markdown_to_slack(input);
        // URL in backticks should NOT be wrapped
        assert!(output.contains("`https://example.com`"));
        assert!(!output.contains("<https://example.com>"));
    }
}
