use anyhow::Result;
use skim::prelude::*;
use std::borrow::Cow;
use std::fs;
use std::io::Cursor;
use std::sync::Arc;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::{LinesWithEndings, as_24_bit_terminal_escaped};

use crate::target::Target;

/// A skim item that holds a target and provides syntax-highlighted preview
struct TargetItem {
    target: Target,
    display: String,
    syntax_set: Arc<SyntaxSet>,
    theme_set: Arc<ThemeSet>,
}

impl TargetItem {
    fn new(target: Target, syntax_set: Arc<SyntaxSet>, theme_set: Arc<ThemeSet>) -> Self {
        let display = target.display_name();
        Self {
            target,
            display,
            syntax_set,
            theme_set,
        }
    }

    fn get_highlighted_preview(&self) -> String {
        let content = match fs::read_to_string(&self.target.file) {
            Ok(c) => c,
            Err(_) => return "Error reading file".to_string(),
        };

        let lines: Vec<&str> = content.lines().collect();
        let target_line = self.target.line.saturating_sub(1); // Convert to 0-indexed

        // Find the end of this target's recipe by looking for the next target or end of file
        let mut end = target_line + 1;
        while end < lines.len() {
            let line = lines[end];
            // Skip empty lines and lines that start with whitespace (recipe lines)
            if !line.is_empty() && !line.starts_with('\t') && !line.starts_with(' ') {
                // Stop at non-indented comments (these are descriptions for the next target)
                if line.trim().starts_with('#') {
                    break;
                }
                // Stop at a new target definition (line with ':')
                if line.contains(':') {
                    break;
                }
            }
            end += 1;
        }

        // Trim trailing empty lines from the recipe
        while end > target_line + 1 && lines[end - 1].trim().is_empty() {
            end -= 1;
        }

        let start = target_line;

        let snippet = lines[start..end].join("\n");

        // Use Makefile syntax highlighting
        let syntax = self
            .syntax_set
            .find_syntax_by_extension("mk")
            .or_else(|| self.syntax_set.find_syntax_by_name("Makefile"))
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let theme = &self.theme_set.themes["base16-ocean.dark"];
        let mut highlighter = HighlightLines::new(syntax, theme);

        let mut result = String::new();

        // Add description at the top if present (in cyan color)
        if let Some(ref description) = self.target.description {
            result.push_str(&format!("\x1b[36m{}\x1b[0m\n\n", description));
        }

        for (i, line) in LinesWithEndings::from(&snippet).enumerate() {
            let line_num = start + i + 1;
            let marker = if line_num == self.target.line {
                ">"
            } else {
                " "
            };

            let ranges: Vec<(Style, &str)> = highlighter
                .highlight_line(line, &self.syntax_set)
                .unwrap_or_default();
            let escaped = as_24_bit_terminal_escaped(&ranges[..], false);

            result.push_str(&format!("{} {:4} │ {}", marker, line_num, escaped));
        }
        result.push_str("\x1b[0m"); // Reset colors

        result
    }
}

impl SkimItem for TargetItem {
    fn text(&self) -> Cow<'_, str> {
        // Return plain text for matching
        Cow::Borrowed(&self.target.name)
    }

    fn display<'a>(&'a self, _context: DisplayContext<'a>) -> AnsiString<'a> {
        // Return ANSI-formatted string for display
        AnsiString::parse(&self.display)
    }

    fn preview(&self, _context: PreviewContext) -> ItemPreview {
        ItemPreview::AnsiText(self.get_highlighted_preview())
    }

    fn output(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.target.name)
    }
}

/// Run the fuzzy finder UI and return the selected target
#[allow(dead_code)]
pub fn select_target(targets: &[Target]) -> Result<Option<Target>> {
    if targets.is_empty() {
        return Ok(None);
    }

    // Build the input string for skim
    let input_str = targets
        .iter()
        .map(|t| t.display_name())
        .collect::<Vec<_>>()
        .join("\n");

    // Configure skim options
    let options = SkimOptionsBuilder::default()
        .height("50%".to_string())
        .multi(false)
        .reverse(true)
        .prompt("Select target > ".to_string())
        .header(Some("Make targets (ESC to cancel)".to_string()))
        .preview(None)
        .build()
        .unwrap();

    // Run skim
    let item_reader = SkimItemReader::default();
    let items = item_reader.of_bufread(Cursor::new(input_str));

    let selected = Skim::run_with(&options, Some(items));

    // Clear the screen after skim exits to remove the TUI
    print!("\x1B[2J\x1B[H");

    match selected {
        Some(output) => {
            if output.is_abort {
                return Ok(None);
            }

            // Get the selected item
            if let Some(item) = output.selected_items.first() {
                let selected_text = item.output().to_string();
                // Extract just the target name (first word before spaces)
                let target_name = selected_text
                    .split_whitespace()
                    .next()
                    .unwrap_or(&selected_text);

                // Find the matching target
                let target = targets.iter().find(|t| t.name == target_name).cloned();
                return Ok(target);
            }
        }
        None => return Ok(None),
    }

    Ok(None)
}

/// Run the fuzzy finder with preview showing the Makefile context
pub fn select_target_with_preview(targets: &[Target]) -> Result<Option<Target>> {
    if targets.is_empty() {
        return Ok(None);
    }

    // Create a map for quick lookup
    let target_map: std::collections::HashMap<String, &Target> =
        targets.iter().map(|t| (t.name.clone(), t)).collect();

    // Load syntax highlighting resources (shared across all items)
    let syntax_set = Arc::new(SyntaxSet::load_defaults_newlines());
    let theme_set = Arc::new(ThemeSet::load_defaults());

    // Create skim items with syntax highlighting support
    let items: Vec<Arc<dyn SkimItem>> = targets
        .iter()
        .map(|t| {
            Arc::new(TargetItem::new(
                t.clone(),
                Arc::clone(&syntax_set),
                Arc::clone(&theme_set),
            )) as Arc<dyn SkimItem>
        })
        .collect();

    // Configure skim options with preview
    let options = SkimOptionsBuilder::default()
        .height("80%".to_string())
        .multi(false)
        .reverse(true)
        .prompt("Select target > ".to_string())
        .header(Some(
            "Make targets (ESC to cancel, ↑/↓ navigate, Enter select)".to_string(),
        ))
        .preview(Some("".to_string())) // Enable preview window (content comes from SkimItem)
        .preview_window("right:70%:wrap".to_string())
        .build()
        .unwrap();

    // Run skim with our custom items
    let (tx, rx): (SkimItemSender, SkimItemReceiver) = unbounded();
    for item in items {
        let _ = tx.send(item);
    }
    drop(tx); // Close the sender

    let selected = Skim::run_with(&options, Some(rx));

    // Clear the screen after skim exits to remove the TUI
    print!("\x1B[2J\x1B[H");

    match selected {
        Some(output) => {
            if output.is_abort {
                return Ok(None);
            }

            // Get the selected item
            if let Some(item) = output.selected_items.first() {
                let selected_text = item.output().to_string();

                // Find the matching target
                let target = target_map.get(&selected_text).map(|t| (*t).clone());
                return Ok(target);
            }
        }
        None => return Ok(None),
    }

    Ok(None)
}

/// Get a snippet of the Makefile around a target for display
#[allow(dead_code)]
pub fn get_target_snippet(target: &Target, context_lines: usize) -> Result<String> {
    let content = fs::read_to_string(&target.file)?;
    let lines: Vec<&str> = content.lines().collect();

    let start = target.line.saturating_sub(context_lines + 1);
    let end = (target.line + context_lines).min(lines.len());

    let snippet: Vec<String> = lines[start..end]
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let line_num = start + i + 1;
            let marker = if line_num == target.line { ">" } else { " " };
            format!("{} {:4} | {}", marker, line_num, line)
        })
        .collect();

    Ok(snippet.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_select_target_empty() {
        // With empty targets, should return None
        let result = select_target(&[]);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_get_target_snippet() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "# Comment").unwrap();
        writeln!(file, "build:").unwrap();
        writeln!(file, "\techo building").unwrap();
        writeln!(file, "").unwrap();
        writeln!(file, "test:").unwrap();
        writeln!(file, "\techo testing").unwrap();

        let target = Target::new("build".to_string(), None, file.path().to_path_buf(), 2);

        let snippet = get_target_snippet(&target, 2).unwrap();
        assert!(snippet.contains("build:"));
        assert!(snippet.contains("echo building"));
    }

    #[test]
    fn test_display_name_formatting() {
        let target_with_desc = Target::new(
            "build".to_string(),
            Some("Build the project".to_string()),
            PathBuf::from("Makefile"),
            1,
        );

        let target_without_desc =
            Target::new("clean".to_string(), None, PathBuf::from("Makefile"), 5);

        // display_name now only shows the name, description is shown in preview pane
        assert_eq!(target_with_desc.display_name(), "build");
        assert_eq!(target_without_desc.display_name(), "clean");
    }
}
