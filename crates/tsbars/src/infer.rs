//! Walks the template AST and infers the shape of the context object.

use std::collections::HashSet;

use crate::parser::{Block, Call, Comment, Expr, Node, Path, Segment, Span};
use crate::{DIAGNOSTIC_INVALID_DIRECTIVE, Diagnostic, MapOptions};

pub type NodeId = usize;

#[derive(Debug)]
pub struct Shape {
    pub nodes: Vec<ShapeNode>,
    pub root: NodeId,
}

#[derive(Debug)]
pub struct ShapeNode {
    pub kind: Kind,
    /// Used as `{{value}}`. Ignored when the node has props.
    pub printed: bool,
    /// Used in an `if`/`unless` guard, or in a `with` that has an `else`.
    pub optional: bool,
    /// In first-seen order.
    pub props: Vec<Prop>,
}

#[derive(Debug)]
pub struct Prop {
    pub name: String,
    /// First place the property was referenced in the template.
    pub key_span: Span,
    pub node: NodeId,
}

#[derive(Debug)]
pub enum Kind {
    /// Shape is determined by `props` and `printed`.
    Leaf,
    /// `{{#each}}` target. `keyed` (`@key` used in the body) makes it a record instead of an array.
    Collection { elem: NodeId, keyed: bool },
    /// Replaced by a `@param` comment; the span is the comment's type text.
    Override(Span),
}

#[derive(Debug)]
pub struct Inferred {
    pub shape: Shape,
    /// Text spans of `@import` comments, after the `@import` keyword.
    pub imports: Vec<Span>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn infer(source: &str, nodes: &[Node], options: &MapOptions) -> Inferred {
    let mut inferrer = Inferrer {
        source,
        helpers: options.helpers.iter().map(String::as_str).collect(),
        shape: Shape {
            nodes: Vec::new(),
            root: 0,
        },
        scopes: Vec::new(),
        bindings: Vec::new(),
        each_frames: Vec::new(),
    };
    let root = inferrer.new_node();
    inferrer.shape.root = root;
    inferrer.scopes.push(Scope::Node(root));
    inferrer.walk(nodes);

    let mut imports = Vec::new();
    let mut diagnostics = Vec::new();
    for node in nodes {
        let Node::Comment(comment) = node else {
            continue;
        };
        match inferrer.parse_directive(comment) {
            None => {}
            Some(Err(diagnostic)) => diagnostics.push(diagnostic),
            Some(Ok(Directive::Import(span))) => imports.push(span),
            Some(Ok(Directive::Param {
                path,
                type_span,
                directive_span,
            })) => {
                if let Err(diagnostic) = inferrer.apply_param(&path, type_span, directive_span) {
                    diagnostics.push(diagnostic);
                }
            }
        }
    }

    Inferred {
        shape: inferrer.shape,
        imports,
        diagnostics,
    }
}

#[derive(Clone, Copy)]
enum Scope {
    Node(NodeId),
    /// Anything resolved here records no constraints.
    Unknown,
}

enum Directive {
    Import(Span),
    Param {
        path: Vec<Segment>,
        type_span: Span,
        directive_span: Span,
    },
}

struct Inferrer<'a> {
    source: &'a str,
    helpers: HashSet<&'a str>,
    shape: Shape,
    scopes: Vec<Scope>,
    /// Block params (`as |item idx|`), innermost last.
    bindings: Vec<(String, Scope)>,
    /// Enclosing `each` bodies, innermost last. `None` when the target is untracked.
    each_frames: Vec<Option<NodeId>>,
}

/// Name of a plain identifier path such as `if` (not `a.b`, `../a`, `this.a` or `@a`).
fn simple_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Path(path)
            if !path.data && path.depth == 0 && !path.this && path.segments.len() == 1 =>
        {
            Some(path.segments[0].name.as_str())
        }
        _ => None,
    }
}

impl<'a> Inferrer<'a> {
    fn new_node(&mut self) -> NodeId {
        self.shape.nodes.push(ShapeNode {
            kind: Kind::Leaf,
            printed: false,
            optional: false,
            props: Vec::new(),
        });
        self.shape.nodes.len() - 1
    }

    fn current_scope(&self) -> Scope {
        self.scopes[self.scopes.len() - 1]
    }

    fn walk(&mut self, nodes: &[Node]) {
        for node in nodes {
            match node {
                Node::Mustache(mustache) => self.visit_mustache(&mustache.call),
                Node::Block(block) => self.visit_block(block),
                Node::Comment(_) | Node::Partial(_) | Node::Raw(_) => {}
            }
        }
    }

    fn visit_mustache(&mut self, call: &Call) {
        if !call.params.is_empty() || !call.hash.is_empty() {
            self.visit_helper_call(call);
            return;
        }
        match &call.head {
            Expr::Path(path) => {
                let is_helper =
                    simple_name(&call.head).is_some_and(|name| self.helpers.contains(name));
                if is_helper {
                    return;
                }
                self.mark_printed(path);
            }
            Expr::Sub(sub) => self.visit_helper_call(&sub.call),
            Expr::Literal(_) => {}
        }
    }

    fn mark_printed(&mut self, path: &Path) {
        if let Some(id) = self.resolve(path) {
            self.shape.nodes[id].printed = true;
        }
    }

    fn visit_helper_call(&mut self, call: &Call) {
        if simple_name(&call.head) == Some("lookup") {
            for (index, param) in call.params.iter().enumerate() {
                if index == 1 {
                    self.visit_printed_expr(param);
                } else {
                    self.reference(param);
                }
            }
        } else {
            for param in &call.params {
                self.reference(param);
            }
        }
        for pair in &call.hash {
            self.reference(&pair.value);
        }
    }

    fn reference(&mut self, expr: &Expr) {
        match expr {
            Expr::Path(path) => {
                self.resolve(path);
            }
            Expr::Sub(sub) => self.visit_helper_call(&sub.call),
            Expr::Literal(_) => {}
        }
    }

    fn visit_printed_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Path(path) => self.mark_printed(path),
            Expr::Sub(sub) => self.visit_helper_call(&sub.call),
            Expr::Literal(_) => {}
        }
    }

    fn guard(&mut self, expr: &Expr) {
        match expr {
            Expr::Path(path) => {
                if let Some(id) = self.resolve(path)
                    && !path.segments.is_empty()
                {
                    self.shape.nodes[id].optional = true;
                }
            }
            Expr::Sub(sub) => self.visit_helper_call(&sub.call),
            Expr::Literal(_) => {}
        }
    }

    fn visit_block(&mut self, block: &Block) {
        match simple_name(&block.call.head) {
            Some("if" | "unless") => {
                if let Some(condition) = block.call.params.first() {
                    self.guard(condition);
                }
                for param in block.call.params.iter().skip(1) {
                    self.reference(param);
                }
                self.walk(&block.body);
                if let Some(inverse) = &block.inverse {
                    self.walk(inverse);
                }
            }
            Some("each") => self.visit_each(block),
            Some("with") => self.visit_with(block),
            name => {
                let is_helper = name.is_some_and(|name| self.helpers.contains(name));
                let is_bare = block.call.params.is_empty() && block.call.hash.is_empty();
                if is_bare && !is_helper && matches!(block.call.head, Expr::Path(_)) {
                    self.guard(&block.call.head);
                } else {
                    self.visit_helper_call(&block.call);
                }
                self.visit_unknown_block(block);
            }
        }
    }

    /// Body of a non-builtin block: block params and context are untracked, the inverse is not.
    fn visit_unknown_block(&mut self, block: &Block) {
        let bindings_mark = self.bindings.len();
        for name in &block.block_params {
            self.bindings.push((name.clone(), Scope::Unknown));
        }
        self.scopes.push(Scope::Unknown);
        self.walk(&block.body);
        self.scopes.pop();
        self.bindings.truncate(bindings_mark);
        if let Some(inverse) = &block.inverse {
            self.walk(inverse);
        }
    }

    fn visit_each(&mut self, block: &Block) {
        let mut collection = None;
        let mut elem_scope = Scope::Unknown;
        match block.call.params.first() {
            Some(Expr::Path(path)) => {
                if let Some(id) = self.resolve(path)
                    && let Some(elem) = self.make_collection(id)
                {
                    collection = Some(id);
                    elem_scope = Scope::Node(elem);
                }
            }
            Some(Expr::Sub(sub)) => self.visit_helper_call(&sub.call),
            Some(Expr::Literal(_)) | None => {}
        }
        for param in block.call.params.iter().skip(1) {
            self.reference(param);
        }
        for pair in &block.call.hash {
            self.reference(&pair.value);
        }

        let bindings_mark = self.bindings.len();
        for (index, name) in block.block_params.iter().enumerate() {
            let scope = if index == 0 {
                elem_scope
            } else {
                Scope::Unknown
            };
            self.bindings.push((name.clone(), scope));
        }
        self.scopes.push(elem_scope);
        self.each_frames.push(collection);
        self.walk(&block.body);
        self.each_frames.pop();
        self.scopes.pop();
        self.bindings.truncate(bindings_mark);
        if let Some(inverse) = &block.inverse {
            self.walk(inverse);
        }
    }

    fn visit_with(&mut self, block: &Block) {
        let mut scope = Scope::Unknown;
        match block.call.params.first() {
            Some(Expr::Path(path)) => {
                if let Some(id) = self.resolve(path) {
                    if block.inverse.is_some() && !path.segments.is_empty() {
                        self.shape.nodes[id].optional = true;
                    }
                    scope = Scope::Node(id);
                }
            }
            Some(Expr::Sub(sub)) => self.visit_helper_call(&sub.call),
            Some(Expr::Literal(_)) | None => {}
        }
        for param in block.call.params.iter().skip(1) {
            self.reference(param);
        }

        let bindings_mark = self.bindings.len();
        for (index, name) in block.block_params.iter().enumerate() {
            let bound = if index == 0 { scope } else { Scope::Unknown };
            self.bindings.push((name.clone(), bound));
        }
        self.scopes.push(scope);
        self.walk(&block.body);
        self.scopes.pop();
        self.bindings.truncate(bindings_mark);
        if let Some(inverse) = &block.inverse {
            self.walk(inverse);
        }
    }

    /// Makes `id` an each-target. A collection wins over property access regardless of order.
    fn make_collection(&mut self, id: NodeId) -> Option<NodeId> {
        match self.shape.nodes[id].kind {
            Kind::Collection { elem, .. } => Some(elem),
            Kind::Override(_) => None,
            Kind::Leaf => {
                let elem = self.new_node();
                let node = &mut self.shape.nodes[id];
                node.props.clear();
                node.kind = Kind::Collection { elem, keyed: false };
                Some(elem)
            }
        }
    }

    fn mark_keyed(&mut self) {
        let Some(Some(collection)) = self.each_frames.last().copied() else {
            return;
        };
        if let Kind::Collection { keyed, .. } = &mut self.shape.nodes[collection].kind {
            *keyed = true;
        }
    }

    fn lookup_binding(&self, path: &Path) -> Option<Scope> {
        if path.data || path.depth > 0 || path.this {
            return None;
        }
        let first = path.segments.first()?;
        self.bindings
            .iter()
            .rev()
            .find(|(name, _)| *name == first.name)
            .map(|(_, scope)| *scope)
    }

    /// Resolves `path` against the scope stack, creating properties on the way. `None` means the
    /// target is not tracked (data variable, unknown scope, or inside a collection/override).
    fn resolve(&mut self, path: &Path) -> Option<NodeId> {
        let mut segments: &[Segment] = &path.segments;
        let start = if path.data {
            if path.depth > 0 {
                return None;
            }
            match segments.first()?.name.as_str() {
                "root" => {
                    segments = &segments[1..];
                    self.scopes[0]
                }
                "key" => {
                    self.mark_keyed();
                    return None;
                }
                _ => return None,
            }
        } else if path.depth > 0 {
            let index = self.scopes.len().checked_sub(1 + path.depth)?;
            self.scopes[index]
        } else if let Some(binding) = self.lookup_binding(path) {
            segments = &segments[1..];
            binding
        } else {
            self.current_scope()
        };

        let Scope::Node(mut node) = start else {
            return None;
        };
        for segment in segments {
            node = self.child(node, segment)?;
        }
        Some(node)
    }

    fn child(&mut self, parent: NodeId, segment: &Segment) -> Option<NodeId> {
        if !matches!(self.shape.nodes[parent].kind, Kind::Leaf) {
            return None;
        }
        if let Some(prop) = self.shape.nodes[parent]
            .props
            .iter()
            .find(|prop| prop.name == segment.name)
        {
            return Some(prop.node);
        }
        let node = self.new_node();
        self.shape.nodes[parent].props.push(Prop {
            name: segment.name.clone(),
            key_span: segment.span,
            node,
        });
        Some(node)
    }

    fn parse_directive(&self, comment: &Comment) -> Option<Result<Directive, Diagnostic>> {
        let text = comment.text.text(self.source);
        let trimmed = text.trim();
        let start = comment.text.start + (text.len() - text.trim_start().len());
        let directive_span = Span::new(start, start + trimmed.len());
        let invalid = |message: &str| {
            Err(Diagnostic {
                message: message.to_string(),
                start: directive_span.start,
                length: directive_span.len(),
                code: DIAGNOSTIC_INVALID_DIRECTIVE,
            })
        };

        let (keyword, after) = match trimmed.split_once(char::is_whitespace) {
            Some((keyword, _)) => (keyword, &trimmed[keyword.len()..]),
            None => (trimmed, ""),
        };
        let after_start = start + keyword.len();
        match keyword {
            "@import" => {
                let value = after.trim();
                if value.is_empty() {
                    return Some(invalid("Expected `@import <import clause>`"));
                }
                let value_start = after_start + (after.len() - after.trim_start().len());
                Some(Ok(Directive::Import(Span::new(
                    value_start,
                    value_start + value.len(),
                ))))
            }
            "@param" => {
                let after_ws = after.trim_start();
                let path_start = after_start + (after.len() - after_ws.len());
                let path_text = after_ws.split(char::is_whitespace).next().unwrap_or("");
                let type_part = &after_ws[path_text.len()..];
                let type_text = type_part.trim();
                if type_text.is_empty() {
                    return Some(invalid("Expected `@param <path> <type>`"));
                }
                let type_start =
                    path_start + path_text.len() + (type_part.len() - type_part.trim_start().len());
                let mut segments = Vec::new();
                let mut offset = path_start;
                for part in path_text.split('.') {
                    if part.is_empty() {
                        return Some(invalid("Invalid path in `@param`"));
                    }
                    segments.push(Segment {
                        name: part.to_string(),
                        span: Span::new(offset, offset + part.len()),
                    });
                    offset += part.len() + 1;
                }
                Some(Ok(Directive::Param {
                    path: segments,
                    type_span: Span::new(type_start, type_start + type_text.len()),
                    directive_span,
                }))
            }
            _ => None,
        }
    }

    fn apply_param(
        &mut self,
        path: &[Segment],
        type_span: Span,
        directive_span: Span,
    ) -> Result<(), Diagnostic> {
        let mut node = self.shape.root;
        for segment in path {
            node = self.child(node, segment).ok_or_else(|| Diagnostic {
                message: format!(
                    "Cannot apply `@param`: `{}` is inside a value that is already typed as an array or overridden",
                    segment.name
                ),
                start: directive_span.start,
                length: directive_span.len(),
                code: DIAGNOSTIC_INVALID_DIRECTIVE,
            })?;
        }
        let target = &mut self.shape.nodes[node];
        target.props.clear();
        target.kind = Kind::Override(type_span);
        Ok(())
    }
}
