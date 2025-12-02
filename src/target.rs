use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Represents a single Makefile target with its metadata
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Target {
    /// The name of the target (e.g., "build", "test", "clean")
    pub name: String,
    /// Optional description extracted from comments
    pub description: Option<String>,
    /// The file where this target was found
    pub file: PathBuf,
    /// The line number where the target is defined
    pub line: usize,
}

impl Target {
    /// Create a new Target
    pub fn new(name: String, description: Option<String>, file: PathBuf, line: usize) -> Self {
        Self {
            name,
            description,
            file,
            line,
        }
    }

    /// Returns a display string for the fuzzy finder
    pub fn display_name(&self) -> String {
        // ANSI bold: \x1b[1m, reset: \x1b[0m
        match &self.description {
            Some(desc) => format!("\x1b[1m{:<16}\x1b[0m {}", self.name, desc),
            None => format!("\x1b[1m{}\x1b[0m", self.name),
        }
    }

    /// Check if this is a private target (starts with underscore)
    #[allow(dead_code)]
    pub fn is_private(&self) -> bool {
        self.name.starts_with('_')
    }
}

impl std::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.description {
            Some(desc) => write!(f, "{} - {}", self.name, desc),
            None => write!(f, "{}", self.name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_creation() {
        let target = Target::new(
            "build".to_string(),
            Some("Build the project".to_string()),
            PathBuf::from("Makefile"),
            10,
        );

        assert_eq!(target.name, "build");
        assert_eq!(target.description, Some("Build the project".to_string()));
        assert_eq!(target.file, PathBuf::from("Makefile"));
        assert_eq!(target.line, 10);
    }

    #[test]
    fn test_is_private() {
        let private_target = Target::new(
            "_internal".to_string(),
            None,
            PathBuf::from("Makefile"),
            1,
        );
        let public_target = Target::new(
            "build".to_string(),
            None,
            PathBuf::from("Makefile"),
            1,
        );

        assert!(private_target.is_private());
        assert!(!public_target.is_private());
    }

    #[test]
    fn test_display_name_with_description() {
        let target = Target::new(
            "test".to_string(),
            Some("Run tests".to_string()),
            PathBuf::from("Makefile"),
            5,
        );

        let display = target.display_name();
        assert!(display.contains("test"));
        assert!(display.contains("Run tests"));
    }

    #[test]
    fn test_display_name_without_description() {
        let target = Target::new(
            "clean".to_string(),
            None,
            PathBuf::from("Makefile"),
            3,
        );

        assert_eq!(target.display_name(), "clean");
    }
}
