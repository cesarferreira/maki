use anyhow::Result;
use colored::Colorize;
use dialoguer::{FuzzySelect, Input, theme::ColorfulTheme};

use crate::target::RequiredVar;

/// Prompt the user for values for required variables
/// Returns a Vec of (name, value) tuples
pub fn prompt_for_variables(required_vars: &[RequiredVar]) -> Result<Vec<(String, String)>> {
    let mut values = Vec::new();

    for var in required_vars {
        let value = prompt_single_variable(var)?;
        values.push((var.name.clone(), value));
    }

    Ok(values)
}

/// Prompt for a single variable value
fn prompt_single_variable(var: &RequiredVar) -> Result<String> {
    let theme = ColorfulTheme::default();

    // If hint contains pipe-separated values, show a selection menu
    if let Some(ref hint) = var.hint {
        let options: Vec<&str> = hint.split('|').collect();

        // If there are multiple options, let user select
        if options.len() > 1 {
            println!(
                "{} Select value for {}:",
                "?".cyan().bold(),
                var.name.green().bold()
            );

            let selection = FuzzySelect::with_theme(&theme)
                .items(&options)
                .default(0)
                .interact()?;

            return Ok(options[selection].to_string());
        }
    }

    // Otherwise, prompt for free-form input
    let prompt_msg = match &var.hint {
        Some(hint) => format!("{} (hint: {})", var.name.green().bold(), hint.dimmed()),
        None => format!("{}", var.name.green().bold()),
    };

    let value: String = Input::with_theme(&theme)
        .with_prompt(prompt_msg)
        .interact_text()?;

    Ok(value)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_hint_parsing() {
        let hint = "patch|minor|major";
        let options: Vec<&str> = hint.split('|').collect();

        assert_eq!(options.len(), 3);
        assert_eq!(options[0], "patch");
        assert_eq!(options[1], "minor");
        assert_eq!(options[2], "major");
    }

    #[test]
    fn test_single_option_hint() {
        let hint = "value";
        let options: Vec<&str> = hint.split('|').collect();

        assert_eq!(options.len(), 1);
    }
}

