//! Utility functions shared across modules

use std::collections::HashSet;
use std::path::Path;

/// Result of encoding detection and transcoding
#[derive(Debug)]
pub struct TranscodeResult {
    /// The transcoded UTF-8 string
    pub content: String,
    /// The name of the detected encoding (e.g. "windows-1252", "Shift_JIS")
    pub encoding_name: &'static str,
}

/// Detect encoding of raw bytes and transcode to UTF-8 if needed.
///
/// Returns `Ok(None)` if already valid UTF-8 (zero-copy fast path).
/// Returns `Ok(Some(TranscodeResult))` with transcoded content for non-UTF-8 text.
/// Returns `Err` if the content appears to be binary (not text in any encoding).
pub fn transcode_to_utf8(bytes: &[u8]) -> Result<Option<TranscodeResult>, &'static str> {
    // Fast path: already valid UTF-8
    if std::str::from_utf8(bytes).is_ok() {
        return Ok(None);
    }

    // Check for UTF-8 BOM (EF BB BF) — strip it and re-validate
    if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
        if let Ok(s) = std::str::from_utf8(&bytes[3..]) {
            return Ok(Some(TranscodeResult {
                content: s.to_string(),
                encoding_name: "UTF-8 (BOM)",
            }));
        }
    }

    // Check for UTF-16 BOM and decode directly
    if bytes.len() >= 2 {
        if bytes[0] == 0xFF && bytes[1] == 0xFE {
            // UTF-16 LE BOM
            let (decoded, _, had_errors) = encoding_rs::UTF_16LE.decode(bytes);
            if !had_errors {
                return Ok(Some(TranscodeResult {
                    content: decoded.into_owned(),
                    encoding_name: "UTF-16LE",
                }));
            }
        } else if bytes[0] == 0xFE && bytes[1] == 0xFF {
            // UTF-16 BE BOM
            let (decoded, _, had_errors) = encoding_rs::UTF_16BE.decode(bytes);
            if !had_errors {
                return Ok(Some(TranscodeResult {
                    content: decoded.into_owned(),
                    encoding_name: "UTF-16BE",
                }));
            }
        }
    }

    // Use chardetng to guess encoding from first 8KB
    let sample_len = bytes.len().min(8192);
    let mut detector = chardetng::EncodingDetector::new();
    detector.feed(&bytes[..sample_len], sample_len == bytes.len());

    // allow_utf8 = false because we already checked for valid UTF-8 above
    let encoding = detector.guess(None, false);

    // Decode the full content with the detected encoding
    let (decoded, _actual_encoding, had_errors) = encoding.decode(bytes);

    if had_errors {
        return Err("encoding detection produced replacement characters");
    }

    // Sanity check: make sure the transcoded result looks like text
    if is_binary_content(&decoded) {
        return Err("transcoded content appears to be binary");
    }

    Ok(Some(TranscodeResult {
        content: decoded.into_owned(),
        encoding_name: encoding.name(),
    }))
}

/// Check if raw bytes appear to be binary content.
///
/// Similar to `is_binary_content` but operates on raw `&[u8]` instead of `&str`.
/// Checks the first 8KB for null bytes or high ratio of non-printable characters.
pub fn is_binary_bytes(bytes: &[u8]) -> bool {
    let check_len = bytes.len().min(8192);
    let sample = &bytes[..check_len];

    let mut non_text_count = 0;
    for &byte in sample {
        if byte == 0 {
            return true;
        }
        if byte < 32 && !matches!(byte, b'\t' | b'\n' | b'\r') {
            non_text_count += 1;
        }
    }

    check_len > 0 && non_text_count > check_len / 10
}

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

/// Maximum line length (bytes) before we consider a file unsafe for tree-sitter parsing.
/// Deeply-minified files with extremely long lines can cause stack overflow in C parsers.
const MAX_SAFE_LINE_LENGTH: usize = 100_000;

/// Maximum nesting depth heuristic: count of unmatched open brackets in a single scan.
const MAX_NESTING_DEPTH: usize = 500;

/// Check if file content is safe to pass to tree-sitter and trigram indexing.
///
/// Returns `None` if safe, or `Some(reason)` describing why it's unsafe.
/// This acts as a gate before any tree-sitter C FFI calls to prevent crashes
/// from malformed, generated, or binary-masquerading files.
pub fn content_safety_check(content: &str) -> Option<&'static str> {
    // Check for binary content masquerading as valid UTF-8
    if is_binary_content(content) {
        return Some("appears to be binary content");
    }

    // Check for excessively long lines (minified JS/CSS, generated code).
    // Tree-sitter can stack-overflow or spend excessive time on these.
    for line in content.as_bytes().split(|&b| b == b'\n') {
        if line.len() > MAX_SAFE_LINE_LENGTH {
            return Some("contains line exceeding 100KB (likely minified/generated)");
        }
    }

    // Check for extreme nesting depth (generated code, fuzzer output).
    // Deeply nested bracket structures cause unbounded recursion in C parsers.
    let mut depth: usize = 0;
    let mut max_depth: usize = 0;
    for &b in content.as_bytes() {
        match b {
            b'{' | b'(' | b'[' => {
                depth = depth.saturating_add(1);
                if depth > max_depth {
                    max_depth = depth;
                }
            }
            b'}' | b')' | b']' => {
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }
    if max_depth > MAX_NESTING_DEPTH {
        return Some("extreme nesting depth (likely generated code)");
    }

    None
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

    #[test]
    fn test_content_safety_check_normal_file() {
        let content = "fn main() {\n    println!(\"hello\");\n}\n";
        assert!(content_safety_check(content).is_none());
    }

    #[test]
    fn test_content_safety_check_empty() {
        assert!(content_safety_check("").is_none());
    }

    #[test]
    fn test_content_safety_check_long_line() {
        let long_line = "a".repeat(200_000);
        let reason = content_safety_check(&long_line);
        assert!(reason.is_some());
        assert!(reason.unwrap().contains("minified"));
    }

    #[test]
    fn test_content_safety_check_deep_nesting() {
        let deep = "{".repeat(600) + &"}".repeat(600);
        let reason = content_safety_check(&deep);
        assert!(reason.is_some());
        assert!(reason.unwrap().contains("nesting"));
    }

    #[test]
    fn test_content_safety_check_binary() {
        let binary = "\0\0\0\0\x01\x02\x03\x04";
        let reason = content_safety_check(binary);
        assert!(reason.is_some());
        assert!(reason.unwrap().contains("binary"));
    }

    #[test]
    fn test_content_safety_check_acceptable_nesting() {
        // 100 levels of nesting should be fine
        let nested = "{".repeat(100) + &"}".repeat(100);
        assert!(content_safety_check(&nested).is_none());
    }

    #[test]
    fn test_content_safety_check_acceptable_line_length() {
        // 50KB line should be fine
        let long_line = "a".repeat(50_000);
        assert!(content_safety_check(&long_line).is_none());
    }

    // --- Encoding transcoding tests ---

    #[test]
    fn test_transcode_utf8_passthrough() {
        // Valid UTF-8 returns Ok(None) — zero-copy fast path
        let utf8 = "Hello, world! café résumé naïve";
        let result = transcode_to_utf8(utf8.as_bytes());
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_transcode_empty() {
        let result = transcode_to_utf8(b"");
        assert!(result.is_ok());
        assert!(result.unwrap().is_none()); // Empty is valid UTF-8
    }

    #[test]
    fn test_transcode_latin1() {
        // "café" in Latin-1: c=0x63, a=0x61, f=0x66, é=0xE9
        let latin1_bytes: &[u8] = &[0x63, 0x61, 0x66, 0xE9];
        let result = transcode_to_utf8(latin1_bytes);
        assert!(result.is_ok());
        let transcoded = result.unwrap();
        assert!(transcoded.is_some());
        let t = transcoded.unwrap();
        assert!(t.content.contains("caf"));
        assert_eq!(&t.content[3..], "é");
    }

    #[test]
    fn test_transcode_utf16_le_bom() {
        // UTF-16 LE with BOM: "Hello"
        // BOM FF FE, then H=48 00, e=65 00, l=6C 00, l=6C 00, o=6F 00
        let utf16le: &[u8] = &[
            0xFF, 0xFE, 0x48, 0x00, 0x65, 0x00, 0x6C, 0x00, 0x6C, 0x00, 0x6F, 0x00,
        ];
        let result = transcode_to_utf8(utf16le);
        assert!(result.is_ok());
        let transcoded = result.unwrap();
        assert!(transcoded.is_some());
        let t = transcoded.unwrap();
        // encoding_rs may include BOM replacement char, content should contain "Hello"
        assert!(t.content.contains("Hello"), "Got: {}", t.content);
        assert_eq!(t.encoding_name, "UTF-16LE");
    }

    #[test]
    fn test_transcode_utf16_be_bom() {
        // UTF-16 BE with BOM: "Hello"
        // BOM FE FF, then H=00 48, e=00 65, l=00 6C, l=00 6C, o=00 6F
        let utf16be: &[u8] = &[
            0xFE, 0xFF, 0x00, 0x48, 0x00, 0x65, 0x00, 0x6C, 0x00, 0x6C, 0x00, 0x6F,
        ];
        let result = transcode_to_utf8(utf16be);
        assert!(result.is_ok());
        let transcoded = result.unwrap();
        assert!(transcoded.is_some());
        let t = transcoded.unwrap();
        assert!(t.content.contains("Hello"), "Got: {}", t.content);
        assert_eq!(t.encoding_name, "UTF-16BE");
    }

    #[test]
    fn test_transcode_shift_jis() {
        // A longer Shift-JIS sample to give chardetng enough context for detection.
        // "日本語のテストです。これは日本語のテキストです。" in Shift-JIS
        // We encode via encoding_rs to get correct bytes.
        let text = "日本語のテストです。これは日本語のテキストです。";
        let (encoded, _, _) = encoding_rs::SHIFT_JIS.encode(text);
        let shift_jis_bytes = encoded.to_vec();

        let result = transcode_to_utf8(&shift_jis_bytes);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result);
        let transcoded = result.unwrap();
        assert!(transcoded.is_some(), "Expected Some transcoded result");
        let t = transcoded.unwrap();
        assert!(
            t.content.contains("日本語"),
            "Expected '日本語' in result, got: {}",
            t.content
        );
    }

    #[test]
    fn test_transcode_binary_rejected() {
        // Binary content with lots of null bytes — should be rejected
        let binary: &[u8] = &[0x00, 0x01, 0x02, 0x03, 0x00, 0x00, 0xFF, 0xFD];
        let result = transcode_to_utf8(binary);
        assert!(
            result.is_err(),
            "Expected Err for binary content, got: {:?}",
            result
        );
    }

    #[test]
    fn test_transcode_windows_1252() {
        // Windows-1252 "smart quotes": left double quote 0x93, right 0x94
        // Plus some normal ASCII around them
        let win1252: &[u8] = b"He said \x93hello\x94 to her.";
        let result = transcode_to_utf8(win1252);
        assert!(result.is_ok());
        let transcoded = result.unwrap();
        assert!(transcoded.is_some());
        let t = transcoded.unwrap();
        assert!(t.content.contains("hello"));
        assert!(t.content.contains("He said"));
    }

    #[test]
    fn test_is_binary_bytes() {
        // Text bytes
        assert!(!is_binary_bytes(b"Hello, world!\n"));
        assert!(!is_binary_bytes(b"fn main() { }"));

        // Binary with null bytes
        assert!(is_binary_bytes(b"Hello\0World"));

        // Pure binary
        assert!(is_binary_bytes(&[0x00, 0x01, 0x02, 0x03]));

        // Empty
        assert!(!is_binary_bytes(b""));
    }
}

/// System limits information relevant to mmap-heavy workloads
#[derive(Debug, Clone)]
pub struct SystemLimits {
    /// Current vm.max_map_count (Linux only)
    pub max_map_count: Option<u64>,
    /// Current process mmap count (Linux only)
    pub current_map_count: Option<usize>,
    /// Current open file descriptor count
    pub open_fds: Option<usize>,
    /// Max file descriptor limit (soft)
    pub max_fds: Option<u64>,
}

impl SystemLimits {
    /// Collect system limits information (Linux-specific)
    pub fn collect() -> Self {
        #[cfg(target_os = "linux")]
        {
            let max_map_count = std::fs::read_to_string("/proc/sys/vm/max_map_count")
                .ok()
                .and_then(|s| s.trim().parse().ok());

            let current_map_count = std::process::id().try_into().ok().and_then(|pid: i32| {
                std::fs::read_to_string(format!("/proc/{}/maps", pid))
                    .ok()
                    .map(|s| s.lines().count())
            });

            let open_fds = std::process::id().try_into().ok().and_then(|pid: i32| {
                std::fs::read_dir(format!("/proc/{}/fd", pid))
                    .ok()
                    .map(|entries| entries.count())
            });

            let max_fds = std::fs::read_to_string("/proc/sys/fs/file-max")
                .ok()
                .and_then(|s| s.trim().parse().ok());

            Self {
                max_map_count,
                current_map_count,
                open_fds,
                max_fds,
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            Self {
                max_map_count: None,
                current_map_count: None,
                open_fds: None,
                max_fds: None,
            }
        }
    }

    /// Check if the system is approaching mmap limits
    pub fn approaching_limit(&self) -> Option<String> {
        if let (Some(current), Some(max)) = (self.current_map_count, self.max_map_count) {
            let usage_pct = (current as f64 / max as f64) * 100.0;
            if usage_pct > 80.0 {
                return Some(format!(
                    "WARNING: High mmap usage: {}/{} ({:.1}%). Consider increasing vm.max_map_count",
                    current, max, usage_pct
                ));
            }
        }
        None
    }

    /// Calculate safe mmap limit (85% of max to leave headroom)
    pub fn safe_mmap_limit(&self) -> Option<usize> {
        self.max_map_count.map(|max| ((max as f64) * 0.85) as usize)
    }

    /// Check if we can safely add more mmaps
    pub fn can_allocate_more(&self, additional: usize) -> bool {
        if let (Some(current), Some(safe_limit)) = (self.current_map_count, self.safe_mmap_limit())
        {
            return current + additional < safe_limit;
        }
        // If we can't determine limits (non-Linux), allow it
        true
    }

    /// Log current system limits
    pub fn log_limits(&self) {
        tracing::info!(
            max_map_count = ?self.max_map_count,
            current_map_count = ?self.current_map_count,
            open_fds = ?self.open_fds,
            max_fds = ?self.max_fds,
            "System resource limits"
        );

        if let Some(warning) = self.approaching_limit() {
            tracing::warn!("{}", warning);
        }
    }

    /// Check if system limits are too low and provide actionable advice
    pub fn check_and_warn(&self) -> Option<String> {
        #[cfg(target_os = "linux")]
        {
            if let Some(max_map) = self.max_map_count {
                if max_map < 131072 {
                    return Some(format!(
                        "\n⚠️  WARNING: vm.max_map_count is {} (very low!)\n\
                        \n\
                        This will cause 'cannot allocate memory' errors when indexing large codebases.\n\
                        \n\
                        === SOLUTIONS (choose one) ===\n\
                        \n\
                        1. WITH sudo: Increase system limit (recommended)\n\
                           sudo sysctl -w vm.max_map_count=524288\n\
                           echo 'vm.max_map_count=524288' | sudo tee -a /etc/sysctl.conf\n\
                        \n\
                        2. WITHOUT sudo: Enable low_memory_mode in config.toml\n\
                           [indexer]\n\
                           low_memory_mode = true\n\
                           max_file_size = 2097152  # 2MB to further reduce memory\n\
                           exclude_patterns = [\"node_modules\", \"target\", \".git\", \"build\"]\n\
                        \n\
                        Low memory mode trades some search performance for the ability to index\n\
                        larger codebases without hitting system limits.\n",
                        max_map
                    ));
                }
            }
        }
        None
    }
}

/// Check if an error is a memory mapping limit error and provide helpful context
pub fn diagnose_mmap_error(error: &std::io::Error, file_path: &str) -> String {
    let error_str = error.to_string().to_lowercase();
    let is_mmap_limit = error_str.contains("cannot allocate memory")
        || error_str.contains("out of memory")
        || error_str.contains("too many open files")
        || error_str.contains("resource temporarily unavailable");

    if !is_mmap_limit {
        return format!("Failed to mmap {}: {}", file_path, error);
    }

    // This looks like a resource limit issue
    let _limits = SystemLimits::collect();
    let mut msg = format!("Memory mapping failed for {}: {}", file_path, error);

    #[cfg(target_os = "linux")]
    {
        msg.push_str("\n\n=== SYSTEM RESOURCE LIMITS ===");

        if let Some(max_map) = _limits.max_map_count {
            msg.push_str(&format!("\nvm.max_map_count: {}", max_map));
            if max_map < 262144 {
                msg.push_str(" (TOO LOW - recommend 524288)");
            }
        }

        if let Some(current) = _limits.current_map_count {
            msg.push_str(&format!("\nCurrent mmap count: {}", current));
            if let Some(max_map) = _limits.max_map_count {
                let pct = (current as f64 / max_map as f64) * 100.0;
                msg.push_str(&format!(" ({:.1}% of limit)", pct));
            }
        }

        if let Some(open) = _limits.open_fds {
            msg.push_str(&format!("\nOpen file descriptors: {}", open));
        }

        msg.push_str("\n\n=== RECOMMENDED FIXES ===");
        msg.push_str("\n1. WITH sudo - Increase mmap limit:");
        msg.push_str("\n   sudo sysctl -w vm.max_map_count=524288");
        msg.push_str("\n   echo 'vm.max_map_count=524288' | sudo tee -a /etc/sysctl.conf");
        msg.push_str("\n\n2. WITHOUT sudo - Enable low_memory_mode in config.toml:");
        msg.push_str("\n   [indexer]");
        msg.push_str("\n   low_memory_mode = true");
        msg.push_str("\n   max_file_size = 2097152  # 2MB");
        msg.push_str("\n\n3. Reduce scope with exclude_patterns in config.toml");
    }

    #[cfg(not(target_os = "linux"))]
    {
        msg.push_str(
            "\n\nThis appears to be a system resource limit. Check your OS documentation.",
        );
    }

    msg
}
