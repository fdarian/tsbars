//! Turns an inferred shape into virtual TypeScript text plus mappings back to the template.

use crate::features;
use crate::infer::{Inferred, Kind, NodeId, Shape};
use crate::{Mapping, MappingKind};

pub struct Emitted {
    pub text: String,
    pub mappings: Vec<Mapping>,
}

const PRINTABLE_DECLARATION: &str = "type Printable = string | number | bigint | boolean;\n";
const EXPORT_DEFAULT: &str = "export default template";
const KEY_FEATURES: u32 = features::HOVER | features::DEFINITION | features::REFERENCES;
const EXPORT_FEATURES: u32 = features::DEFINITION | features::TYPE_DEFINITION;

pub fn emit(source: &str, inferred: &Inferred) -> Emitted {
    let mut body = Emitter {
        source,
        shape: &inferred.shape,
        text: String::new(),
        mappings: Vec::new(),
        used_printable: false,
    };
    body.emit_body();

    let mut text = String::new();
    let mut mappings = Vec::new();
    for import in &inferred.imports {
        text.push_str("import ");
        mappings.push(Mapping {
            virtual_start: text.len(),
            virtual_len: import.len(),
            original_start: import.start,
            original_len: import.len(),
            kind: MappingKind::Verbatim,
            features: features::ALL,
        });
        let clause = import.text(source);
        text.push_str(clause);
        if !clause.ends_with(';') {
            text.push(';');
        }
        text.push('\n');
    }
    if body.used_printable {
        text.push_str(PRINTABLE_DECLARATION);
    }
    let offset = text.len();
    text.push_str(&body.text);
    mappings.extend(body.mappings.into_iter().map(|mut mapping| {
        mapping.virtual_start += offset;
        mapping
    }));
    Emitted { text, mappings }
}

/// Module for templates that failed to parse: accepts any context so importers don't cascade.
pub fn emit_fallback(source: &str) -> Emitted {
    let mut emitter = Emitter {
        source,
        shape: &EMPTY_SHAPE,
        text: String::new(),
        mappings: Vec::new(),
        used_printable: false,
    };
    emitter.text.push_str("export type Context = any;\n");
    emitter.emit_template_declaration("context: Context");
    Emitted {
        text: emitter.text,
        mappings: emitter.mappings,
    }
}

static EMPTY_SHAPE: Shape = Shape {
    nodes: Vec::new(),
    root: 0,
};

struct Emitter<'a> {
    source: &'a str,
    shape: &'a Shape,
    text: String,
    mappings: Vec<Mapping>,
    used_printable: bool,
}

fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_' || first == '$')
        && chars.all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '$'
        })
}

fn escape_string(name: &str) -> String {
    let mut escaped = String::with_capacity(name.len());
    for character in name.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            control if control.is_control() => {
                escaped.push_str(&format!("\\u{:04x}", u32::from(control)));
            }
            other => escaped.push(other),
        }
    }
    escaped
}

impl Emitter<'_> {
    fn emit_body(&mut self) {
        let root = &self.shape.nodes[self.shape.root];
        let is_empty = matches!(root.kind, Kind::Leaf) && root.props.is_empty() && !root.printed;
        self.text.push_str("export type Context = ");
        if is_empty {
            self.text.push_str("Record<string, never>");
        } else {
            self.emit_type(self.shape.root, 0);
        }
        self.text.push_str(";\n");
        self.emit_template_declaration(if is_empty {
            "context?: Context"
        } else {
            "context: Context"
        });
    }

    fn emit_template_declaration(&mut self, parameter: &str) {
        self.text.push_str(&format!(
            "declare const template: ({parameter}) => string;\n"
        ));
        let start = self.text.len();
        self.text.push_str(EXPORT_DEFAULT);
        if !self.source.is_empty() {
            self.mappings.push(Mapping {
                virtual_start: start,
                virtual_len: EXPORT_DEFAULT.len(),
                original_start: 0,
                original_len: self.source.len(),
                kind: MappingKind::Atom,
                features: EXPORT_FEATURES,
            });
        }
        self.text.push_str(";\n");
    }

    fn indent(&mut self, depth: usize) {
        for _ in 0..depth {
            self.text.push_str("  ");
        }
    }

    fn emit_type(&mut self, id: NodeId, depth: usize) {
        let shape = self.shape;
        let node = &shape.nodes[id];
        match &node.kind {
            Kind::Override(span) => {
                let original = span.text(self.source);
                self.mappings.push(Mapping {
                    virtual_start: self.text.len(),
                    virtual_len: original.len(),
                    original_start: span.start,
                    original_len: span.len(),
                    kind: MappingKind::Verbatim,
                    features: features::ALL,
                });
                self.text.push_str(original);
            }
            Kind::Collection { elem, keyed } => {
                self.text.push_str(if *keyed {
                    "Readonly<Record<string, "
                } else {
                    "ReadonlyArray<"
                });
                self.emit_type(*elem, depth);
                self.text.push_str(if *keyed { ">>" } else { ">" });
            }
            Kind::Leaf if !node.props.is_empty() => {
                self.text.push_str("{\n");
                for prop in &node.props {
                    self.indent(depth + 1);
                    self.emit_key(&prop.name, prop.key_span);
                    if shape.nodes[prop.node].optional {
                        self.text.push('?');
                    }
                    self.text.push_str(": ");
                    self.emit_type(prop.node, depth + 1);
                    self.text.push_str(";\n");
                }
                self.indent(depth);
                self.text.push('}');
            }
            Kind::Leaf if node.printed => {
                self.used_printable = true;
                self.text.push_str("Printable");
            }
            Kind::Leaf => self.text.push_str("unknown"),
        }
    }

    fn emit_key(&mut self, name: &str, key_span: crate::parser::Span) {
        let start;
        if is_identifier(name) {
            start = self.text.len();
            self.text.push_str(name);
        } else {
            self.text.push('"');
            start = self.text.len();
            self.text.push_str(&escape_string(name));
        }
        let key_text = self.text[start..].to_string();
        if !is_identifier(name) {
            self.text.push('"');
        }
        let kind = if key_text == key_span.text(self.source) {
            MappingKind::Verbatim
        } else {
            MappingKind::Atom
        };
        self.mappings.push(Mapping {
            virtual_start: start,
            virtual_len: key_text.len(),
            original_start: key_span.start,
            original_len: key_span.len(),
            kind,
            features: KEY_FEATURES,
        });
    }
}
