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
        let mut out: Vec<String> = Vec::with_capacity(preselected.len());
        for cli in preselected {
            if !all_names.contains(cli) {
                return Err(AgixError::Other(format!(
                    "unknown CLI '{}' (known: {})",
                    cli,
                    all_names.join(", ")
                )));
            }
            if !out.contains(cli) {
                out.push(cli.clone());
            }
        }
        return Ok(out);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_interactive_empty_preselect_returns_empty() {
        let out = pick_clis(&[], true).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn non_interactive_passes_through_known_clis() {
        let out = pick_clis(&["claude".into(), "codex".into()], true).unwrap();
        assert_eq!(out, vec!["claude".to_string(), "codex".to_string()]);
    }

    #[test]
    fn non_interactive_rejects_unknown_cli() {
        let err = pick_clis(&["bogus".into()], true).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("unknown CLI 'bogus'"), "got: {msg}");
    }

    #[test]
    fn non_interactive_deduplicates_preselect() {
        let out = pick_clis(&["claude".into(), "claude".into(), "codex".into()], true).unwrap();
        assert_eq!(out, vec!["claude".to_string(), "codex".to_string()]);
    }

    #[test]
    fn non_interactive_preserves_order() {
        let out = pick_clis(&["codex".into(), "claude".into()], true).unwrap();
        assert_eq!(out, vec!["codex".to_string(), "claude".to_string()]);
    }
}
