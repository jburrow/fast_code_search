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

/// A UTF-8 byte order mark (U+FEFF) at the start of a file.
const UTF8_BOM: char = '\u{feff}';

/// `s` without a leading UTF-8 byte order mark. The mark is invisible in
/// editors, so it is not treated as text: `^` matches at the start of line 1
/// and results do not show a stray character, as in ripgrep.
pub fn strip_utf8_bom(s: &str) -> &str {
    s.strip_prefix(UTF8_BOM).unwrap_or(s)
}

/// [`strip_utf8_bom`] for an owned string, in place.
pub fn strip_utf8_bom_owned(mut s: String) -> String {
    if s.starts_with(UTF8_BOM) {
        s.drain(..UTF8_BOM.len_utf8());
    }
    s
}

/// Detect encoding of raw bytes and transcode to UTF-8 if needed.
///
/// Returns `Ok(None)` if already valid UTF-8 (zero-copy fast path; the caller
/// strips a byte order mark with [`strip_utf8_bom`]).
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

/// Check if raw bytes appear to be binary content: a NUL byte in the
/// first 8 KB, ripgrep's own test (see [`is_binary_content`]).
pub fn is_binary_bytes(bytes: &[u8]) -> bool {
    bytes[..bytes.len().min(BINARY_SNIFF_BYTES)].contains(&0)
}

/// How much of a file the binary test looks at.
const BINARY_SNIFF_BYTES: usize = 8192;

/// Binary file extensions to skip during indexing.
/// These are common binary formats that don't contain searchable text.
pub const BINARY_EXTENSIONS: &[&str] = &[
    "exe", "dll", "so", "dylib", "bin", "o", "a", "lib", // Executables and libraries
    // Not `svg`: it is XML text, and ripgrep searches it.
    "png", "jpg", "jpeg", "gif", "ico", "bmp", "webp", // Images
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
        BINARY_EXTENSIONS.contains(&ext.as_str())
    } else {
        false
    }
}

/// Check if content appears to be binary: a NUL byte in its first 8 KB.
///
/// That is ripgrep's test (a NUL byte), so a file rg searches is indexed.
/// A second rule, more than 10% control bytes, used to drop text files rg
/// searches (logs and snapshots full of ANSI colour codes) without saying
/// so.
pub fn is_binary_content(content: &str) -> bool {
    // Operate on the byte slice so a multi-byte character at the 8 KB
    // boundary cannot cause a panic.
    is_binary_bytes(content.as_bytes())
}

/// Maximum line length (bytes) before we consider a file unsafe for tree-sitter parsing.
/// Deeply-minified files with extremely long lines can cause stack overflow in C parsers.
const MAX_SAFE_LINE_LENGTH: usize = 100_000;

/// Maximum nesting depth heuristic: count of unmatched open brackets in a single scan.
const MAX_NESTING_DEPTH: usize = 500;

/// Check if file content is safe to pass to tree-sitter *and* to index at all.
///
/// Returns `None` if safe, or `Some(reason)` describing why it's unsafe.
/// This is the combined gate: binary-masquerading content (which should not
/// be indexed at all) plus the structural checks in
/// [`tree_sitter_safety_check`] (which only exclude a file from symbol
/// extraction). Prefer calling the two halves separately on the indexing
/// path so that large-line / deeply nested files stay text-searchable.
pub fn content_safety_check(content: &str) -> Option<&'static str> {
    if is_binary_content(content) {
        return Some("appears to be binary content");
    }
    tree_sitter_safety_check(content)
}

/// Structural checks that make a file unsafe (or pointless) to hand to the
/// tree-sitter C parsers: extremely long lines and extreme bracket nesting.
///
/// Returns `None` if safe, or `Some(reason)`. A file failing this check is
/// still perfectly valid text and MUST remain in the trigram index; it only
/// skips symbol/import extraction.
pub fn tree_sitter_safety_check(content: &str) -> Option<&'static str> {
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
/// The name each indexed root is shown under in result paths: its folder
/// name (`/code/app` -> `app`), or, when several roots share that name,
/// as many trailing folders as it takes to tell them apart
/// (`/one/app` -> `one/app`, `/two/app` -> `two/app`). Two roots shown under
/// one name gave identical paths, and opening either hit showed the same
/// file. A root with no folder name (`/`) gets an empty label.
pub fn root_labels<P: AsRef<Path>>(roots: &[P]) -> Vec<String> {
    let parts: Vec<Vec<String>> = roots
        .iter()
        .map(|r| {
            r.as_ref()
                .components()
                .filter_map(|c| match c {
                    std::path::Component::Normal(n) => Some(n.to_string_lossy().into_owned()),
                    _ => None,
                })
                .collect()
        })
        .collect();
    let label = |i: usize, depth: usize| -> String {
        let p = &parts[i];
        let from = p.len().saturating_sub(depth);
        p[from..].join("/")
    };
    let mut depth = vec![1usize; roots.len()];
    // Lengthen every clashing label by one folder until none clash (or a
    // path runs out of folders).
    loop {
        let labels: Vec<String> = (0..roots.len()).map(|i| label(i, depth[i])).collect();
        let mut grew = false;
        for i in 0..roots.len() {
            let clashes = (0..roots.len()).any(|j| j != i && labels[j] == labels[i]);
            if clashes && depth[i] < parts[i].len() {
                depth[i] += 1;
                grew = true;
            }
        }
        if !grew {
            return labels;
        }
    }
}

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
    fn root_labels_tell_same_named_roots_apart() {
        assert_eq!(root_labels(&["/code/app", "/srv/lib"]), vec!["app", "lib"]);
        assert_eq!(
            root_labels(&["/one/app", "/two/app", "/x/other"]),
            vec!["one/app", "two/app", "other"]
        );
        // Clashing two folders up as well.
        assert_eq!(
            root_labels(&["/a/w/app", "/b/w/app"]),
            vec!["a/w/app", "b/w/app"]
        );
        assert_eq!(root_labels(&["/"]), vec![""]);
    }

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

        // Control characters (it holds a NUL) are binary...
        let binary_like = (0..100).map(|i| (i % 32) as u8 as char).collect::<String>();
        assert!(is_binary_content(&binary_like));
        // ...but text dense with ANSI colour codes is not: rg searches it.
        let ansi = "\x1b[31mE\x1b[0m \x1b[1mx\x1b[0m\n".repeat(50);
        assert!(!is_binary_content(&ansi));
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
        assert_eq!(format_bytes(0), "0 bytes");
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
    fn test_strip_utf8_bom() {
        assert_eq!(strip_utf8_bom("\u{feff}abc"), "abc");
        assert_eq!(strip_utf8_bom("abc\u{feff}"), "abc\u{feff}");
        assert_eq!(strip_utf8_bom_owned("\u{feff}abc".to_string()), "abc");
        assert_eq!(strip_utf8_bom_owned(String::new()), "");
    }

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

/// Process limits worth logging at startup: a million-file tree opens many
/// files while it is indexed and watched.
#[derive(Debug, Clone)]
pub struct SystemLimits {
    /// Current open file descriptor count
    pub open_fds: Option<usize>,
    /// System-wide file descriptor limit
    pub max_fds: Option<u64>,
}

impl SystemLimits {
    /// Collect system limits information (Linux-specific)
    pub fn collect() -> Self {
        #[cfg(target_os = "linux")]
        {
            let open_fds = std::fs::read_dir("/proc/self/fd")
                .ok()
                .map(|entries| entries.count());
            let max_fds = std::fs::read_to_string("/proc/sys/fs/file-max")
                .ok()
                .and_then(|s| s.trim().parse().ok());
            Self { open_fds, max_fds }
        }
        #[cfg(not(target_os = "linux"))]
        {
            Self {
                open_fds: None,
                max_fds: None,
            }
        }
    }

    /// Log current system limits
    pub fn log_limits(&self) {
        tracing::info!(
            open_fds = ?self.open_fds,
            max_fds = ?self.max_fds,
            "System resource limits"
        );
    }
}
