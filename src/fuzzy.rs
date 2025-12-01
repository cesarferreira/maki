use anyhow::Result;
use skim::prelude::*;
use std::fs;
use std::io::Cursor;

use crate::target::Target;

/// Run the fuzzy finder UI and return the selected target
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
        .height(Some("50%"))
        .multi(false)
        .reverse(true)
        .prompt(Some("Select target > "))
        .header(Some("Make targets (ESC to cancel)"))
        .preview(None) // We'll add preview separately if needed
        .build()
        .unwrap();

    // Run skim
    let item_reader = SkimItemReader::default();
    let items = item_reader.of_bufread(Cursor::new(input_str));

    let selected = Skim::run_with(&options, Some(items));

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

    // Build the input string for skim
    let input_str = targets
        .iter()
        .map(|t| t.display_name())
        .collect::<Vec<_>>()
        .join("\n");

    // Create a temporary file list for preview
    let preview_cmd = create_preview_command(targets);

    // Configure skim options with preview
    let options = SkimOptionsBuilder::default()
        .height(Some("80%"))
        .multi(false)
        .reverse(true)
        .prompt(Some("Select target > "))
        .header(Some("Make targets (ESC to cancel, ↑/↓ navigate, Enter select)"))
        .preview(preview_cmd.as_deref())
        .preview_window(Some("right:50%:wrap"))
        .build()
        .unwrap();

    // Run skim
    let item_reader = SkimItemReader::default();
    let items = item_reader.of_bufread(Cursor::new(input_str));

    let selected = Skim::run_with(&options, Some(items));

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
                let target = target_map.get(target_name).map(|t| (*t).clone());
                return Ok(target);
            }
        }
        None => return Ok(None),
    }

    Ok(None)
}

/// Create a preview command that shows the Makefile context around the target
fn create_preview_command(targets: &[Target]) -> Option<String> {
    // For simplicity, if all targets are from the same file, use that file for preview
    // Otherwise, we need a more complex solution
    if let Some(first_target) = targets.first() {
        let file_path = first_target.file.display();
        // Use sed/head/tail to show context around the target line
        // This is a simple preview that shows lines around the target
        Some(format!(
            "grep -n '{{}}:' {} | head -1 | cut -d: -f1 | xargs -I{{}} sh -c 'sed -n \"$(({{}} - 3 < 1 ? 1 : {{}} - 3)),$(({{}} + 10))p\" {}'",
            file_path, file_path
        ))
    } else {
        None
    }
}

/// Get a snippet of the Makefile around a target for display
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

        let target = Target::new(
            "build".to_string(),
            None,
            file.path().to_path_buf(),
            2,
        );

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

        let target_without_desc = Target::new(
            "clean".to_string(),
            None,
            PathBuf::from("Makefile"),
            5,
        );

        let display1 = target_with_desc.display_name();
        let display2 = target_without_desc.display_name();

        assert!(display1.contains("build"));
        assert!(display1.contains("Build the project"));
        assert_eq!(display2, "clean");
    }
}
