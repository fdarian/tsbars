//! Hand-written Handlebars parser. Every offset is a UTF-8 byte offset into the
//! source, and every span starts and ends on a char boundary.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Span {
        Span { start, end }
    }

    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.start..self.end]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segment {
    /// Unescaped name. For `[foo bar]` this is `foo bar`.
    pub name: String,
    /// Source span of the name only (brackets excluded).
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Path {
    pub span: Span,
    /// `@name` path. The first segment is the data variable (`root`, `index`, `key`, ...).
    pub data: bool,
    /// Number of leading `../`.
    pub depth: usize,
    /// Written with an explicit `this`, `.` or `./` prefix (or is just `this`).
    pub this: bool,
    pub segments: Vec<Segment>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Literal {
    String(String),
    Number(String),
    Bool(bool),
    Null,
    Undefined,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiteralExpr {
    pub span: Span,
    pub value: Literal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubExpr {
    pub span: Span,
    pub call: Call,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expr {
    Path(Path),
    Sub(Box<SubExpr>),
    Literal(LiteralExpr),
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Path(path) => path.span,
            Expr::Sub(sub) => sub.span,
            Expr::Literal(literal) => literal.span,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HashPair {
    pub key: String,
    pub key_span: Span,
    pub value: Expr,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Call {
    pub head: Expr,
    pub params: Vec<Expr>,
    pub hash: Vec<HashPair>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mustache {
    pub span: Span,
    pub call: Call,
    pub escaped: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    /// The opening tag.
    pub open: Span,
    /// The closing tag. A `{{else if}}` chain shares the closing tag of its outermost block.
    pub close: Span,
    pub call: Call,
    pub block_params: Vec<String>,
    pub body: Vec<Node>,
    pub inverse: Option<Vec<Node>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comment {
    pub span: Span,
    /// Text between the delimiters (`{{!--` / `--}}` or `{{!` / `}}`).
    pub text: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    Mustache(Mustache),
    Block(Block),
    Comment(Comment),
    /// `{{> name}}` or `{{#> name}}…{{/name}}`; the contents are not analysed.
    Partial(Span),
    /// `{{{{raw}}}}…{{{{/raw}}}}`; the contents are literal.
    Raw(Span),
}

pub fn parse(source: &str) -> Result<Vec<Node>, ParseError> {
    let mut parser = Parser {
        src: source,
        pos: 0,
        tag_start: 0,
    };
    let (nodes, terminator) = parser.parse_nodes()?;
    match terminator {
        Terminator::Eof => Ok(nodes),
        Terminator::Close { span, name } => Err(ParseError {
            message: format!("Unexpected closing tag `{name}`"),
            span,
        }),
        Terminator::Else { span, .. } => Err(ParseError {
            message: "`else` outside of a block".to_string(),
            span,
        }),
    }
}

struct BlockOpen {
    span: Span,
    call: Call,
    block_params: Vec<String>,
    inverted: bool,
    partial: bool,
}

enum Terminator {
    Eof,
    Close {
        span: Span,
        name: String,
    },
    Else {
        span: Span,
        chain: Option<BlockOpen>,
    },
}

enum Tag {
    Node(Node),
    Open(BlockOpen),
    Terminator(Terminator),
}

struct Parser<'a> {
    src: &'a str,
    pos: usize,
    /// Start of the tag being parsed, for reporting unterminated tags.
    tag_start: usize,
}

fn is_ws(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0c)
}

fn is_id_byte(byte: u8) -> bool {
    !is_ws(byte)
        && !matches!(
            byte,
            b'!' | b'"'
                | b'#'
                | b'%'
                | b'&'
                | b'\''
                | b'('
                | b')'
                | b'*'
                | b'+'
                | b','
                | b'.'
                | b'/'
                | b';'
                | b'<'
                | b'='
                | b'>'
                | b'@'
                | b'['
                | b'\\'
                | b']'
                | b'^'
                | b'`'
                | b'{'
                | b'|'
                | b'}'
                | b'~'
        )
}

fn is_terminator(byte: Option<u8>) -> bool {
    match byte {
        None => true,
        Some(byte) => is_ws(byte) || matches!(byte, b'}' | b')' | b'~' | b'|'),
    }
}

impl<'a> Parser<'a> {
    fn bytes(&self) -> &'a [u8] {
        self.src.as_bytes()
    }

    fn rest(&self) -> &'a str {
        &self.src[self.pos..]
    }

    fn peek(&self) -> Option<u8> {
        self.bytes().get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<u8> {
        self.bytes().get(self.pos + offset).copied()
    }

    fn starts_with(&self, text: &str) -> bool {
        self.rest().starts_with(text)
    }

    fn eat(&mut self, byte: u8) -> bool {
        if self.peek() == Some(byte) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(is_ws) {
            self.pos += 1;
        }
    }

    fn error_here(&self, message: &str) -> ParseError {
        let width = self
            .rest()
            .chars()
            .next()
            .map_or(0, |character| character.len_utf8());
        ParseError {
            message: message.to_string(),
            span: Span::new(self.pos, self.pos + width),
        }
    }

    fn eof_error(&self) -> ParseError {
        ParseError {
            message: "Unterminated tag".to_string(),
            span: Span::new(self.tag_start, self.tag_start + 2),
        }
    }

    fn at_closer(&self) -> bool {
        self.starts_with("}}") || self.starts_with("~}}")
    }

    fn expect_close(&mut self) -> Result<(), ParseError> {
        self.skip_ws();
        self.eat(b'~');
        if self.starts_with("}}") {
            self.pos += 2;
            Ok(())
        } else if self.peek().is_none() {
            Err(self.eof_error())
        } else {
            Err(self.error_here("Expected `}}`"))
        }
    }

    fn parse_nodes(&mut self) -> Result<(Vec<Node>, Terminator), ParseError> {
        let mut nodes = Vec::new();
        loop {
            match self.find_next_tag() {
                None => {
                    self.pos = self.src.len();
                    return Ok((nodes, Terminator::Eof));
                }
                Some(position) => self.pos = position,
            }
            self.tag_start = self.pos;
            match self.parse_tag()? {
                Tag::Node(node) => nodes.push(node),
                Tag::Open(open) => nodes.push(self.parse_block(open)?),
                Tag::Terminator(terminator) => return Ok((nodes, terminator)),
            }
        }
    }

    /// Finds the next unescaped `{{`. `\{{` makes the mustache literal content, `\\{{` is an
    /// escaped backslash followed by a real mustache.
    fn find_next_tag(&self) -> Option<usize> {
        let bytes = self.bytes();
        let mut from = self.pos;
        loop {
            let index = from + self.src[from..].find("{{")?;
            let escaped = index > self.pos
                && bytes[index - 1] == b'\\'
                && !(index - 1 > self.pos && bytes[index - 2] == b'\\');
            if !escaped {
                return Some(index);
            }
            from = index + 2;
        }
    }

    fn parse_tag(&mut self) -> Result<Tag, ParseError> {
        let open = self.pos;
        if self.starts_with("{{{{") {
            return self.parse_raw(open).map(Tag::Node);
        }
        self.pos += 2;
        self.eat(b'~');
        match self.peek() {
            Some(b'!') => self.parse_comment(open).map(Tag::Node),
            Some(b'>') => {
                self.pos += 1;
                self.parse_call(false)?;
                self.expect_close()?;
                Ok(Tag::Node(Node::Partial(Span::new(open, self.pos))))
            }
            Some(b'#') => {
                self.pos += 1;
                let partial = self.eat(b'>');
                let (call, block_params) = self.parse_call(true)?;
                self.expect_close()?;
                Ok(Tag::Open(BlockOpen {
                    span: Span::new(open, self.pos),
                    call,
                    block_params,
                    inverted: false,
                    partial,
                }))
            }
            Some(b'/') => self.parse_close(open),
            Some(b'^') => {
                self.pos += 1;
                self.skip_ws();
                if self.at_closer() {
                    self.expect_close()?;
                    return Ok(Tag::Terminator(Terminator::Else {
                        span: Span::new(open, self.pos),
                        chain: None,
                    }));
                }
                let (call, block_params) = self.parse_call(true)?;
                self.expect_close()?;
                Ok(Tag::Open(BlockOpen {
                    span: Span::new(open, self.pos),
                    call,
                    block_params,
                    inverted: true,
                    partial: false,
                }))
            }
            Some(b'&') => {
                self.pos += 1;
                self.parse_mustache(open, false)
            }
            Some(b'{') => {
                self.pos += 1;
                let (call, _) = self.parse_call(false)?;
                self.skip_ws();
                self.eat(b'~');
                if !self.eat(b'}') {
                    return Err(self.error_here("Expected `}}}`"));
                }
                self.eat(b'~');
                if !self.starts_with("}}") {
                    return Err(self.error_here("Expected `}}}`"));
                }
                self.pos += 2;
                Ok(Tag::Node(Node::Mustache(Mustache {
                    span: Span::new(open, self.pos),
                    call,
                    escaped: false,
                })))
            }
            _ => {
                self.skip_ws();
                if self.at_else_keyword() {
                    self.pos += 4;
                    self.skip_ws();
                    let chain = if self.at_closer() {
                        None
                    } else {
                        let (call, block_params) = self.parse_call(true)?;
                        Some(BlockOpen {
                            span: Span::new(open, self.pos),
                            call,
                            block_params,
                            inverted: false,
                            partial: false,
                        })
                    };
                    self.expect_close()?;
                    return Ok(Tag::Terminator(Terminator::Else {
                        span: Span::new(open, self.pos),
                        chain,
                    }));
                }
                self.parse_mustache(open, true)
            }
        }
    }

    fn at_else_keyword(&self) -> bool {
        self.starts_with("else") && is_terminator(self.peek_at(4))
    }

    fn parse_mustache(&mut self, open: usize, escaped: bool) -> Result<Tag, ParseError> {
        let (call, _) = self.parse_call(false)?;
        self.expect_close()?;
        Ok(Tag::Node(Node::Mustache(Mustache {
            span: Span::new(open, self.pos),
            call,
            escaped,
        })))
    }

    fn parse_close(&mut self, open: usize) -> Result<Tag, ParseError> {
        self.pos += 1;
        let start = self.pos;
        let Some(offset) = self.rest().find("}}") else {
            return Err(self.eof_error());
        };
        let end = start + offset;
        let inner = self.src[start..end].trim_end_matches('~').trim();
        self.pos = end + 2;
        Ok(Tag::Terminator(Terminator::Close {
            span: Span::new(open, self.pos),
            name: inner.to_string(),
        }))
    }

    fn parse_comment(&mut self, open: usize) -> Result<Node, ParseError> {
        self.pos += 1;
        if self.starts_with("--") {
            let body_start = self.pos + 2;
            let mut from = body_start;
            loop {
                let Some(offset) = self.src[from..].find("--") else {
                    return Err(ParseError {
                        message: "Unterminated comment".to_string(),
                        span: Span::new(open, open + 2),
                    });
                };
                let dashes = from + offset;
                let after = &self.src[dashes + 2..];
                let closer = if after.starts_with("}}") {
                    Some(2)
                } else if after.starts_with("~}}") {
                    Some(3)
                } else {
                    None
                };
                if let Some(width) = closer {
                    self.pos = dashes + 2 + width;
                    return Ok(Node::Comment(Comment {
                        span: Span::new(open, self.pos),
                        text: Span::new(body_start, dashes),
                    }));
                }
                from = dashes + 1;
            }
        }
        let body_start = self.pos;
        let Some(offset) = self.rest().find("}}") else {
            return Err(ParseError {
                message: "Unterminated comment".to_string(),
                span: Span::new(open, open + 2),
            });
        };
        let mut body_end = body_start + offset;
        if body_end > body_start && self.bytes()[body_end - 1] == b'~' {
            body_end -= 1;
        }
        self.pos = body_start + offset + 2;
        Ok(Node::Comment(Comment {
            span: Span::new(open, self.pos),
            text: Span::new(body_start, body_end),
        }))
    }

    fn parse_raw(&mut self, open: usize) -> Result<Node, ParseError> {
        self.pos += 4;
        let (call, _) = self.parse_call(false)?;
        self.skip_ws();
        if !self.starts_with("}}}}") {
            return Err(self.error_here("Expected `}}}}`"));
        }
        self.pos += 4;
        let name = call.head.span().text(self.src);
        let closer = ["{{{{/", name, "}}}}"].concat();
        let Some(offset) = self.rest().find(&closer) else {
            return Err(ParseError {
                message: format!("Raw block `{name}` is not closed"),
                span: Span::new(open, self.pos),
            });
        };
        self.pos += offset + closer.len();
        Ok(Node::Raw(Span::new(open, self.pos)))
    }

    fn parse_block(&mut self, open: BlockOpen) -> Result<Node, ParseError> {
        let expected = open.call.head.span().text(self.src).to_string();
        let inverted = open.inverted;
        let partial = open.partial;
        let mut block = self.parse_block_tail(open, &expected)?;
        if partial {
            return Ok(Node::Partial(Span::new(block.open.start, block.close.end)));
        }
        if inverted {
            let content = std::mem::take(&mut block.body);
            block.body = block.inverse.take().unwrap_or_default();
            block.inverse = Some(content);
        }
        Ok(Node::Block(block))
    }

    fn parse_block_tail(&mut self, open: BlockOpen, expected: &str) -> Result<Block, ParseError> {
        let unclosed = |open: &BlockOpen| ParseError {
            message: format!("Block `{expected}` is not closed"),
            span: open.span,
        };
        let (body, terminator) = self.parse_nodes()?;
        let mut inverse = None;
        let close;
        match terminator {
            Terminator::Eof => return Err(unclosed(&open)),
            Terminator::Close { span, name } => {
                Self::check_close(&name, expected, span)?;
                close = span;
            }
            Terminator::Else { chain: None, .. } => {
                let (inverse_nodes, terminator) = self.parse_nodes()?;
                match terminator {
                    Terminator::Eof => return Err(unclosed(&open)),
                    Terminator::Close { span, name } => {
                        Self::check_close(&name, expected, span)?;
                        close = span;
                    }
                    Terminator::Else { span, .. } => {
                        return Err(ParseError {
                            message: "Block has more than one `else`".to_string(),
                            span,
                        });
                    }
                }
                inverse = Some(inverse_nodes);
            }
            Terminator::Else {
                chain: Some(chained_open),
                ..
            } => {
                let chained = self.parse_block_tail(chained_open, expected)?;
                close = chained.close;
                inverse = Some(vec![Node::Block(chained)]);
            }
        }
        Ok(Block {
            open: open.span,
            close,
            call: open.call,
            block_params: open.block_params,
            body,
            inverse,
        })
    }

    fn check_close(name: &str, expected: &str, span: Span) -> Result<(), ParseError> {
        if name == expected {
            Ok(())
        } else {
            Err(ParseError {
                message: format!("Closing tag `{name}` does not match opening block `{expected}`"),
                span,
            })
        }
    }

    fn parse_call(&mut self, allow_block_params: bool) -> Result<(Call, Vec<String>), ParseError> {
        self.skip_ws();
        let head = self.parse_expr()?;
        let mut params = Vec::new();
        let mut hash: Vec<HashPair> = Vec::new();
        let mut block_params = Vec::new();
        loop {
            self.skip_ws();
            match self.peek() {
                None => return Err(self.eof_error()),
                Some(b'}' | b')') => break,
                Some(b'~') if self.peek_at(1) == Some(b'}') => break,
                _ => {}
            }
            if allow_block_params && self.at_block_params() {
                block_params = self.parse_block_params()?;
                self.skip_ws();
                break;
            }
            if let Some((key, key_span)) = self.try_hash_key() {
                let value = self.parse_expr()?;
                hash.push(HashPair {
                    key,
                    key_span,
                    value,
                });
            } else {
                if !hash.is_empty() {
                    return Err(self.error_here("Positional parameter after hash argument"));
                }
                params.push(self.parse_expr()?);
            }
        }
        Ok((Call { head, params, hash }, block_params))
    }

    fn at_block_params(&self) -> bool {
        if !self.starts_with("as") || !self.peek_at(2).is_some_and(is_ws) {
            return false;
        }
        let mut offset = 3;
        while self.peek_at(offset).is_some_and(is_ws) {
            offset += 1;
        }
        self.peek_at(offset) == Some(b'|')
    }

    fn parse_block_params(&mut self) -> Result<Vec<String>, ParseError> {
        self.pos += 2;
        self.skip_ws();
        self.pos += 1;
        let mut names = Vec::new();
        loop {
            self.skip_ws();
            if self.eat(b'|') {
                return Ok(names);
            }
            let start = self.pos;
            while self.peek().is_some_and(is_id_byte) {
                self.pos += 1;
            }
            if self.pos == start {
                return Err(if self.peek().is_none() {
                    self.eof_error()
                } else {
                    self.error_here("Expected block parameter name or `|`")
                });
            }
            names.push(self.src[start..self.pos].to_string());
        }
    }

    fn try_hash_key(&mut self) -> Option<(String, Span)> {
        let start = self.pos;
        let mut end = start;
        while self.bytes().get(end).copied().is_some_and(is_id_byte) {
            end += 1;
        }
        if end > start && self.bytes().get(end) == Some(&b'=') {
            self.pos = end + 1;
            Some((self.src[start..end].to_string(), Span::new(start, end)))
        } else {
            None
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        match self.peek() {
            None => Err(self.eof_error()),
            Some(b'(') => self.parse_sub_expr(),
            Some(quote @ (b'"' | b'\'')) => self.parse_string(quote),
            Some(b'}' | b')' | b'|' | b'=') => Err(self.error_here("Expected expression")),
            Some(b'~') if self.peek_at(1) == Some(b'}') => {
                Err(self.error_here("Expected expression"))
            }
            _ => {
                if let Some(literal) = self.try_literal() {
                    return Ok(Expr::Literal(literal));
                }
                self.parse_path().map(Expr::Path)
            }
        }
    }

    fn parse_sub_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.pos;
        self.pos += 1;
        let (call, _) = self.parse_call(false)?;
        self.skip_ws();
        if !self.eat(b')') {
            return Err(self.error_here("Expected `)`"));
        }
        Ok(Expr::Sub(Box::new(SubExpr {
            span: Span::new(start, self.pos),
            call,
        })))
    }

    fn parse_string(&mut self, quote: u8) -> Result<Expr, ParseError> {
        let start = self.pos;
        self.pos += 1;
        let mut value = String::new();
        let mut chunk_start = self.pos;
        loop {
            match self.peek() {
                None => return Err(self.eof_error()),
                Some(b'\\') if self.peek_at(1) == Some(quote) => {
                    value.push_str(&self.src[chunk_start..self.pos]);
                    value.push(quote as char);
                    self.pos += 2;
                    chunk_start = self.pos;
                }
                Some(byte) if byte == quote => {
                    value.push_str(&self.src[chunk_start..self.pos]);
                    self.pos += 1;
                    return Ok(Expr::Literal(LiteralExpr {
                        span: Span::new(start, self.pos),
                        value: Literal::String(value),
                    }));
                }
                Some(_) => self.pos += 1,
            }
        }
    }

    fn try_literal(&mut self) -> Option<LiteralExpr> {
        let start = self.pos;
        let bytes = self.bytes();
        let mut end = start;
        if bytes.get(end) == Some(&b'-') {
            end += 1;
        }
        let digits_start = end;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
        if end > digits_start {
            if bytes.get(end) == Some(&b'.') && bytes.get(end + 1).is_some_and(u8::is_ascii_digit) {
                end += 1;
                while bytes.get(end).is_some_and(u8::is_ascii_digit) {
                    end += 1;
                }
            }
            if is_terminator(bytes.get(end).copied()) {
                self.pos = end;
                return Some(LiteralExpr {
                    span: Span::new(start, end),
                    value: Literal::Number(self.src[start..end].to_string()),
                });
            }
        }
        let keywords = [
            ("true", Literal::Bool(true)),
            ("false", Literal::Bool(false)),
            ("null", Literal::Null),
            ("undefined", Literal::Undefined),
        ];
        for (keyword, value) in keywords {
            if self.starts_with(keyword) && is_terminator(bytes.get(start + keyword.len()).copied())
            {
                self.pos = start + keyword.len();
                return Some(LiteralExpr {
                    span: Span::new(start, self.pos),
                    value,
                });
            }
        }
        None
    }

    fn parse_path(&mut self) -> Result<Path, ParseError> {
        struct Token {
            name: String,
            span: Span,
            bracketed: bool,
        }

        let start = self.pos;
        let data = self.eat(b'@');
        let mut tokens: Vec<Token> = Vec::new();
        loop {
            let (name, span, bracketed) = self.parse_segment()?;
            tokens.push(Token {
                name,
                span,
                bracketed,
            });
            if matches!(self.peek(), Some(b'.' | b'/')) {
                self.pos += 1;
            } else {
                break;
            }
        }

        let mut depth = 0;
        let mut this = false;
        let mut segments: Vec<Segment> = Vec::new();
        for token in tokens {
            let special = !token.bracketed && matches!(token.name.as_str(), ".." | "." | "this");
            if special && segments.is_empty() && !(this && token.name == "..") {
                match token.name.as_str() {
                    ".." => depth += 1,
                    _ => this = true,
                }
            } else if special {
                return Err(ParseError {
                    message: format!(
                        "Invalid path: `{}` is only allowed at the start",
                        token.name
                    ),
                    span: token.span,
                });
            } else {
                segments.push(Segment {
                    name: token.name,
                    span: token.span,
                });
            }
        }
        Ok(Path {
            span: Span::new(start, self.pos),
            data,
            depth,
            this,
            segments,
        })
    }

    fn parse_segment(&mut self) -> Result<(String, Span, bool), ParseError> {
        match self.peek() {
            Some(b'[') => {
                let inner_start = self.pos + 1;
                let mut name = String::new();
                let mut chunk_start = inner_start;
                self.pos = inner_start;
                loop {
                    match self.peek() {
                        None => return Err(self.eof_error()),
                        Some(b'\\') if self.peek_at(1) == Some(b']') => {
                            name.push_str(&self.src[chunk_start..self.pos]);
                            name.push(']');
                            self.pos += 2;
                            chunk_start = self.pos;
                        }
                        Some(b']') => {
                            name.push_str(&self.src[chunk_start..self.pos]);
                            let span = Span::new(inner_start, self.pos);
                            self.pos += 1;
                            return Ok((name, span, true));
                        }
                        Some(_) => self.pos += 1,
                    }
                }
            }
            Some(b'.') => {
                let start = self.pos;
                self.pos += if self.starts_with("..") { 2 } else { 1 };
                Ok((
                    self.src[start..self.pos].to_string(),
                    Span::new(start, self.pos),
                    false,
                ))
            }
            _ => {
                let start = self.pos;
                while self.peek().is_some_and(is_id_byte) {
                    self.pos += 1;
                }
                if self.pos == start {
                    return Err(if self.peek().is_none() {
                        self.eof_error()
                    } else {
                        self.error_here("Expected path segment")
                    });
                }
                Ok((
                    self.src[start..self.pos].to_string(),
                    Span::new(start, self.pos),
                    false,
                ))
            }
        }
    }
}
