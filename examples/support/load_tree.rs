//! The generated tree the load tools use (`watch_load`, `scoreboard`,
//! `chaos`): files, 1,000 to a folder, each rewritable with a new revision.
//!
//! It is shaped like the pinned real corpus (Linux v6.11, LLVM 19.1.0, Rust
//! 1.82.0, Kubernetes v1.31.0; 304,688 files, measured 2026-10-25):
//!
//! - sizes are log-normal with a median of 2 KB (log-mean 7.60, log-sd
//!   1.72, so a mean near 9 KB), capped at 2 MiB, so a few files are over
//!   1 MB;
//! - 55% of files are in a language fcs extracts symbols from (C, C++,
//!   Rust, Go, Python) and the rest are IR, assembly, YAML and text;
//! - the filler is tuned so that indexing costs, per megabyte, about what
//!   the real corpus costs. On 30,000 files, against the real corpus:
//!   symbols 0.097 MB per MB (real 0.097), references 0.033 (0.020),
//!   posting lists 0.175 (0.14), build 5.9 MB/s (7.9 MB/s).
//!
//! Every file starts with the same few lines (a function in its language,
//! or a comment block) so the load tools' queries ([`MIXED`]) find it.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::time::Instant;

/// Queries for a mix of searches: (regex, text). Common words make a search
/// read most of the tree; the rest are cheap.
pub const MIXED: &[(bool, &str)] = &[
    (false, "task_17_"),
    (false, "return value"),
    (true, r"default_\w+\(\)"),
    (false, "revision"),
    (true, r"task_\d+_amber"),
    (false, "helpers.default_cobalt"),
];

/// Names this generator's output. Change it whenever `script`, `path_of`
/// or `size_of` produce something different, so existing trees are
/// regenerated rather than mixed with the new output.
pub const GENERATOR: &str = "real-shaped-1";

pub const WORDS: &[&str] = &[
    "amber", "basalt", "cobalt", "dune", "ember", "fjord", "garnet", "harbor", "indigo", "jasper",
];

/// File `i` at revision `rev`.
pub fn script(i: usize, rev: u64) -> String {
    let w = WORDS[i % WORDS.len()];
    let v = WORDS[(i / 7 + rev as usize) % WORDS.len()];
    let mut text = match language(i) {
        "py" => format!(
            "import os\nfrom . import helpers\n\n\ndef task_{i}_{w}(ctx):\n    \"\"\"{w} {v} revision {rev}\"\"\"\n    value = ctx.get(\"{v}_{i}\")\n    if value is None:\n        return helpers.default_{w}()\n    return value * {rev}\n"
        ),
        "rs" => format!(
            "use crate::helpers;\n\n/// {w} {v} revision {rev}\npub fn task_{i}_{w}(ctx: &Ctx) -> i64 {{\n    let Some(value) = ctx.get(\"{v}_{i}\") else {{\n        return helpers.default_{w}();\n    }};\n    return value * {rev};\n}}\n"
        ),
        "go" => format!(
            "package pkg\n\n// task_{i}_{w}: {w} {v} revision {rev}\nfunc task_{i}_{w}(ctx *Ctx) int {{\n\tvalue, ok := ctx.Get(\"{v}_{i}\")\n\tif !ok {{\n\t\treturn helpers.default_{w}()\n\t}}\n\treturn value * {rev}\n}}\n"
        ),
        "ll" | "s" | "yaml" | "txt" => format!(
            "# task_{i}_{w}: {w} {v} revision {rev}\n# value = ctx.get(\"{v}_{i}\")\n# if no value, return helpers.default_{w}()\n# return value * {rev}\n"
        ),
        "cpp" => format!(
            "#include \"helpers.h\"\n\n// {w} {v} revision {rev}\nlong task_{i}_{w}(const Ctx &ctx) {{\n  auto value = ctx.get(\"{v}_{i}\");\n  if (!value)\n    return helpers.default_{w}();\n  return value * {rev};\n}}\n"
        ),
        _ => format!(
            "#include \"helpers.h\"\n\n/* {w} {v} revision {rev} */\nlong task_{i}_{w}(struct ctx *ctx)\n{{\n\tlong value = ctx_get(ctx, \"{v}_{i}\");\n\tif (!value)\n\t\treturn helpers.default_{w}();\n\treturn value * {rev};\n}}\n"
        ),
    };
    filler(&mut text, i, size_of(i));
    text
}

/// File `i`'s language, as its extension, in about the pinned real corpus's
/// shares: 55% of its files are in a language fcs extracts symbols from (C
/// and headers, C++, Rust, Go, Python), the rest are LLVM IR, assembly,
/// YAML and text, which are indexed but not parsed.
pub fn language(i: usize) -> &'static str {
    match mix(i as u64, 3) % 20 {
        0..=3 => "c",
        4..=5 => "cpp",
        6..=7 => "rs",
        8 => "go",
        9..=10 => "py",
        11..=13 => "ll",
        14..=15 => "s",
        16..=17 => "yaml",
        _ => "txt",
    }
}

/// File `i`'s size in bytes: log-normal like the real corpus, capped at
/// 2 MiB.
pub fn size_of(i: usize) -> usize {
    const LOG_MEAN: f64 = 7.60;
    const LOG_SD: f64 = 1.72;
    let unit = |salt| (mix(i as u64, salt) >> 11) as f64 / (1u64 << 53) as f64;
    let (a, b) = (unit(1).max(f64::MIN_POSITIVE), unit(2));
    let z = (-2.0 * a.ln()).sqrt() * (std::f64::consts::TAU * b).cos();
    (LOG_MEAN + LOG_SD * z).exp().min(2.0 * 1024.0 * 1024.0) as usize
}

/// A deterministic hash of `i` and `salt` (splitmix64).
fn mix(i: u64, salt: u64) -> u64 {
    let mut z = i
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(salt.wrapping_mul(0xBF58_476D_1CE4_E5B9));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Syllables for filler identifiers and comment words. One to four of them
/// give about 17 million words, so the filler has a spread of trigrams like
/// real code. None has a `t` or a `v`, so no filler can match a load
/// tool's query ([`MIXED`]) by chance.
const SYLLABLES: [&str; 64] = [
    "ba", "be", "bi", "bo", "ca", "ce", "ci", "co", "da", "de", "di", "do", "fa", "fe", "fi", "fo",
    "ga", "ge", "gi", "go", "ha", "he", "hi", "ho", "ka", "ke", "ki", "ko", "la", "le", "li", "lo",
    "ma", "me", "mi", "mo", "na", "ne", "ni", "no", "pa", "pe", "pi", "po", "ra", "re", "ri", "ro",
    "sa", "se", "si", "so", "wa", "we", "wi", "wo", "ya", "ye", "yi", "yo", "za", "ze", "zi", "zo",
];

/// A word from the bits of `h`.
fn word(h: u64) -> String {
    let mut s = String::new();
    for j in 0..1 + (h % 4) {
        s.push_str(SYLLABLES[((h >> (2 + 6 * j)) % 64) as usize]);
    }
    s
}

/// Pad `text` to about `size` bytes with filler in `i`'s language: comment
/// blocks and functions of 6 to 18 statement lines, with a call now and
/// then. The mix is tuned so that indexing it costs, per megabyte, about
/// what the pinned real corpus costs (symbols, references, posting-list
/// bytes and build time; the book's scoreboard page has the comparison).
fn filler(text: &mut String, i: usize, size: usize) {
    use std::fmt::Write;
    let lang = language(i);
    if matches!(lang, "ll" | "s" | "yaml" | "txt") {
        return data_filler(text, i, size, lang);
    }
    let py = lang == "py";
    let mut k = 0u64;
    let mut next = || {
        k += 1;
        mix(i as u64, 100 + k)
    };
    while text.len() < size {
        let h = next();
        if h % 4 == 0 {
            for _ in 0..1 + (h >> 8) % 3 {
                let h = next();
                let (a, b, c, d) = (word(h), word(h >> 12), word(h >> 24), word(h >> 36));
                let _ = if py {
                    writeln!(text, "# {a} {b} {c}, {d} {:x}.", h >> 52)
                } else {
                    writeln!(text, "// {a} {b} {c}, {d} {:x}.", h >> 52)
                };
            }
            continue;
        }
        let (f, g) = (word(h >> 4), word(h >> 20));
        let _ = match lang {
            "py" => writeln!(text, "def {f}_{g}(x, y):"),
            "rs" => writeln!(text, "pub fn {f}_{g}(mut x: i64, y: i64) -> i64 {{"),
            "go" => writeln!(text, "func {f}_{g}(x, y int) int {{"),
            _ => writeln!(text, "static int {f}_{g}(int x, int y)\n{{"),
        };
        let indent = if lang == "go" || lang == "c" {
            "\t"
        } else {
            "    "
        };
        for _ in 0..6 + (h >> 40) % 13 {
            let h = next();
            let (n, m) = (h % 4096, (h >> 12) % 64);
            let (a, b) = (word(h >> 20), word(h >> 32));
            let end = if py || lang == "go" { "" } else { ";" };
            let _ = match h >> 59 {
                0 => writeln!(text, "{indent}x = {a}_{b}(x, y){end}"),
                1..=11 => writeln!(text, "{indent}x = x * {n} + y % {m}{end}"),
                12..=19 if py => writeln!(text, "{indent}if x > {n}:\n{indent}    x -= {m}"),
                12..=19 if lang == "c" || lang == "cpp" => {
                    writeln!(text, "{indent}if (x > {n})\n{indent}{indent}x -= {m};")
                }
                12..=19 => writeln!(
                    text,
                    "{indent}if x > {n} {{\n{indent}{indent}x -= {m}{end}\n{indent}}}"
                ),
                _ if py => writeln!(text, "{indent}# {a} {b} {n:x}"),
                _ => writeln!(text, "{indent}// {a} {b} {n:x}"),
            };
        }
        let _ = match lang {
            "py" => writeln!(text, "    return x\n"),
            "rs" => writeln!(text, "    x\n}}\n"),
            "go" => writeln!(text, "\treturn x\n}}\n"),
            _ => writeln!(text, "\treturn x;\n}}\n"),
        };
    }
}

/// Filler for a file fcs indexes but does not parse: one line of IR,
/// assembly, YAML or prose at a time.
fn data_filler(text: &mut String, i: usize, size: usize, lang: &str) {
    use std::fmt::Write;
    let mut k = 0u64;
    while text.len() < size {
        k += 1;
        let h = mix(i as u64, 100 + k);
        let (a, b, c, d) = (word(h), word(h >> 12), word(h >> 24), word(h >> 36));
        let n = h >> 52;
        let _ = match lang {
            "ll" => writeln!(text, "  %{a}{n} = add nsw i32 %{b}, {n}  ; {c}"),
            "s" => writeln!(text, "\tmovq\t${n}, %r{a}\t# {b} {c}"),
            "yaml" => writeln!(text, "{a}_{b}:\n  {c}: {n}\n  {d}: \"{a} {c}\""),
            _ => writeln!(text, "{a} {b} {c} {d}, {n}."),
        };
    }
}

pub fn path_of(root: &Path, i: usize) -> PathBuf {
    root.join(format!("pkg{:04}", i / 1000))
        .join(format!("job_{i}.{}", language(i)))
}

/// Write `files` scripts under `root` at revision 0, unless it already
/// holds exactly that tree.
pub fn generate(root: &Path, files: usize) {
    // The marker names the generator, so a tree from an older one is
    // regenerated (rewriting it would leave the old files beside the new).
    let marker = root.join(format!(".generated-{files}-{GENERATOR}"));
    if marker.exists() {
        eprintln!("reusing {} ({files} files)", root.display());
        return;
    }
    let _ = std::fs::remove_dir_all(root);
    let t = Instant::now();
    use rayon::prelude::*;
    (0..files.div_ceil(1000)).into_par_iter().for_each(|d| {
        let dir = root.join(format!("pkg{d:04}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("helpers.py"),
            "def default_amber():\n    return 0\n",
        )
        .unwrap();
        for i in d * 1000..((d + 1) * 1000).min(files) {
            std::fs::write(path_of(root, i), script(i, 0)).unwrap();
        }
    });
    std::fs::write(marker, "").unwrap();
    eprintln!("generated {files} files in {:.1?}", t.elapsed());
}
