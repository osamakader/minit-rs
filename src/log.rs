pub fn debug_logs_enabled() -> bool {
    std::env::var("MINIT_DEBUG")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}
