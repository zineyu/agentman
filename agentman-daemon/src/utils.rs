pub fn setup_tracing() {
    tracing_subscriber::fmt::init();
}

pub fn sanitize_branch_name(name: &str) -> String {
    name.replace(" ", "-")
        .replace("/", "-")
        .replace("\\", "-")
        .replace(":", "-")
        .to_lowercase()
}
