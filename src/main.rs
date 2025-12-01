mod cli;
mod executor;
mod fuzzy;
mod makefile;
mod target;

use anyhow::{Context, Result};
use clap::Parser;
use colored::Colorize;

use cli::{Cli, Commands};
use executor::ExecuteOptions;
use makefile::ParseOptions;

fn main() {
    if let Err(e) = run() {
        eprintln!("{} {}", "error:".red().bold(), e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    // Set up working directory
    let working_dir = cli.working_dir();
    if !working_dir.exists() {
        anyhow::bail!("Working directory does not exist: {}", working_dir.display());
    }

    // Parse options
    let parse_options = ParseOptions {
        include_private: cli.all,
        include_patterns: cli.patterns,
    };

    // Get targets based on whether a specific file was provided
    let targets = if let Some(ref makefile) = cli.file {
        if !makefile.exists() {
            anyhow::bail!("Makefile not found: {}", makefile.display());
        }
        makefile::parse_makefile(makefile, &parse_options)?
    } else {
        makefile::parse_all_makefiles(&working_dir, cli.recursive, &parse_options)?
    };

    if targets.is_empty() {
        println!("{}", "No targets found.".yellow());
        return Ok(());
    }

    // Handle commands
    match cli.command {
        Some(Commands::List) => {
            handle_list(&targets, cli.json)?;
        }
        Some(Commands::Pick) => {
            handle_pick(&targets, &cli)?;
        }
        Some(Commands::Run { ref target }) => {
            handle_run(target, &cli)?;
        }
        None => {
            // Default behavior: if --json or --no-ui, list targets; otherwise pick
            if cli.json || cli.no_ui {
                handle_list(&targets, cli.json)?;
            } else {
                handle_pick(&targets, &cli)?;
            }
        }
    }

    Ok(())
}

/// Handle the list command
fn handle_list(targets: &[target::Target], json_output: bool) -> Result<()> {
    if json_output {
        let json = serde_json::to_string_pretty(targets)
            .context("Failed to serialize targets to JSON")?;
        println!("{}", json);
    } else {
        let max_name_len = targets.iter().map(|t| t.name.len()).max().unwrap_or(20);

        for target in targets {
            let name = format!("{:<width$}", target.name, width = max_name_len);
            match &target.description {
                Some(desc) => {
                    println!("  {}  {}", name.green(), desc.dimmed());
                }
                None => {
                    println!("  {}", name.green());
                }
            }
        }

        println!();
        println!(
            "{} {} target(s) found",
            "→".blue(),
            targets.len().to_string().bold()
        );
    }

    Ok(())
}

/// Handle the pick command (fuzzy finder)
fn handle_pick(targets: &[target::Target], cli: &Cli) -> Result<()> {
    if cli.no_ui || cli.json {
        return handle_list(targets, cli.json);
    }

    let selected = fuzzy::select_target_with_preview(targets)?;

    match selected {
        Some(target) => {
            println!("{} {}", "Selected:".green(), target.name.bold());

            if !cli.dry_run {
                let exec_options = ExecuteOptions {
                    dry_run: cli.dry_run,
                    print_cmd: true,
                    cwd: Some(cli.working_dir()),
                    makefile: cli.file.clone(),
                };

                let status = executor::execute_target(&target.name, &exec_options)?;

                if !status.success() {
                    std::process::exit(status.code().unwrap_or(1));
                }
            } else {
                println!(
                    "{} make {}",
                    "Would run:".yellow(),
                    target.name
                );
            }
        }
        None => {
            println!("{}", "No target selected.".yellow());
        }
    }

    Ok(())
}

/// Handle the run command
fn handle_run(target_name: &str, cli: &Cli) -> Result<()> {
    let exec_options = ExecuteOptions {
        dry_run: cli.dry_run,
        print_cmd: true,
        cwd: Some(cli.working_dir()),
        makefile: cli.file.clone(),
    };

    let status = executor::execute_target(target_name, &exec_options)?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_options_from_cli() {
        let cli = Cli::parse_from(["maki", "--all", "--patterns", "list"]);

        let parse_options = ParseOptions {
            include_private: cli.all,
            include_patterns: cli.patterns,
        };

        assert!(parse_options.include_private);
        assert!(parse_options.include_patterns);
    }

    #[test]
    fn test_default_working_dir() {
        let cli = Cli::parse_from(["maki"]);
        let wd = cli.working_dir();

        // Should return current directory when not specified
        assert!(wd.exists() || wd == PathBuf::from("."));
    }
}
