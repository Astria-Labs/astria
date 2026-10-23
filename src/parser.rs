use tree_sitter::Parser;
use std::collections::BTreeMap;

/// Represents a code chunk extracted via AST analysis (used for Vector Indexing)
#[derive(Debug, Clone)]
pub struct CodeChunk {
    pub file: String,
    pub kind: String,       // e.g., "function_item", "struct_item"
    pub name: String,       // Identifier/Name of the function or struct
    pub start_line: usize,
    pub end_line: usize,
    #[allow(dead_code)]
    pub content: String,    // Raw code slice corresponding to the chunk
}

/// Represents structural info of a symbol for AST Diffing
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolInfo {
    pub name: String,
    pub symbol_type: String, // "function", "struct", "enum", "trait"
    pub content_hash: String,
    pub is_public: bool,
    pub line_number: usize,
}

pub struct SemanticParser {
    parser: Parser,
}

impl SemanticParser {
    pub fn new() -> Self {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .expect("Error loading Rust grammar for tree-sitter");
        Self { parser }
    }

    /// Parses Rust code into CodeChunks for vector embeddings
    pub fn parse_rust_code(&mut self, code: &str, file_name: &str) -> Vec<CodeChunk> {
        let mut chunks = Vec::new();
        let tree = match self.parser.parse(code, None) {
            Some(t) => t,
            None => return chunks,
        };

        let root_node = tree.root_node();
        let bytes = code.as_bytes();

        for i in 0..root_node.child_count() {
            if let Some(child) = root_node.child(i) {
                let kind = child.kind();
                if kind == "function_item" || kind == "struct_item" || kind == "enum_item" || kind == "trait_item" {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        if let Ok(name) = name_node.utf8_text(bytes) {
                            let start_line = child.start_position().row + 1;
                            let end_line = child.end_position().row + 1;
                            if let Ok(content) = child.utf8_text(bytes) {
                                chunks.push(CodeChunk {
                                    file: file_name.to_string(),
                                    kind: kind.to_string(),
                                    name: name.to_string(),
                                    start_line,
                                    end_line,
                                    content: content.to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }

        chunks
    }

    /// Extracts structural symbols and content hashes for AST Diffs
    pub fn extract_symbols(&mut self, code: &str) -> BTreeMap<String, SymbolInfo> {
        let mut symbols = BTreeMap::new();
        let tree = match self.parser.parse(code, None) {
            Some(t) => t,
            None => return symbols,
        };

        let root_node = tree.root_node();
        let bytes = code.as_bytes();

        for i in 0..root_node.child_count() {
            if let Some(child) = root_node.child(i) {
                let kind = child.kind();
                if kind == "function_item" || kind == "struct_item" || kind == "enum_item" || kind == "trait_item" {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        if let Ok(name) = name_node.utf8_text(bytes) {
                            let symbol_type = kind.replace("_item", "");

                            let body_node = child.child_by_field_name("body").unwrap_or(child);
                            let body_text = body_node.utf8_text(bytes).unwrap_or("");
                            let content_hash = blake3::hash(body_text.as_bytes()).to_string();

                            let key = format!("{} {}", symbol_type, name);
                            symbols.insert(
                                key.clone(),
                                SymbolInfo {
                                    name: name.to_string(),
                                    symbol_type,
                                    content_hash,
                                    is_public: true,
                                    line_number: child.start_position().row + 1
                                },
                            );
                        }
                    }
                }
            }
        }

        symbols
    }
}