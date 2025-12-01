use anyhow::{Context, Result};
use colored::Colorize;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};

/// Options for executing a make target
#[derive(Debug, Clone, Default)]
pub struct ExecuteOptions {
    /// Print the command without executing
    pub dry_run: bool,
    /// Always print the command before executing
    pub print_cmd: bool,
    /// Working directory for execution
    pub cwd: Option<std::path::PathBuf>,
    /// Custom Makefile to use
    pub makefile: Option<std::path::PathBuf>,
}

/// Execute a make target
pub fn execute_target(target: &str, options: &ExecuteOptions) -> Result<ExitStatus> {
    let cmd = build_command(target, options);
    let cmd_str = format_command(&cmd);

    if options.dry_run {
        println!("{} {}", "Would run:".yellow(), cmd_str);
        return Ok(ExitStatus::default());
    }

    if options.print_cmd {
        println!("{} {}", "Running:".green(), cmd_str);
    }

    run_make_command(target, options)
}

/// Build the command arguments
fn build_command(target: &str, options: &ExecuteOptions) -> Vec<String> {
    let mut args = vec!["make".to_string()];

    if let Some(ref makefile) = options.makefile {
        args.push("-f".to_string());
        args.push(makefile.display().to_string());
    }

    args.push(target.to_string());
    args
}

/// Format command for display
fn format_command(cmd: &[String]) -> String {
    cmd.join(" ")
}

/// Run the make command
fn run_make_command(target: &str, options: &ExecuteOptions) -> Result<ExitStatus> {
    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.arg("/C").arg("make");
        c
    } else {
        Command::new("make")
    };

    // Add makefile option if specified
    if let Some(ref makefile) = options.makefile {
        cmd.arg("-f").arg(makefile);
    }

    // Add the target
    cmd.arg(target);

    // Set working directory if specified
    if let Some(ref cwd) = options.cwd {
        cmd.current_dir(cwd);
    }

    // Inherit stdio for interactive output
    cmd.stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .stdin(Stdio::inherit());

    let status = cmd
        .status()
        .with_context(|| format!("Failed to execute 'make {}'", target))?;

    Ok(status)
}

/// Check if make is available on the system
#[allow(dead_code)]
pub fn check_make_available() -> bool {
    let result = if cfg!(windows) {
        Command::new("cmd")
            .args(["/C", "make", "--version"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
    } else {
        Command::new("make")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
    };

    result.map(|s| s.success()).unwrap_or(false)
}

/// Get the make version string
#[allow(dead_code)]
pub fn get_make_version() -> Option<String> {
    let output = if cfg!(windows) {
        Command::new("cmd")
            .args(["/C", "make", "--version"])
            .output()
            .ok()?
    } else {
        Command::new("make").arg("--version").output().ok()?
    };

    if output.status.success() {
        let version = String::from_utf8_lossy(&output.stdout);
        version.lines().next().map(|s| s.to_string())
    } else {
        None
    }
}

/// Execute a target and capture its output (for testing or scripting)
#[allow(dead_code)]
pub fn execute_target_capture(
    target: &str,
    cwd: Option<&Path>,
    makefile: Option<&Path>,
) -> Result<(String, String, ExitStatus)> {
    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.arg("/C").arg("make");
        c
    } else {
        Command::new("make")
    };

    if let Some(makefile) = makefile {
        cmd.arg("-f").arg(makefile);
    }

    cmd.arg(target);

    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }

    let output = cmd
        .output()
        .with_context(|| format!("Failed to execute 'make {}'", target))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    Ok((stdout, stderr, output.status))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_make_available() {
        // This test will pass if make is installed, which it usually is on dev machines
        // We just verify it doesn't panic
        let _ = check_make_available();
    }

    #[test]
    fn test_build_command_simple() {
        let options = ExecuteOptions::default();
        let cmd = build_command("build", &options);

        assert_eq!(cmd, vec!["make", "build"]);
    }

    #[test]
    fn test_build_command_with_makefile() {
        let options = ExecuteOptions {
            makefile: Some(std::path::PathBuf::from("custom.mk")),
            ..Default::default()
        };
        let cmd = build_command("test", &options);

        assert_eq!(cmd, vec!["make", "-f", "custom.mk", "test"]);
    }

    #[test]
    fn test_format_command() {
        let cmd = vec![
            "make".to_string(),
            "-f".to_string(),
            "Makefile".to_string(),
            "build".to_string(),
        ];

        assert_eq!(format_command(&cmd), "make -f Makefile build");
    }

    #[test]
    fn test_dry_run_does_not_execute() {
        let options = ExecuteOptions {
            dry_run: true,
            ..Default::default()
        };

        // This should succeed without actually running anything
        let result = execute_target("nonexistent_target", &options);
        assert!(result.is_ok());
    }
}
