//! The generated tree the load tools use (`watch_load`, `scoreboard`): small
//! Python scripts, 1,000 to a folder, each rewritable with a new revision.

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

pub const WORDS: &[&str] = &[
    "amber", "basalt", "cobalt", "dune", "ember", "fjord", "garnet", "harbor", "indigo", "jasper",
];

/// Script `i` at revision `rev`.
pub fn script(i: usize, rev: u64) -> String {
    let w = WORDS[i % WORDS.len()];
    let v = WORDS[(i / 7 + rev as usize) % WORDS.len()];
    format!(
        "import os\nfrom . import helpers\n\n\ndef task_{i}_{w}(ctx):\n    \"\"\"{w} {v} revision {rev}\"\"\"\n    value = ctx.get(\"{v}_{i}\")\n    if value is None:\n        return helpers.default_{w}()\n    return value * {rev}\n"
    )
}

pub fn path_of(root: &Path, i: usize) -> PathBuf {
    root.join(format!("pkg{:04}", i / 1000))
        .join(format!("job_{i}.py"))
}

/// Write `files` scripts under `root` at revision 0, unless it already
/// holds exactly that tree.
pub fn generate(root: &Path, files: usize) {
    let marker = root.join(format!(".generated-{files}"));
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
