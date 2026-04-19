use crate::drivers::all_drivers;
use crate::error::{AgixError, Result};

/// Select which CLIs the user wants to manage via agix.
///
/// - If `non_interactive` is true OR `AGIX_NO_INTERACTIVE=1` is set, return
///   `preselected` (validated against known drivers).
/// - Otherwise show a `dialoguer::MultiSelect` with all drivers listed;
///   default-check detected drivers plus anything in `preselected`.
pub fn pick_clis(preselected: &[String], non_interactive: bool) -> Result<Vec<String>> {
    let drivers = all_drivers();
    let all_names: Vec<String> = drivers.iter().map(|d| d.name().to_string()).collect();

    if non_interactive || std::env::var("AGIX_NO_INTERACTIVE").is_ok() {
        for cli in preselected {
            if !all_names.contains(cli) {
                return Err(AgixError::Other(format!(
                    "unknown CLI '{}' (known: {})",
                    cli,
                    all_names.join(", ")
                )));
            }
        }
        return Ok(preselected.to_vec());
    }

    let default_selected: Vec<bool> = drivers
        .iter()
        .map(|d| preselected.contains(&d.name().to_string()) || d.detect())
        .collect();

    let labels: Vec<String> = drivers
        .iter()
        .map(|d| {
            let tag = if d.detect() { " (detected)" } else { "" };
            format!("{}{}", d.name(), tag)
        })
        .collect();

    let picked = dialoguer::MultiSelect::new()
        .with_prompt("Select CLIs to manage with agix")
        .items(&labels)
        .defaults(&default_selected)
        .interact()
        .map_err(|e| AgixError::Other(format!("prompt failed: {e}")))?;

    Ok(picked.iter().map(|&i| all_names[i].clone()).collect())
}
