mod cache;
mod cli;
mod executor;
mod fuzzy;
mod makefile;
mod prompt;
mod target;

use anyhow::{Context, Result};
use clap::Parser;
use colored::Colorize;

use cache::Cache;
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
        anyhow::bail!(
            "Working directory does not exist: {}",
            working_dir.display()
        );
    }

    // Parse options
    let parse_options = ParseOptions {
        include_private: cli.all,
        include_patterns: cli.patterns,
    };

    // Get targets (with caching unless --no-cache is specified)
    let targets = get_targets(&cli, &working_dir, &parse_options)?;

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
            handle_run(target, &targets, &cli)?;
        }
        None => {
            // Default behavior: start interactive picker (unless --json or --no-ui)
            if cli.json || cli.no_ui {
                handle_list(&targets, cli.json)?;
            } else {
                handle_pick(&targets, &cli)?;
            }
        }
    }

    Ok(())
}

/// Get targets with caching support
fn get_targets(
    cli: &Cli,
    working_dir: &std::path::Path,
    parse_options: &ParseOptions,
) -> Result<Vec<target::Target>> {
    // If a specific file is provided
    if let Some(ref makefile) = cli.file {
        if !makefile.exists() {
            anyhow::bail!("Makefile not found: {}", makefile.display());
        }
        return get_targets_for_file(makefile, parse_options, cli.no_cache);
    }

    // Find all Makefiles
    let makefiles = makefile::find_makefiles(working_dir, cli.recursive);
    if makefiles.is_empty() {
        anyhow::bail!("No Makefile found in {}", working_dir.display());
    }

    // Load cache
    let mut cache = if cli.no_cache {
        Cache::new()
    } else {
        Cache::load().unwrap_or_else(|_| Cache::new())
    };

    let mut all_targets = Vec::new();
    let mut seen_names = std::collections::HashSet::new();
    let mut cache_modified = false;

    for makefile_path in &makefiles {
        let targets = if cli.no_cache {
            // Skip cache, parse directly
            makefile::parse_makefile(makefile_path, parse_options)?
        } else if let Some(cached_targets) = cache.get(makefile_path) {
            // Use cached targets
            cached_targets.clone()
        } else {
            // Parse and cache
            let parsed = makefile::parse_makefile(makefile_path, parse_options)?;
            cache.set(makefile_path, parsed.clone())?;
            cache_modified = true;
            parsed
        };

        for target in targets {
            if !seen_names.contains(&target.name) {
                seen_names.insert(target.name.clone());
                all_targets.push(target);
            }
        }
    }

    // Save cache if modified
    if cache_modified && !cli.no_cache {
        let _ = cache.save(); // Ignore save errors, caching is best-effort
    }

    // Sort targets alphabetically
    all_targets.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(all_targets)
}

/// Get targets for a single file with caching support
fn get_targets_for_file(
    makefile: &std::path::Path,
    parse_options: &ParseOptions,
    no_cache: bool,
) -> Result<Vec<target::Target>> {
    if no_cache {
        return makefile::parse_makefile(makefile, parse_options);
    }

    let mut cache = Cache::load().unwrap_or_else(|_| Cache::new());

    if let Some(cached_targets) = cache.get(makefile) {
        return Ok(cached_targets.clone());
    }

    let targets = makefile::parse_makefile(makefile, parse_options)?;
    cache.set(makefile, targets.clone())?;
    let _ = cache.save();

    Ok(targets)
}

/// Handle the list command
fn handle_list(targets: &[target::Target], json_output: bool) -> Result<()> {
    if json_output {
        let json =
            serde_json::to_string_pretty(targets).context("Failed to serialize targets to JSON")?;
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

            // Prompt for required variables if any
            let variables = if target.has_required_vars() {
                prompt::prompt_for_variables(&target.required_vars)?
            } else {
                Vec::new()
            };

            if !cli.dry_run {
                let exec_options = ExecuteOptions {
                    dry_run: cli.dry_run,
                    print_cmd: true,
                    cwd: Some(cli.working_dir()),
                    makefile: cli.file.clone(),
                    variables,
                };

                let status = executor::execute_target(&target.name, &exec_options)?;

                if !status.success() {
                    std::process::exit(status.code().unwrap_or(1));
                }
            } else {
                let vars_str = if !variables.is_empty() {
                    format!(
                        " {}",
                        variables
                            .iter()
                            .map(|(k, v)| format!("{}={}", k, v))
                            .collect::<Vec<_>>()
                            .join(" ")
                    )
                } else {
                    String::new()
                };
                println!("{} make {}{}", "Would run:".yellow(), target.name, vars_str);
            }
        }
        None => {
            println!("{}", "No target selected.".yellow());
        }
    }

    Ok(())
}

/// Handle the run command
fn handle_run(target_name: &str, targets: &[target::Target], cli: &Cli) -> Result<()> {
    // Find the target to check for required variables
    let target = targets.iter().find(|t| t.name == target_name);

    // Prompt for required variables if any
    let variables = if let Some(t) = target {
        if t.has_required_vars() {
            prompt::prompt_for_variables(&t.required_vars)?
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    let exec_options = ExecuteOptions {
        dry_run: cli.dry_run,
        print_cmd: true,
        cwd: Some(cli.working_dir()),
        makefile: cli.file.clone(),
        variables,
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

    #[test]
    fn test_no_cache_flag() {
        let cli = Cli::parse_from(["maki", "--no-cache"]);
        assert!(cli.no_cache);
    }

    #[test]
    fn test_default_command_starts_picker() {
        let cli = Cli::parse_from(["maki"]);
        // When command is None and not --json/--no-ui, it should start picker
        assert!(cli.command.is_none());
        assert!(!cli.json);
        assert!(!cli.no_ui);
    }
}
