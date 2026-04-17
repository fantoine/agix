pub fn success(msg: &str) {
    println!("  \u{2713} {}", msg);
}
pub fn warn(msg: &str) {
    eprintln!("  \u{26a0} {}", msg);
}
pub fn info(msg: &str) {
    println!("  {}", msg);
}
