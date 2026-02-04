//! Synthetic corpus generator for whitebox validation testing
//!
//! Generates deterministic, multi-language test files with:
//! - Known "needle" strings at specific locations for verification
//! - Varied file sizes (small/medium/large)
//! - Varied complexity (flat modules, nested classes, deep nesting)
//! - Language-specific patterns (imports, classes, functions, async)

use rand::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Distribution of file sizes
#[derive(Debug, Clone, Copy)]
pub enum FileSize {
    Small,  // 10-50 lines
    Medium, // 50-200 lines
    Large,  // 200-500 lines
}

impl FileSize {
    fn line_range(self) -> (usize, usize) {
        match self {
            FileSize::Small => (10, 50),
            FileSize::Medium => (50, 200),
            FileSize::Large => (200, 500),
        }
    }

    fn from_rng(rng: &mut impl Rng) -> Self {
        let r: f32 = rng.random();
        if r < 0.4 {
            FileSize::Small
        } else if r < 0.8 {
            FileSize::Medium
        } else {
            FileSize::Large
        }
    }
}

/// Programming language for generated files
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    Rust,
    Python,
    TypeScript,
    JavaScript,
}

impl Language {
    fn extension(self) -> &'static str {
        match self {
            Language::Rust => "rs",
            Language::Python => "py",
            Language::TypeScript => "ts",
            Language::JavaScript => "js",
        }
    }

    fn from_rng(rng: &mut impl Rng) -> Self {
        let r: f32 = rng.random();
        if r < 0.40 {
            Language::Rust
        } else if r < 0.65 {
            Language::Python
        } else if r < 0.85 {
            Language::TypeScript
        } else {
            Language::JavaScript
        }
    }
}

/// A needle embedded in the corpus for verification
#[derive(Debug, Clone)]
pub struct Needle {
    pub marker: String,
    pub file_path: PathBuf,
    pub line_number: usize,
}

/// A symbol (function/class) generated in the corpus
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct GeneratedSymbol {
    pub name: String,
    pub symbol_type: String, // "function", "class", "method"
    pub file_path: PathBuf,
    pub line_number: usize,
}

/// Manifest of everything generated in the corpus
#[derive(Debug, Clone)]
pub struct CorpusManifest {
    pub files: Vec<PathBuf>,
    pub needles: Vec<Needle>,
    pub symbols: Vec<GeneratedSymbol>,
    pub total_lines: usize,
    pub files_by_language: HashMap<Language, usize>,
    pub files_by_size: HashMap<String, usize>,
}

impl CorpusManifest {
    fn new() -> Self {
        Self {
            files: Vec::new(),
            needles: Vec::new(),
            symbols: Vec::new(),
            total_lines: 0,
            files_by_language: HashMap::new(),
            files_by_size: HashMap::new(),
        }
    }
}

/// Corpus generator with seeded randomness
pub struct CorpusGenerator {
    rng: StdRng,
    file_id: usize,
}

impl CorpusGenerator {
    /// Create a new generator with the given seed
    pub fn new(seed: u64) -> Self {
        Self {
            rng: StdRng::seed_from_u64(seed),
            file_id: 0,
        }
    }

    /// Generate a complete corpus in the given directory
    pub fn generate(
        &mut self,
        output_dir: &Path,
        num_files: usize,
    ) -> std::io::Result<CorpusManifest> {
        let mut manifest = CorpusManifest::new();

        // Create subdirectories for organization
        let src_dir = output_dir.join("src");
        let lib_dir = output_dir.join("lib");
        let modules_dir = output_dir.join("modules");

        std::fs::create_dir_all(&src_dir)?;
        std::fs::create_dir_all(&lib_dir)?;
        std::fs::create_dir_all(&modules_dir)?;

        for _ in 0..num_files {
            let language = Language::from_rng(&mut self.rng);
            let size = FileSize::from_rng(&mut self.rng);

            // Choose directory
            let base_dir = match self.rng.random_range(0..3) {
                0 => &src_dir,
                1 => &lib_dir,
                _ => &modules_dir,
            };

            // Optionally create nested subdirectory
            let file_dir = if self.rng.random_bool(0.3) {
                let subdir = base_dir.join(format!("sub_{}", self.file_id / 10));
                std::fs::create_dir_all(&subdir)?;
                subdir
            } else {
                base_dir.clone()
            };

            let (content, file_needles, file_symbols, line_count) =
                self.generate_file(language, size, &file_dir);

            let file_name = format!("file_{}.{}", self.file_id, language.extension());
            let file_path = file_dir.join(&file_name);

            std::fs::write(&file_path, &content)?;

            // Update manifest
            manifest.files.push(file_path.clone());
            manifest.total_lines += line_count;

            for mut needle in file_needles {
                needle.file_path = file_path.clone();
                manifest.needles.push(needle);
            }

            for mut symbol in file_symbols {
                symbol.file_path = file_path.clone();
                manifest.symbols.push(symbol);
            }

            *manifest.files_by_language.entry(language).or_insert(0) += 1;
            *manifest
                .files_by_size
                .entry(format!("{:?}", size))
                .or_insert(0) += 1;

            self.file_id += 1;
        }

        Ok(manifest)
    }

    /// Generate a single file's content
    fn generate_file(
        &mut self,
        language: Language,
        size: FileSize,
        _file_dir: &Path,
    ) -> (String, Vec<Needle>, Vec<GeneratedSymbol>, usize) {
        let (min_lines, max_lines) = size.line_range();
        let target_lines = self.rng.random_range(min_lines..=max_lines);

        match language {
            Language::Rust => self.generate_rust_file(target_lines),
            Language::Python => self.generate_python_file(target_lines),
            Language::TypeScript => self.generate_typescript_file(target_lines),
            Language::JavaScript => self.generate_javascript_file(target_lines),
        }
    }

    fn generate_rust_file(
        &mut self,
        target_lines: usize,
    ) -> (String, Vec<Needle>, Vec<GeneratedSymbol>, usize) {
        let mut content = String::new();
        let mut needles = Vec::new();
        let mut symbols = Vec::new();
        let mut line = 1;

        // File header with imports
        content.push_str("//! Generated test file for validation\n");
        line += 1;
        content.push_str("use std::collections::HashMap;\n");
        line += 1;
        content.push_str("use std::sync::Arc;\n");
        line += 1;
        content.push('\n');
        line += 1;

        while line < target_lines {
            let remaining = target_lines - line;

            if remaining > 20 && self.rng.random_bool(0.3) {
                // Generate a struct with impl block
                let (struct_content, struct_lines, struct_needles, struct_symbols) =
                    self.generate_rust_struct(line);
                content.push_str(&struct_content);
                needles.extend(struct_needles);
                symbols.extend(struct_symbols);
                line += struct_lines;
            } else if remaining > 10 {
                // Generate a function
                let (fn_content, fn_lines, fn_needles, fn_symbols) =
                    self.generate_rust_function(line);
                content.push_str(&fn_content);
                needles.extend(fn_needles);
                symbols.extend(fn_symbols);
                line += fn_lines;
            } else {
                // Fill with comments
                content.push_str(&format!("// Line {} padding\n", line));
                line += 1;
            }
        }

        (content, needles, symbols, line)
    }

    fn generate_rust_struct(
        &mut self,
        start_line: usize,
    ) -> (String, usize, Vec<Needle>, Vec<GeneratedSymbol>) {
        let mut content = String::new();
        let mut needles = Vec::new();
        let mut symbols = Vec::new();
        let struct_name = format!("DataProcessor{}", self.file_id);
        let mut line = start_line;

        // Struct definition
        content.push_str(&format!("/// {} handles data operations\n", struct_name));
        line += 1;
        content.push_str(&format!("pub struct {} {{\n", struct_name));
        symbols.push(GeneratedSymbol {
            name: struct_name.clone(),
            symbol_type: "class".to_string(),
            file_path: PathBuf::new(),
            line_number: line,
        });
        line += 1;
        content.push_str("    data: Vec<u8>,\n");
        line += 1;
        content.push_str("    cache: HashMap<String, String>,\n");
        line += 1;

        // Insert needle in struct
        let needle_marker = format!("NEEDLE_{}_{}", self.file_id, line);
        content.push_str(&format!("    // {} - validation marker\n", needle_marker));
        needles.push(Needle {
            marker: needle_marker,
            file_path: PathBuf::new(),
            line_number: line,
        });
        line += 1;

        content.push_str("}\n\n");
        line += 2;

        // Impl block
        content.push_str(&format!("impl {} {{\n", struct_name));
        line += 1;

        // Constructor
        let method_name = "new";
        content.push_str(&format!("    pub fn {}() -> Self {{\n", method_name));
        symbols.push(GeneratedSymbol {
            name: method_name.to_string(),
            symbol_type: "function".to_string(),
            file_path: PathBuf::new(),
            line_number: line,
        });
        line += 1;
        content.push_str("        Self {\n");
        line += 1;
        content.push_str("            data: Vec::new(),\n");
        line += 1;
        content.push_str("            cache: HashMap::new(),\n");
        line += 1;
        content.push_str("        }\n");
        line += 1;
        content.push_str("    }\n\n");
        line += 2;

        // Process method
        let process_name = format!("process_data_{}", self.file_id);
        content.push_str(&format!(
            "    pub fn {}(&self, input: &str) -> String {{\n",
            process_name
        ));
        symbols.push(GeneratedSymbol {
            name: process_name,
            symbol_type: "function".to_string(),
            file_path: PathBuf::new(),
            line_number: line,
        });
        line += 1;
        content.push_str("        let mut result = String::new();\n");
        line += 1;
        content.push_str("        for line in input.lines() {\n");
        line += 1;
        content.push_str("            if line.contains(\"pattern\") {\n");
        line += 1;
        content.push_str("                result.push_str(line);\n");
        line += 1;
        content.push_str("            }\n");
        line += 1;
        content.push_str("        }\n");
        line += 1;
        content.push_str("        result\n");
        line += 1;
        content.push_str("    }\n");
        line += 1;
        content.push_str("}\n\n");
        line += 2;

        (content, line - start_line, needles, symbols)
    }

    fn generate_rust_function(
        &mut self,
        start_line: usize,
    ) -> (String, usize, Vec<Needle>, Vec<GeneratedSymbol>) {
        let mut content = String::new();
        let mut needles = Vec::new();
        let mut symbols = Vec::new();
        let fn_name = format!("handle_request_{}", self.file_id);
        let mut line = start_line;

        content.push_str("/// Handles request processing\n");
        line += 1;
        content.push_str(&format!(
            "pub async fn {}(input: &str) -> Result<String, Box<dyn std::error::Error>> {{\n",
            fn_name
        ));
        symbols.push(GeneratedSymbol {
            name: fn_name,
            symbol_type: "function".to_string(),
            file_path: PathBuf::new(),
            line_number: line,
        });
        line += 1;

        // Needle in function body
        let needle_marker = format!("NEEDLE_{}_{}", self.file_id, line);
        content.push_str(&format!("    // {} - validation marker\n", needle_marker));
        needles.push(Needle {
            marker: needle_marker,
            file_path: PathBuf::new(),
            line_number: line,
        });
        line += 1;

        content.push_str("    let processed = input.trim().to_uppercase();\n");
        line += 1;
        content.push_str("    Ok(processed)\n");
        line += 1;
        content.push_str("}\n\n");
        line += 2;

        (content, line - start_line, needles, symbols)
    }

    fn generate_python_file(
        &mut self,
        target_lines: usize,
    ) -> (String, Vec<Needle>, Vec<GeneratedSymbol>, usize) {
        let mut content = String::new();
        let mut needles = Vec::new();
        let mut symbols = Vec::new();
        let mut line = 1;

        // File header
        content.push_str("\"\"\"Generated test file for validation.\"\"\"\n");
        line += 1;
        content.push_str("import asyncio\n");
        line += 1;
        content.push_str("from typing import Dict, List, Optional\n");
        line += 1;
        content.push_str("from dataclasses import dataclass\n");
        line += 1;
        content.push('\n');
        line += 1;

        while line < target_lines {
            let remaining = target_lines - line;

            if remaining > 25 && self.rng.random_bool(0.4) {
                // Generate a class
                let (class_content, class_lines, class_needles, class_symbols) =
                    self.generate_python_class(line);
                content.push_str(&class_content);
                needles.extend(class_needles);
                symbols.extend(class_symbols);
                line += class_lines;
            } else if remaining > 8 {
                // Generate a function
                let (fn_content, fn_lines, fn_needles, fn_symbols) =
                    self.generate_python_function(line);
                content.push_str(&fn_content);
                needles.extend(fn_needles);
                symbols.extend(fn_symbols);
                line += fn_lines;
            } else {
                content.push_str(&format!("# Line {} padding\n", line));
                line += 1;
            }
        }

        (content, needles, symbols, line)
    }

    fn generate_python_class(
        &mut self,
        start_line: usize,
    ) -> (String, usize, Vec<Needle>, Vec<GeneratedSymbol>) {
        let mut content = String::new();
        let mut needles = Vec::new();
        let mut symbols = Vec::new();
        let class_name = format!("DataHandler{}", self.file_id);
        let mut line = start_line;

        content.push_str("@dataclass\n");
        line += 1;
        content.push_str(&format!("class {}:\n", class_name));
        symbols.push(GeneratedSymbol {
            name: class_name.clone(),
            symbol_type: "class".to_string(),
            file_path: PathBuf::new(),
            line_number: line,
        });
        line += 1;
        content.push_str(&format!(
            "    \"\"\"Handles data for file {}.\"\"\"\n",
            self.file_id
        ));
        line += 1;
        content.push_str("    \n");
        line += 1;
        content.push_str("    name: str\n");
        line += 1;
        content.push_str("    data: Dict[str, str]\n");
        line += 1;

        // Needle as comment
        let needle_marker = format!("NEEDLE_{}_{}", self.file_id, line);
        content.push_str(&format!("    # {} - validation marker\n", needle_marker));
        needles.push(Needle {
            marker: needle_marker,
            file_path: PathBuf::new(),
            line_number: line,
        });
        line += 1;

        content.push_str("    \n");
        line += 1;

        // Method
        let method_name = format!("process_item_{}", self.file_id);
        content.push_str(&format!(
            "    def {}(self, item: str) -> Optional[str]:\n",
            method_name
        ));
        symbols.push(GeneratedSymbol {
            name: method_name,
            symbol_type: "function".to_string(),
            file_path: PathBuf::new(),
            line_number: line,
        });
        line += 1;
        content.push_str("        \"\"\"Process a single item.\"\"\"\n");
        line += 1;
        content.push_str("        if item in self.data:\n");
        line += 1;
        content.push_str("            return self.data[item]\n");
        line += 1;
        content.push_str("        return None\n");
        line += 1;
        content.push('\n');
        line += 1;

        // Async method
        let async_name = format!("fetch_data_{}", self.file_id);
        content.push_str(&format!(
            "    async def {}(self, url: str) -> Dict:\n",
            async_name
        ));
        symbols.push(GeneratedSymbol {
            name: async_name,
            symbol_type: "function".to_string(),
            file_path: PathBuf::new(),
            line_number: line,
        });
        line += 1;
        content.push_str("        \"\"\"Fetch data asynchronously.\"\"\"\n");
        line += 1;
        content.push_str("        await asyncio.sleep(0.1)\n");
        line += 1;
        content.push_str("        return {\"status\": \"ok\"}\n");
        line += 1;
        content.push_str("\n\n");
        line += 2;

        (content, line - start_line, needles, symbols)
    }

    fn generate_python_function(
        &mut self,
        start_line: usize,
    ) -> (String, usize, Vec<Needle>, Vec<GeneratedSymbol>) {
        let mut content = String::new();
        let mut needles = Vec::new();
        let mut symbols = Vec::new();
        let fn_name = format!("authenticate_user_{}", self.file_id);
        let mut line = start_line;

        content.push_str(&format!(
            "def {}(username: str, password: str) -> bool:\n",
            fn_name
        ));
        symbols.push(GeneratedSymbol {
            name: fn_name,
            symbol_type: "function".to_string(),
            file_path: PathBuf::new(),
            line_number: line,
        });
        line += 1;
        content.push_str("    \"\"\"Authenticate a user with credentials.\"\"\"\n");
        line += 1;

        let needle_marker = format!("NEEDLE_{}_{}", self.file_id, line);
        content.push_str(&format!("    # {} - validation marker\n", needle_marker));
        needles.push(Needle {
            marker: needle_marker,
            file_path: PathBuf::new(),
            line_number: line,
        });
        line += 1;

        content.push_str("    if not username or not password:\n");
        line += 1;
        content.push_str("        return False\n");
        line += 1;
        content.push_str("    return len(password) >= 8\n");
        line += 1;
        content.push_str("\n\n");
        line += 2;

        (content, line - start_line, needles, symbols)
    }

    fn generate_typescript_file(
        &mut self,
        target_lines: usize,
    ) -> (String, Vec<Needle>, Vec<GeneratedSymbol>, usize) {
        let mut content = String::new();
        let mut needles = Vec::new();
        let mut symbols = Vec::new();
        let mut line = 1;

        // Imports
        content.push_str("// Generated test file for validation\n");
        line += 1;
        content.push_str("import { EventEmitter } from 'events';\n");
        line += 1;
        content.push_str("import type { Config, Result } from './types';\n");
        line += 1;
        content.push('\n');
        line += 1;

        while line < target_lines {
            let remaining = target_lines - line;

            if remaining > 30 && self.rng.random_bool(0.35) {
                let (class_content, class_lines, class_needles, class_symbols) =
                    self.generate_typescript_class(line);
                content.push_str(&class_content);
                needles.extend(class_needles);
                symbols.extend(class_symbols);
                line += class_lines;
            } else if remaining > 10 && self.rng.random_bool(0.5) {
                let (iface_content, iface_lines) = self.generate_typescript_interface(line);
                content.push_str(&iface_content);
                line += iface_lines;
            } else if remaining > 8 {
                let (fn_content, fn_lines, fn_needles, fn_symbols) =
                    self.generate_typescript_function(line);
                content.push_str(&fn_content);
                needles.extend(fn_needles);
                symbols.extend(fn_symbols);
                line += fn_lines;
            } else {
                content.push_str(&format!("// Line {} padding\n", line));
                line += 1;
            }
        }

        (content, needles, symbols, line)
    }

    fn generate_typescript_interface(&mut self, start_line: usize) -> (String, usize) {
        let mut content = String::new();
        let iface_name = format!("DataConfig{}", self.file_id);
        let mut line = start_line;

        content.push_str(&format!("export interface {} {{\n", iface_name));
        line += 1;
        content.push_str("    readonly id: string;\n");
        line += 1;
        content.push_str("    name: string;\n");
        line += 1;
        content.push_str("    options?: Record<string, unknown>;\n");
        line += 1;
        content.push_str("}\n\n");
        line += 2;

        (content, line - start_line)
    }

    fn generate_typescript_class(
        &mut self,
        start_line: usize,
    ) -> (String, usize, Vec<Needle>, Vec<GeneratedSymbol>) {
        let mut content = String::new();
        let mut needles = Vec::new();
        let mut symbols = Vec::new();
        let class_name = format!("ServiceHandler{}", self.file_id);
        let mut line = start_line;

        content.push_str(&format!(
            "export class {} extends EventEmitter {{\n",
            class_name
        ));
        symbols.push(GeneratedSymbol {
            name: class_name.clone(),
            symbol_type: "class".to_string(),
            file_path: PathBuf::new(),
            line_number: line,
        });
        line += 1;
        content.push_str("    private data: Map<string, unknown>;\n");
        line += 1;

        let needle_marker = format!("NEEDLE_{}_{}", self.file_id, line);
        content.push_str(&format!("    // {} - validation marker\n", needle_marker));
        needles.push(Needle {
            marker: needle_marker,
            file_path: PathBuf::new(),
            line_number: line,
        });
        line += 1;

        content.push('\n');
        line += 1;

        // Constructor
        content.push_str("    constructor(private readonly config: Config) {\n");
        line += 1;
        content.push_str("        super();\n");
        line += 1;
        content.push_str("        this.data = new Map();\n");
        line += 1;
        content.push_str("    }\n\n");
        line += 2;

        // Async method
        let method_name = format!("processRequest{}", self.file_id);
        content.push_str(&format!(
            "    async {}(request: unknown): Promise<Result> {{\n",
            method_name
        ));
        symbols.push(GeneratedSymbol {
            name: method_name,
            symbol_type: "function".to_string(),
            file_path: PathBuf::new(),
            line_number: line,
        });
        line += 1;
        content.push_str("        this.emit('request', request);\n");
        line += 1;
        content.push_str("        await new Promise(resolve => setTimeout(resolve, 10));\n");
        line += 1;
        content.push_str("        return { success: true };\n");
        line += 1;
        content.push_str("    }\n");
        line += 1;
        content.push_str("}\n\n");
        line += 2;

        (content, line - start_line, needles, symbols)
    }

    fn generate_typescript_function(
        &mut self,
        start_line: usize,
    ) -> (String, usize, Vec<Needle>, Vec<GeneratedSymbol>) {
        let mut content = String::new();
        let mut needles = Vec::new();
        let mut symbols = Vec::new();
        let fn_name = format!("validateInput{}", self.file_id);
        let mut line = start_line;

        content.push_str(&format!(
            "export async function {}(input: string): Promise<boolean> {{\n",
            fn_name
        ));
        symbols.push(GeneratedSymbol {
            name: fn_name,
            symbol_type: "function".to_string(),
            file_path: PathBuf::new(),
            line_number: line,
        });
        line += 1;

        let needle_marker = format!("NEEDLE_{}_{}", self.file_id, line);
        content.push_str(&format!("    // {} - validation marker\n", needle_marker));
        needles.push(Needle {
            marker: needle_marker,
            file_path: PathBuf::new(),
            line_number: line,
        });
        line += 1;

        content.push_str("    if (!input || input.trim().length === 0) {\n");
        line += 1;
        content.push_str("        return false;\n");
        line += 1;
        content.push_str("    }\n");
        line += 1;
        content.push_str("    return input.length > 3;\n");
        line += 1;
        content.push_str("}\n\n");
        line += 2;

        (content, line - start_line, needles, symbols)
    }

    fn generate_javascript_file(
        &mut self,
        target_lines: usize,
    ) -> (String, Vec<Needle>, Vec<GeneratedSymbol>, usize) {
        let mut content = String::new();
        let mut needles = Vec::new();
        let mut symbols = Vec::new();
        let mut line = 1;

        // Header
        content.push_str("// Generated test file for validation\n");
        line += 1;
        content.push_str("'use strict';\n");
        line += 1;
        content.push('\n');
        line += 1;
        content.push_str("const EventEmitter = require('events');\n");
        line += 1;
        content.push('\n');
        line += 1;

        while line < target_lines {
            let remaining = target_lines - line;

            if remaining > 25 && self.rng.random_bool(0.3) {
                let (class_content, class_lines, class_needles, class_symbols) =
                    self.generate_javascript_class(line);
                content.push_str(&class_content);
                needles.extend(class_needles);
                symbols.extend(class_symbols);
                line += class_lines;
            } else if remaining > 8 {
                let (fn_content, fn_lines, fn_needles, fn_symbols) =
                    self.generate_javascript_function(line);
                content.push_str(&fn_content);
                needles.extend(fn_needles);
                symbols.extend(fn_symbols);
                line += fn_lines;
            } else {
                content.push_str(&format!("// Line {} padding\n", line));
                line += 1;
            }
        }

        // Exports
        content.push_str("\nmodule.exports = {\n");
        line += 2;
        content.push_str("    // Export all functions\n");
        line += 1;
        content.push_str("};\n");
        line += 1;

        (content, needles, symbols, line)
    }

    fn generate_javascript_class(
        &mut self,
        start_line: usize,
    ) -> (String, usize, Vec<Needle>, Vec<GeneratedSymbol>) {
        let mut content = String::new();
        let mut needles = Vec::new();
        let mut symbols = Vec::new();
        let class_name = format!("DataManager{}", self.file_id);
        let mut line = start_line;

        content.push_str(&format!("class {} {{\n", class_name));
        symbols.push(GeneratedSymbol {
            name: class_name,
            symbol_type: "class".to_string(),
            file_path: PathBuf::new(),
            line_number: line,
        });
        line += 1;

        let needle_marker = format!("NEEDLE_{}_{}", self.file_id, line);
        content.push_str(&format!("    // {} - validation marker\n", needle_marker));
        needles.push(Needle {
            marker: needle_marker,
            file_path: PathBuf::new(),
            line_number: line,
        });
        line += 1;

        content.push('\n');
        line += 1;
        content.push_str("    constructor(options = {}) {\n");
        line += 1;
        content.push_str("        this.options = options;\n");
        line += 1;
        content.push_str("        this.cache = new Map();\n");
        line += 1;
        content.push_str("    }\n\n");
        line += 2;

        let method_name = format!("fetchData{}", self.file_id);
        content.push_str(&format!("    async {}(id) {{\n", method_name));
        symbols.push(GeneratedSymbol {
            name: method_name,
            symbol_type: "function".to_string(),
            file_path: PathBuf::new(),
            line_number: line,
        });
        line += 1;
        content.push_str("        if (this.cache.has(id)) {\n");
        line += 1;
        content.push_str("            return this.cache.get(id);\n");
        line += 1;
        content.push_str("        }\n");
        line += 1;
        content.push_str("        return null;\n");
        line += 1;
        content.push_str("    }\n");
        line += 1;
        content.push_str("}\n\n");
        line += 2;

        (content, line - start_line, needles, symbols)
    }

    fn generate_javascript_function(
        &mut self,
        start_line: usize,
    ) -> (String, usize, Vec<Needle>, Vec<GeneratedSymbol>) {
        let mut content = String::new();
        let mut needles = Vec::new();
        let mut symbols = Vec::new();
        let fn_name = format!("processData{}", self.file_id);
        let mut line = start_line;

        content.push_str(&format!("async function {}(data) {{\n", fn_name));
        symbols.push(GeneratedSymbol {
            name: fn_name,
            symbol_type: "function".to_string(),
            file_path: PathBuf::new(),
            line_number: line,
        });
        line += 1;

        let needle_marker = format!("NEEDLE_{}_{}", self.file_id, line);
        content.push_str(&format!("    // {} - validation marker\n", needle_marker));
        needles.push(Needle {
            marker: needle_marker,
            file_path: PathBuf::new(),
            line_number: line,
        });
        line += 1;

        content.push_str("    if (!data) {\n");
        line += 1;
        content.push_str("        throw new Error('Data required');\n");
        line += 1;
        content.push_str("    }\n");
        line += 1;
        content.push_str("    return { processed: true, data };\n");
        line += 1;
        content.push_str("}\n\n");
        line += 2;

        (content, line - start_line, needles, symbols)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_deterministic_generation() {
        let temp_dir1 = TempDir::new().unwrap();
        let temp_dir2 = TempDir::new().unwrap();

        let mut gen1 = CorpusGenerator::new(42);
        let mut gen2 = CorpusGenerator::new(42);

        let manifest1 = gen1.generate(temp_dir1.path(), 10).unwrap();
        let manifest2 = gen2.generate(temp_dir2.path(), 10).unwrap();

        // Same seed should produce same number of needles
        assert_eq!(manifest1.needles.len(), manifest2.needles.len());

        // And same markers (ignoring paths)
        for (n1, n2) in manifest1.needles.iter().zip(manifest2.needles.iter()) {
            assert_eq!(n1.marker, n2.marker);
            assert_eq!(n1.line_number, n2.line_number);
        }
    }

    #[test]
    fn test_language_distribution() {
        let temp_dir = TempDir::new().unwrap();
        let mut gen = CorpusGenerator::new(12345);

        let manifest = gen.generate(temp_dir.path(), 100).unwrap();

        // Should have files of all languages
        assert!(
            manifest
                .files_by_language
                .get(&Language::Rust)
                .unwrap_or(&0)
                > &0
        );
        assert!(
            manifest
                .files_by_language
                .get(&Language::Python)
                .unwrap_or(&0)
                > &0
        );
        assert!(
            manifest
                .files_by_language
                .get(&Language::TypeScript)
                .unwrap_or(&0)
                > &0
        );
        assert!(
            manifest
                .files_by_language
                .get(&Language::JavaScript)
                .unwrap_or(&0)
                > &0
        );
    }

    #[test]
    fn test_needles_present() {
        let temp_dir = TempDir::new().unwrap();
        let mut gen = CorpusGenerator::new(99);

        let manifest = gen.generate(temp_dir.path(), 20).unwrap();

        // Every file should have at least one needle
        assert!(!manifest.needles.is_empty());

        // Verify needles are in files
        for needle in &manifest.needles {
            let content = std::fs::read_to_string(&needle.file_path).unwrap();
            assert!(
                content.contains(&needle.marker),
                "Needle {} not found in {:?}",
                needle.marker,
                needle.file_path
            );
        }
    }
}
