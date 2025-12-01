use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// A cross-platform fuzzy Makefile task finder
#[derive(Parser, Debug)]
#[command(name = "maki")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Use a custom Makefile
    #[arg(short = 'f', long = "file", global = true)]
    pub file: Option<PathBuf>,

    /// Include private targets (those starting with _)
    #[arg(long = "all", global = true)]
    pub all: bool,

    /// Include pattern rules (e.g., %.o: %.c)
    #[arg(long = "patterns", global = true)]
    pub patterns: bool,

    /// Output results as JSON
    #[arg(long = "json", global = true)]
    pub json: bool,

    /// Skip the fuzzy finder UI
    #[arg(long = "no-ui", global = true)]
    pub no_ui: bool,

    /// Scan subdirectories for Makefiles
    #[arg(long = "recursive", short = 'r', global = true)]
    pub recursive: bool,

    /// Print command without executing
    #[arg(long = "dry-run", global = true)]
    pub dry_run: bool,

    /// Set the working directory
    #[arg(long = "cwd", global = true)]
    pub cwd: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Interactive fuzzy search to pick a target
    Pick,

    /// List all available targets
    List,

    /// Run a specific target directly
    Run {
        /// The target name to run
        target: String,
    },
}

impl Cli {
    /// Get the working directory, defaulting to current directory
    pub fn working_dir(&self) -> PathBuf {
        self.cwd.clone().unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        })
    }

    /// Get the Makefile path if explicitly specified
    pub fn makefile_path(&self) -> Option<PathBuf> {
        self.file.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn test_cli_parses() {
        // Just verify the CLI can be constructed
        Cli::command().debug_assert();
    }

    #[test]
    fn test_parse_pick_command() {
        let cli = Cli::parse_from(["maki", "pick"]);
        assert!(matches!(cli.command, Some(Commands::Pick)));
    }

    #[test]
    fn test_parse_list_command() {
        let cli = Cli::parse_from(["maki", "list"]);
        assert!(matches!(cli.command, Some(Commands::List)));
    }

    #[test]
    fn test_parse_run_command() {
        let cli = Cli::parse_from(["maki", "run", "build"]);
        if let Some(Commands::Run { target }) = cli.command {
            assert_eq!(target, "build");
        } else {
            panic!("Expected Run command");
        }
    }

    #[test]
    fn test_parse_global_flags() {
        let cli = Cli::parse_from([
            "maki",
            "--all",
            "--patterns",
            "--json",
            "--recursive",
            "--dry-run",
            "list",
        ]);

        assert!(cli.all);
        assert!(cli.patterns);
        assert!(cli.json);
        assert!(cli.recursive);
        assert!(cli.dry_run);
    }

    #[test]
    fn test_parse_file_option() {
        let cli = Cli::parse_from(["maki", "-f", "custom.mk", "list"]);
        assert_eq!(cli.file, Some(PathBuf::from("custom.mk")));
    }

    #[test]
    fn test_parse_cwd_option() {
        let cli = Cli::parse_from(["maki", "--cwd", "/tmp", "list"]);
        assert_eq!(cli.cwd, Some(PathBuf::from("/tmp")));
    }

    #[test]
    fn test_default_command_is_none() {
        let cli = Cli::parse_from(["maki"]);
        assert!(cli.command.is_none());
    }
}
