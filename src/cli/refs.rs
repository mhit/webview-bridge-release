/// Resolve a target string to a CSS selector.
/// - "e1", "e2" etc → `[data-wb-ref="e1"]`
/// - Anything else → passed through as CSS selector
pub fn resolve_target(target: &str) -> String {
    let trimmed = target.trim();
    if is_element_ref(trimmed) {
        format!("[data-wb-ref=\"{trimmed}\"]")
    } else {
        trimmed.to_string()
    }
}

/// Check if a string looks like an element reference (e1, e2, ...)
fn is_element_ref(s: &str) -> bool {
    s.starts_with('e') && s.len() > 1 && s[1..].chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_element_refs() {
        assert_eq!(resolve_target("e1"), "[data-wb-ref=\"e1\"]");
        assert_eq!(resolve_target("e42"), "[data-wb-ref=\"e42\"]");
        assert_eq!(resolve_target(" e5 "), "[data-wb-ref=\"e5\"]");
    }

    #[test]
    fn test_css_selectors() {
        assert_eq!(resolve_target("#submit"), "#submit");
        assert_eq!(resolve_target(".btn-primary"), ".btn-primary");
        assert_eq!(resolve_target("button"), "button");
        assert_eq!(resolve_target("email"), "email"); // not e + digits
    }
}
