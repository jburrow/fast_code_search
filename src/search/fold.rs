//! Case folding for case-insensitive text search, the way ripgrep's `-i`
//! does it: Unicode **simple** case folding, one character to one
//! character.
//!
//! Two characters match case-insensitively when [`fold_char`] maps them to
//! the same character. That is `char::to_lowercase` (when it gives a
//! single character) plus the few characters whose fold is not their
//! lowercase: the micro sign `µ` is `μ`, the long s `ſ` is `s`, the final
//! sigma `ς` is `σ`, the Greek symbol forms (`ϐ ϑ ϕ ϖ ϰ ϱ ϵ`), the old
//! Cyrillic letter forms (U+1C80..U+1C88), three precomposed letters that
//! Unicode 14 made equivalent, and Cherokee, whose fold is its capital
//! letters. A test checks the table against regex-syntax for every
//! character. A character whose lowercase is several characters
//! (`İ`) folds to itself, so `ß` is never `ss` and `İ` is never `i`.
//!
//! The trigram index stores folded text and queries are folded the same
//! way, so a file is a candidate exactly when its folded text holds the
//! folded query. Only two non-ASCII characters fold to ASCII: `ſ` (to `s`)
//! and the Kelvin sign `K` (to `k`); [`ascii_fold_suffices`] says when the
//! byte-wise ASCII scan can skip them.

use std::borrow::Cow;

/// The case fold of `c`: equal for exactly the characters ripgrep's `-i`
/// treats as equal.
#[inline]
pub fn fold_char(c: char) -> char {
    if c.is_ascii() {
        return c.to_ascii_lowercase();
    }
    let mut lower = c.to_lowercase();
    let l = match (lower.next(), lower.next()) {
        (Some(l), None) => l,
        _ => return c,
    };
    match l {
        'µ' => 'μ',
        'ſ' => 's',
        'ς' => 'σ',
        '\u{345}' | '\u{1FBE}' => 'ι',
        'ϐ' => 'β',
        'ϑ' => 'θ',
        'ϕ' => 'φ',
        'ϖ' => 'π',
        'ϰ' => 'κ',
        'ϱ' => 'ρ',
        'ϵ' => 'ε',
        'ẛ' => 'ṡ',
        // Precomposed forms with the same fold (Unicode 14 and later).
        '\u{1FD3}' => '\u{390}',
        '\u{1FE3}' => '\u{3B0}',
        '\u{FB05}' => '\u{FB06}',
        '\u{1C80}' => 'в',
        '\u{1C81}' => 'д',
        '\u{1C82}' => 'о',
        '\u{1C83}' => 'с',
        '\u{1C84}' | '\u{1C85}' => 'т',
        '\u{1C86}' => 'ъ',
        '\u{1C87}' => 'ѣ',
        '\u{1C88}' => '\u{A64B}',
        // Cherokee folds to its capitals.
        '\u{AB70}'..='\u{ABBF}' => char::from_u32(l as u32 - 0xAB70 + 0x13A0).unwrap_or(l),
        '\u{13F8}'..='\u{13FD}' => char::from_u32(l as u32 - 8).unwrap_or(l),
        _ => l,
    }
}

/// `s` with every character folded ([`fold_char`]); borrowed when it is
/// already folded ASCII.
pub fn fold_str(s: &str) -> Cow<'_, str> {
    if s.is_ascii() {
        if s.bytes().any(|b| b.is_ascii_uppercase()) {
            Cow::Owned(s.to_ascii_lowercase())
        } else {
            Cow::Borrowed(s)
        }
    } else {
        Cow::Owned(s.chars().map(fold_char).collect())
    }
}

/// Whether comparing ASCII bytes case-insensitively finds every match of
/// the folded `needle` in `haystack`: the needle is ASCII, and it has no
/// `s` or `k` or the haystack has no `ſ` or Kelvin sign.
#[inline]
pub fn ascii_fold_suffices(haystack: &str, needle: &str) -> bool {
    needle.is_ascii()
        && (haystack.is_ascii()
            || !needle.bytes().any(|b| b == b's' || b == b'k')
            || (memchr::memmem::find(haystack.as_bytes(), "ſ".as_bytes()).is_none()
                && memchr::memmem::find(haystack.as_bytes(), "\u{212A}".as_bytes()).is_none()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex_syntax::hir::{ClassUnicode, ClassUnicodeRange};
    use std::collections::HashMap;

    /// fold_char agrees with regex-syntax's simple case folding (what
    /// ripgrep's -i uses) on every character: two characters fold alike
    /// exactly when they are in the same case-fold class.
    #[test]
    fn fold_char_matches_regex_simple_case_folding() {
        let chars = || (0u32..=0x10FFFF).filter_map(char::from_u32);
        let mut sizes: HashMap<char, usize> = HashMap::new();
        for c in chars() {
            *sizes.entry(fold_char(c)).or_default() += 1;
            // Folding folded text changes nothing (callers may fold twice).
            assert_eq!(fold_char(fold_char(c)), fold_char(c), "U+{:04X}", c as u32);
        }
        for c in chars() {
            let mut class = ClassUnicode::new([ClassUnicodeRange::new(c, c)]);
            class.case_fold_simple();
            let mut n = 0;
            for r in class.ranges() {
                for m in (r.start() as u32..=r.end() as u32).filter_map(char::from_u32) {
                    n += 1;
                    assert_eq!(
                        fold_char(m),
                        fold_char(c),
                        "{c:?} (U+{:04X}) and {m:?} (U+{:04X}) are case variants",
                        c as u32,
                        m as u32
                    );
                }
            }
            // A character with no case variants in regex-syntax's tables
            // may have some in std's newer Unicode (U+A7CE); skip those.
            if n == 1 {
                continue;
            }
            assert_eq!(
                n,
                sizes[&fold_char(c)],
                "{c:?} (U+{:04X}) folds like characters that are not its case variants",
                c as u32
            );
        }
    }

    #[test]
    fn only_long_s_and_kelvin_fold_into_ascii() {
        let into_ascii: Vec<char> = (0x80u32..=0x10FFFF)
            .filter_map(char::from_u32)
            .filter(|&c| fold_char(c).is_ascii())
            .collect();
        assert_eq!(into_ascii, vec!['ſ', '\u{212A}']);
    }

    #[test]
    fn fold_str_examples() {
        assert_eq!(fold_str("abc"), Cow::Borrowed("abc"));
        assert_eq!(fold_str("ABC"), "abc");
        assert_eq!(fold_str("ΟΣ ὁδός"), "οσ ὁδόσ");
        assert_eq!(fold_str("ſ\u{212A}µ"), "skμ");
        assert_eq!(fold_str("Straße İ"), "straße İ");
        assert!(ascii_fold_suffices("plain", "s"));
        assert!(ascii_fold_suffices("café", "s"));
        assert!(!ascii_fold_suffices("ſ", "s"));
        assert!(!ascii_fold_suffices("\u{212A}", "k"));
        assert!(ascii_fold_suffices("ſ", "abc"));
        assert!(!ascii_fold_suffices("abc", "é"));
    }
}
