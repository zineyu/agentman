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

/// Strip ANSI escape sequences from a string.
/// Handles codes like \x1b[0m, \x1b[31m, \x1b[1;32m, etc.
pub fn strip_ansi_codes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next(); // skip '['
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            result.push(ch);
        }
    }

    result
}
