use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Represents a required variable for a Makefile target
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RequiredVar {
    /// The name of the variable (e.g., "V", "ARGS")
    pub name: String,
    /// Optional hint for possible values (e.g., "patch|minor|major")
    pub hint: Option<String>,
}

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
    /// Required variables that must be provided (e.g., V=patch|minor|major)
    #[serde(default)]
    pub required_vars: Vec<RequiredVar>,
}

impl Target {
    /// Create a new Target
    #[allow(dead_code)]
    pub fn new(name: String, description: Option<String>, file: PathBuf, line: usize) -> Self {
        Self {
            name,
            description,
            file,
            line,
            required_vars: Vec::new(),
        }
    }

    /// Create a new Target with required variables
    pub fn with_required_vars(
        name: String,
        description: Option<String>,
        file: PathBuf,
        line: usize,
        required_vars: Vec<RequiredVar>,
    ) -> Self {
        Self {
            name,
            description,
            file,
            line,
            required_vars,
        }
    }

    /// Check if this target has required variables
    pub fn has_required_vars(&self) -> bool {
        !self.required_vars.is_empty()
    }

    /// Returns a display string for the fuzzy finder
    pub fn display_name(&self) -> String {
        self.name.clone()
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
        assert!(target.required_vars.is_empty());
    }

    #[test]
    fn test_target_with_required_vars() {
        let vars = vec![
            RequiredVar {
                name: "V".to_string(),
                hint: Some("patch|minor|major".to_string()),
            },
        ];
        let target = Target::with_required_vars(
            "bump".to_string(),
            Some("Bump version".to_string()),
            PathBuf::from("Makefile"),
            10,
            vars.clone(),
        );

        assert_eq!(target.name, "bump");
        assert!(target.has_required_vars());
        assert_eq!(target.required_vars.len(), 1);
        assert_eq!(target.required_vars[0].name, "V");
        assert_eq!(target.required_vars[0].hint, Some("patch|minor|major".to_string()));
    }

    #[test]
    fn test_is_private() {
        let private_target =
            Target::new("_internal".to_string(), None, PathBuf::from("Makefile"), 1);
        let public_target = Target::new("build".to_string(), None, PathBuf::from("Makefile"), 1);

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

        // display_name now only shows the name, description is shown in preview
        assert_eq!(target.display_name(), "test");
    }

    #[test]
    fn test_display_name_without_description() {
        let target = Target::new("clean".to_string(), None, PathBuf::from("Makefile"), 3);

        assert_eq!(target.display_name(), "clean");
    }
}
