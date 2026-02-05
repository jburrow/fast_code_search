//! Utility functions shared across modules

use std::collections::HashSet;
use std::path::Path;

/// Binary file extensions to skip during indexing.
/// These are common binary formats that don't contain searchable text.
pub const BINARY_EXTENSIONS: &[&str] = &[
    "exe", "dll", "so", "dylib", "bin", "o", "a", "lib", // Executables and libraries
    "png", "jpg", "jpeg", "gif", "ico", "bmp", "webp", "svg", // Images
    "zip", "tar", "gz", "7z", "rar", "xz", "bz2", // Archives
    "woff", "woff2", "ttf", "eot", "otf", // Fonts
    "pdf", "doc", "docx", "xls", "xlsx", // Documents
    "mp3", "mp4", "wav", "avi", "mkv", "mov", // Media
    "pyc", "pyo", "class", // Compiled bytecode
];

/// Returns a HashSet of binary extensions for efficient lookup.
pub fn get_binary_extensions() -> HashSet<&'static str> {
    BINARY_EXTENSIONS.iter().copied().collect()
}

/// Check if a file should be skipped based on its extension.
pub fn has_binary_extension(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        let ext = ext.to_string_lossy().to_lowercase();
        get_binary_extensions().contains(ext.as_str())
    } else {
        false
    }
}

/// Check if content appears to be binary (contains null bytes or high ratio of non-printable chars).
///
/// This checks the first 8KB of content for binary indicators:
/// - Null bytes are a strong indicator of binary content
/// - More than 10% non-printable characters suggests binary content
pub fn is_binary_content(content: &str) -> bool {
    // Check first 8KB for binary indicators
    let check_len = content.len().min(8192);
    let sample = &content[..check_len];

    let mut non_text_count = 0;
    for byte in sample.bytes() {
        // Null bytes are a strong indicator of binary content
        if byte == 0 {
            return true;
        }
        // Count non-printable, non-whitespace characters (excluding common control chars)
        if byte < 32 && !matches!(byte, b'\t' | b'\n' | b'\r') {
            non_text_count += 1;
        }
    }

    // If more than 10% non-text characters, likely binary
    non_text_count > check_len / 10
}

/// Format a number with underscore separators for readability (e.g., 89210 -> "89_210")
pub fn format_number(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push('_');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

/// Format bytes for human readability (e.g., 1048576 -> "1.00 MB")
pub fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.2} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.2} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.2} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} bytes", bytes)
    }
}

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

    #[test]
    fn test_binary_extensions() {
        let extensions = get_binary_extensions();
        assert!(extensions.contains("exe"));
        assert!(extensions.contains("png"));
        assert!(extensions.contains("zip"));
        assert!(!extensions.contains("rs"));
        assert!(!extensions.contains("txt"));
    }

    #[test]
    fn test_has_binary_extension() {
        use std::path::Path;
        assert!(has_binary_extension(Path::new("file.exe")));
        assert!(has_binary_extension(Path::new("image.PNG"))); // case insensitive
        assert!(!has_binary_extension(Path::new("code.rs")));
        assert!(!has_binary_extension(Path::new("README")));
    }

    #[test]
    fn test_is_binary_content() {
        // Text content
        assert!(!is_binary_content("Hello, world!\n"));
        assert!(!is_binary_content(
            "fn main() {\n    println!(\"test\");\n}\n"
        ));

        // Binary content with null byte
        assert!(is_binary_content("Hello\0World"));

        // Content with many control characters (simulating binary)
        let binary_like = (0..100).map(|i| (i % 32) as u8 as char).collect::<String>();
        assert!(is_binary_content(&binary_like));
    }

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(100), "100");
        assert_eq!(format_number(1000), "1_000");
        assert_eq!(format_number(1234567), "1_234_567");
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 bytes");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1048576), "1.00 MB");
        assert_eq!(format_bytes(1073741824), "1.00 GB");
    }
}
