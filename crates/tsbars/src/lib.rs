//! Handlebars template to TypeScript content mapper: infers a `Context` type from a template and
//! emits a virtual TypeScript module together with a source mapping back to the template.
//!
//! All offsets are UTF-8 byte offsets on char boundaries.

pub mod emit;
pub mod infer;
pub mod parser;

pub mod features {
    pub const HOVER: u32 = 1;
    pub const SIGNATURE_HELP: u32 = 2;
    pub const COMPLETION: u32 = 4;
    pub const DEFINITION: u32 = 8;
    pub const TYPE_DEFINITION: u32 = 16;
    pub const IMPLEMENTATION: u32 = 32;
    pub const REFERENCES: u32 = 64;
    pub const DOCUMENT_HIGHLIGHTS: u32 = 128;
    pub const RENAME: u32 = 256;
    pub const ALL: u32 = 1_048_575;
}

pub const DIAGNOSTIC_PARSE_ERROR: u32 = 1;
pub const DIAGNOSTIC_INVALID_DIRECTIVE: u32 = 2;

#[derive(Clone, Debug, Default)]
pub struct MapOptions {
    /// Names that are helpers even when written without params (`{{now}}`).
    pub helpers: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MappingKind {
    Verbatim = 0,
    Atom = 1,
    Alias = 2,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mapping {
    pub virtual_start: usize,
    pub virtual_len: usize,
    pub original_start: usize,
    pub original_len: usize,
    pub kind: MappingKind,
    pub features: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub message: String,
    pub start: usize,
    pub length: usize,
    pub code: u32,
}

#[derive(Clone, Debug)]
pub struct MapResult {
    pub text: String,
    pub mappings: Vec<Mapping>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn map(source: &str, options: &MapOptions) -> MapResult {
    match parser::parse(source) {
        Ok(nodes) => {
            let inferred = infer::infer(source, &nodes, options);
            let emitted = emit::emit(source, &inferred);
            MapResult {
                text: emitted.text,
                mappings: emitted.mappings,
                diagnostics: inferred.diagnostics,
            }
        }
        Err(error) => {
            let emitted = emit::emit_fallback(source);
            MapResult {
                text: emitted.text,
                mappings: emitted.mappings,
                diagnostics: vec![Diagnostic {
                    message: error.message,
                    start: error.span.start,
                    length: error.span.len(),
                    code: DIAGNOSTIC_PARSE_ERROR,
                }],
            }
        }
    }
}
