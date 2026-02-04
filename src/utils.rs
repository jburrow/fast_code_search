//! Utility functions shared across modules

/// Normalize a path string for cross-platform comparison.
///
/// This function converts all path separators to forward slashes and lowercases
/// the path for case-insensitive comparison. This approach provides lenient
/// comparison of user-provided config paths across all platforms, preventing
/// duplicate path entries due to case or separator variations.
///
/// # Platform Behavior
///
/// While Windows filesystems are case-insensitive and Unix filesystems are
/// case-sensitive, this function always returns lowercase paths for consistent
/// comparison behavior. This is a deliberate design choice for config path
/// matching to be more forgiving of user input variations.
///
/// # Examples
///
/// ```
/// use fast_code_search::utils::normalize_path_for_comparison;
///
/// assert_eq!(normalize_path_for_comparison("C:\\Users\\Dev"), "c:/users/dev");
/// assert_eq!(normalize_path_for_comparison("/home/dev"), "/home/dev");
/// assert_eq!(normalize_path_for_comparison("/Home/Dev"), "/home/dev");
/// ```
pub fn normalize_path_for_comparison(path: &str) -> String {
    // Convert backslashes to forward slashes for consistent comparison
    let normalized = path.replace('\\', "/");
    // Always lowercase for lenient comparison regardless of platform
    normalized.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_windows_path() {
        assert_eq!(
            normalize_path_for_comparison("C:\\Users\\Developer\\project"),
            "c:/users/developer/project"
        );
    }

    #[test]
    fn test_normalize_unix_path() {
        assert_eq!(
            normalize_path_for_comparison("/home/developer/project"),
            "/home/developer/project"
        );
    }

    #[test]
    fn test_normalize_mixed_case() {
        assert_eq!(
            normalize_path_for_comparison("/Home/Developer/Project"),
            "/home/developer/project"
        );
    }

    #[test]
    fn test_normalize_empty_path() {
        assert_eq!(normalize_path_for_comparison(""), "");
    }
}
