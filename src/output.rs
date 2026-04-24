pub fn success(msg: &str) {
    println!("  \u{2713} {}", msg);
}
pub fn warn(msg: &str) {
    eprintln!("  \u{26a0} {}", msg);
}
pub fn info(msg: &str) {
    println!("  {}", msg);
}

pub fn stderr_is_tty() -> bool {
    use std::io::IsTerminal;
    std::io::stderr().is_terminal()
}

/// Print the resolved scope as a dim header line to stderr.
/// Suppressed when stderr is not a TTY (CI, pipes).
pub fn scope_header(agentfile: &std::path::Path, global: bool) {
    if !stderr_is_tty() {
        return;
    }
    let label = if global { "global" } else { "project" };
    eprintln!(
        "  \x1b[2mUsing {}   ({})\x1b[0m",
        agentfile.display(),
        label
    );
}
