use anyhow::{Context, Result};
use regex::Regex;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::target::Target;

/// Options for parsing Makefiles
#[derive(Debug, Clone, Default)]
pub struct ParseOptions {
    /// Include private targets (those starting with _)
    pub include_private: bool,
    /// Include pattern rules (e.g., %.o: %.c)
    pub include_patterns: bool,
}

/// Find Makefiles in the given directory
pub fn find_makefiles(dir: &Path, recursive: bool) -> Vec<PathBuf> {
    let makefile_names = ["Makefile", "makefile", "GNUmakefile"];

    if recursive {
        WalkDir::new(dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map(|name| makefile_names.contains(&name))
                    .unwrap_or(false)
            })
            .map(|e| e.path().to_path_buf())
            .collect()
    } else {
        makefile_names
            .iter()
            .map(|name| dir.join(name))
            .filter(|p| p.exists())
            .collect()
    }
}

/// Parse a single Makefile and extract all targets
pub fn parse_makefile(path: &Path, options: &ParseOptions) -> Result<Vec<Target>> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read Makefile: {}", path.display()))?;

    parse_makefile_content(&content, path, options)
}

/// Check if a line is a variable assignment (not a target)
fn is_variable_assignment(line: &str) -> bool {
    // Simple variable assignments: VAR := value, VAR ?= value, VAR += value, VAR = value
    // These have the form: IDENTIFIER op value (where op is :=, ?=, +=, or = without :)

    // Check for simple assignment operators at the start
    if let Some(pos) = line.find(":=") {
        // Check if there's no ':' before ':=' (which would indicate a target)
        let before = &line[..pos];
        if !before.contains(':') {
            return true;
        }
    }

    if let Some(pos) = line.find("?=") {
        let before = &line[..pos];
        if !before.contains(':') {
            return true;
        }
    }

    if let Some(pos) = line.find("+=") {
        let before = &line[..pos];
        if !before.contains(':') {
            return true;
        }
    }

    // Check for simple = assignment (VAR = value), but not := or ==
    if let Some(pos) = line.find('=') {
        if pos > 0 {
            let before_char = line.chars().nth(pos - 1);
            let after_char = line.chars().nth(pos + 1);
            // Not :=, +=, ?=, or ==
            if before_char != Some(':') && before_char != Some('+') &&
               before_char != Some('?') && after_char != Some('=') {
                let before = &line[..pos];
                // Simple assignment if no colon before the =
                if !before.contains(':') {
                    return true;
                }
            }
        }
    }

    false
}

/// Check if a line is a target-specific variable (target: VAR := value)
fn is_target_specific_variable(line: &str) -> bool {
    // Target-specific variables have the form: target: VAR := value
    // or target: VAR = value
    // The key is that after the first colon and space, there's a variable assignment

    if let Some(first_colon) = line.find(':') {
        let after_first_colon = &line[first_colon + 1..];
        let after_trimmed = after_first_colon.trim_start();

        // Check if what follows looks like a variable assignment
        // It should be: IDENTIFIER followed by :=, ?=, +=, or = (with space before it)
        // Find the first space or assignment operator
        if let Some(space_pos) = after_trimmed.find(|c: char| c.is_whitespace() || c == ':' || c == '?' || c == '+' || c == '=') {
            let potential_var = &after_trimmed[..space_pos];
            // Variable names are typically uppercase letters, numbers, underscores
            if !potential_var.is_empty() &&
               potential_var.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                // Check what operator follows (may have space before it)
                let rest = after_trimmed[space_pos..].trim_start();
                if rest.starts_with(":=") || rest.starts_with("?=") ||
                   rest.starts_with("+=") || rest.starts_with('=') {
                    return true;
                }
            }
        }
    }

    false
}

/// Parse Makefile content and extract targets
pub fn parse_makefile_content(content: &str, file: &Path, options: &ParseOptions) -> Result<Vec<Target>> {
    // Regex to match target definitions
    // Matches: target_name: [dependencies]
    // Includes % for pattern rules like %.o: %.c
    let target_regex = Regex::new(r"^([A-Za-z0-9._/\-%]+)\s*:")?;

    // Regex for pattern rules (e.g., %.o: %.c)
    let pattern_rule_regex = Regex::new(r"%")?;

    let lines: Vec<&str> = content.lines().collect();
    let mut targets = Vec::new();
    let mut seen_names: HashSet<String> = HashSet::new();

    for (line_num, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Skip variable assignments (both simple and target-specific)
        if is_variable_assignment(trimmed) || is_target_specific_variable(trimmed) {
            continue;
        }

        // Try to match a target
        if let Some(caps) = target_regex.captures(trimmed) {
            let target_name = caps.get(1).unwrap().as_str().to_string();

            // Skip pattern rules unless enabled
            if pattern_rule_regex.is_match(&target_name) && !options.include_patterns {
                continue;
            }

            // Skip private targets unless enabled
            if target_name.starts_with('_') && !options.include_private {
                continue;
            }

            // Skip duplicates
            if seen_names.contains(&target_name) {
                continue;
            }

            // Extract description from comments
            let description = extract_description(&lines, line_num);

            seen_names.insert(target_name.clone());
            targets.push(Target::new(
                target_name,
                description,
                file.to_path_buf(),
                line_num + 1, // 1-indexed line numbers
            ));
        }
    }

    Ok(targets)
}

/// Extract description from preceding comments or inline comments
fn extract_description(lines: &[&str], target_line: usize) -> Option<String> {
    let target = lines[target_line];

    // First check for inline comment after ## (common convention)
    if let Some(pos) = target.find("##") {
        let desc = target[pos + 2..].trim();
        if !desc.is_empty() {
            return Some(desc.to_string());
        }
    }

    // Check for preceding comment lines
    let mut comments = Vec::new();
    let mut i = target_line;

    while i > 0 {
        i -= 1;
        let prev_line = lines[i].trim();

        if prev_line.starts_with('#') {
            // Remove the # and any leading whitespace
            let comment = prev_line.trim_start_matches('#').trim();
            if !comment.is_empty() {
                comments.push(comment.to_string());
            }
        } else if prev_line.is_empty() {
            // Allow one blank line between comment and target
            if i > 0 {
                let before_blank = lines[i - 1].trim();
                if before_blank.starts_with('#') {
                    continue;
                }
            }
            break;
        } else {
            break;
        }
    }

    if comments.is_empty() {
        None
    } else {
        comments.reverse();
        Some(comments.join(" "))
    }
}

/// Parse all Makefiles in a directory
pub fn parse_all_makefiles(
    dir: &Path,
    recursive: bool,
    options: &ParseOptions,
) -> Result<Vec<Target>> {
    let makefiles = find_makefiles(dir, recursive);

    if makefiles.is_empty() {
        anyhow::bail!("No Makefile found in {}", dir.display());
    }

    let mut all_targets = Vec::new();
    let mut seen_names: HashSet<String> = HashSet::new();

    for makefile in makefiles {
        let targets = parse_makefile(&makefile, options)?;
        for target in targets {
            if !seen_names.contains(&target.name) {
                seen_names.insert(target.name.clone());
                all_targets.push(target);
            }
        }
    }

    // Sort targets alphabetically
    all_targets.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(all_targets)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_makefile() {
        let content = r#"
build:
	echo "Building..."

test:
	echo "Testing..."

clean:
	rm -rf build/
"#;

        let options = ParseOptions::default();
        let targets = parse_makefile_content(content, Path::new("Makefile"), &options).unwrap();

        assert_eq!(targets.len(), 3);
        assert!(targets.iter().any(|t| t.name == "build"));
        assert!(targets.iter().any(|t| t.name == "test"));
        assert!(targets.iter().any(|t| t.name == "clean"));
    }

    #[test]
    fn test_parse_with_comments() {
        let content = r#"
# Build the project
build:
	echo "Building..."

test: ## Run all tests
	echo "Testing..."
"#;

        let options = ParseOptions::default();
        let targets = parse_makefile_content(content, Path::new("Makefile"), &options).unwrap();

        assert_eq!(targets.len(), 2);

        let build = targets.iter().find(|t| t.name == "build").unwrap();
        assert_eq!(build.description, Some("Build the project".to_string()));

        let test = targets.iter().find(|t| t.name == "test").unwrap();
        assert_eq!(test.description, Some("Run all tests".to_string()));
    }

    #[test]
    fn test_skip_private_targets() {
        let content = r#"
build:
	echo "Building..."

_internal:
	echo "Internal..."
"#;

        let options = ParseOptions::default();
        let targets = parse_makefile_content(content, Path::new("Makefile"), &options).unwrap();

        assert_eq!(targets.len(), 1);
        assert!(targets.iter().any(|t| t.name == "build"));
        assert!(!targets.iter().any(|t| t.name == "_internal"));
    }

    #[test]
    fn test_include_private_targets() {
        let content = r#"
build:
	echo "Building..."

_internal:
	echo "Internal..."
"#;

        let options = ParseOptions {
            include_private: true,
            ..Default::default()
        };
        let targets = parse_makefile_content(content, Path::new("Makefile"), &options).unwrap();

        assert_eq!(targets.len(), 2);
        assert!(targets.iter().any(|t| t.name == "build"));
        assert!(targets.iter().any(|t| t.name == "_internal"));
    }

    #[test]
    fn test_skip_pattern_rules() {
        let content = r#"
build:
	echo "Building..."

%.o: %.c
	$(CC) -c $<
"#;

        let options = ParseOptions::default();
        let targets = parse_makefile_content(content, Path::new("Makefile"), &options).unwrap();

        assert_eq!(targets.len(), 1);
        assert!(targets.iter().any(|t| t.name == "build"));
    }

    #[test]
    fn test_include_pattern_rules() {
        let content = r#"
build:
	echo "Building..."

%.o: %.c
	$(CC) -c $<
"#;

        let options = ParseOptions {
            include_patterns: true,
            ..Default::default()
        };
        let targets = parse_makefile_content(content, Path::new("Makefile"), &options).unwrap();

        assert_eq!(targets.len(), 2);
    }

    #[test]
    fn test_skip_duplicates() {
        let content = r#"
build:
	echo "Building..."

build:
	echo "Building again..."
"#;

        let options = ParseOptions::default();
        let targets = parse_makefile_content(content, Path::new("Makefile"), &options).unwrap();

        assert_eq!(targets.len(), 1);
    }

    #[test]
    fn test_skip_variable_assignments() {
        let content = r#"
CC := gcc
CFLAGS ?= -Wall
LDFLAGS += -lm

build:
	echo "Building..."
"#;

        let options = ParseOptions::default();
        let targets = parse_makefile_content(content, Path::new("Makefile"), &options).unwrap();

        assert_eq!(targets.len(), 1);
        assert!(targets.iter().any(|t| t.name == "build"));
    }

    #[test]
    fn test_line_numbers() {
        let content = r#"
# Line 1 is blank
build:
	echo "Building..."

test:
	echo "Testing..."
"#;

        let options = ParseOptions::default();
        let targets = parse_makefile_content(content, Path::new("Makefile"), &options).unwrap();

        let build = targets.iter().find(|t| t.name == "build").unwrap();
        assert_eq!(build.line, 3);

        let test = targets.iter().find(|t| t.name == "test").unwrap();
        assert_eq!(test.line, 6);
    }

    #[test]
    fn test_complex_target_names() {
        let content = r#"
docker/build:
	docker build .

test.unit:
	cargo test

build-all:
	make build
"#;

        let options = ParseOptions::default();
        let targets = parse_makefile_content(content, Path::new("Makefile"), &options).unwrap();

        assert_eq!(targets.len(), 3);
        assert!(targets.iter().any(|t| t.name == "docker/build"));
        assert!(targets.iter().any(|t| t.name == "test.unit"));
        assert!(targets.iter().any(|t| t.name == "build-all"));
    }

    #[test]
    fn test_multiline_comment_description() {
        let content = r#"
# This is a longer description
# that spans multiple lines
build:
	echo "Building..."
"#;

        let options = ParseOptions::default();
        let targets = parse_makefile_content(content, Path::new("Makefile"), &options).unwrap();

        let build = targets.iter().find(|t| t.name == "build").unwrap();
        assert_eq!(
            build.description,
            Some("This is a longer description that spans multiple lines".to_string())
        );
    }

    #[test]
    fn test_skip_target_specific_variables() {
        let content = r#"
# Target to print the highest tag version
print-highest-tag: HIGHEST_TAG:=$(shell git tag | sort -V | tail -1)
print-highest-tag:
	@echo $(HIGHEST_TAG)

build: CC := clang
build:
	$(CC) main.c
"#;

        let options = ParseOptions::default();
        let targets = parse_makefile_content(content, Path::new("Makefile"), &options).unwrap();

        // Should find the actual targets, not the variable assignment lines
        assert_eq!(targets.len(), 2);
        assert!(targets.iter().any(|t| t.name == "print-highest-tag"));
        assert!(targets.iter().any(|t| t.name == "build"));
    }

    #[test]
    fn test_is_variable_assignment() {
        assert!(is_variable_assignment("CC := gcc"));
        assert!(is_variable_assignment("CFLAGS ?= -Wall"));
        assert!(is_variable_assignment("LDFLAGS += -lm"));
        assert!(is_variable_assignment("FOO = bar"));

        // These are NOT simple variable assignments
        assert!(!is_variable_assignment("build:"));
        assert!(!is_variable_assignment("build: dep1 dep2"));
        assert!(!is_variable_assignment("target: VAR := value"));
    }

    #[test]
    fn test_is_target_specific_variable() {
        assert!(is_target_specific_variable("print-highest-tag: HIGHEST_TAG:=$(shell git tag)"));
        assert!(is_target_specific_variable("build: CC := clang"));
        assert!(is_target_specific_variable("test: CFLAGS += -g"));
        assert!(is_target_specific_variable("foo: BAR = baz"));

        // These are NOT target-specific variables
        assert!(!is_target_specific_variable("build:"));
        assert!(!is_target_specific_variable("build: dep1 dep2"));
        assert!(!is_target_specific_variable("CC := gcc"));
    }
}
