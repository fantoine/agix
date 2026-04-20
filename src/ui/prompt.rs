use crate::drivers::all_drivers;
use crate::error::{AgixError, Result};
use std::io::IsTerminal;

/// Decide whether the caller is running in a non-interactive context.
///
/// Order of precedence (first hit wins):
/// 1. Explicit per-command `--no-interactive` flag (via `non_interactive` param).
/// 2. `AGIX_NO_INTERACTIVE` env var set to any non-empty value.
/// 3. TTY auto-detect: stderr is not a terminal (piped stdin, CI, etc.).
///
/// The TTY path is stable Rust 1.70+ (`std::io::IsTerminal`). We probe stderr
/// rather than stdin because dialoguer writes its prompt to stderr; stdin can
/// be a TTY while stderr is a pipe, which would still block rendering.
pub fn is_non_interactive(non_interactive: bool) -> bool {
    if non_interactive {
        return true;
    }
    if std::env::var("AGIX_NO_INTERACTIVE")
        .map(|v| !v.is_empty())
        .unwrap_or(false)
    {
        return true;
    }
    !std::io::stderr().is_terminal()
}

/// Select which CLIs the user wants to manage via agix.
///
/// - If the caller is in a non-interactive context (see [`is_non_interactive`]),
///   return `preselected` (validated against known drivers).
/// - Otherwise show a `dialoguer::MultiSelect` with all drivers listed;
///   default-check detected drivers plus anything in `preselected`.
pub fn pick_clis(preselected: &[String], non_interactive: bool) -> Result<Vec<String>> {
    let drivers = all_drivers();
    let all_names: Vec<String> = drivers.iter().map(|d| d.name().to_string()).collect();

    if is_non_interactive(non_interactive) {
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

    // Inline keybinding hint: dialoguer renders items directly under the prompt
    // line and doesn't support post-item captions, so we keep the hint adjacent
    // to the menu by appending it to the prompt itself.
    let picked = dialoguer::MultiSelect::new()
        .with_prompt("Select CLIs to manage with agix (space=toggle, enter=confirm, esc=cancel)")
        .items(&labels)
        .defaults(&default_selected)
        .interact_opt()
        .map_err(|e| {
            // Even with the TTY auto-detect guard above, dialoguer can still
            // fail (e.g. stderr is a TTY but stdin isn't, or some exotic
            // terminal quirk). Point the user at the escape hatches so the
            // opaque "IO error: not a terminal" message is never the last word.
            AgixError::Other(format!(
                "prompt failed: {e}. Set AGIX_NO_INTERACTIVE=1 or pass \
                 --no-interactive (on `agix init`) to skip the interactive menu."
            ))
        })?;

    let Some(picked) = picked else {
        return Err(AgixError::Other("selection cancelled".to_string()));
    };

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
