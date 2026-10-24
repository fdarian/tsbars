use tsbars::parser::{Expr, Literal, Node, ParseError, Span, parse};

fn error(source: &str) -> ParseError {
    parse(source).expect_err("expected a parse error")
}

fn error_text(source: &str) -> (String, String) {
    let error = error(source);
    (
        error.message,
        source[error.span.start..error.span.end].to_string(),
    )
}

#[test]
fn path_segments_keep_their_own_spans() {
    let source = "x {{ ../../user.[first name]/age }}";
    let nodes = parse(source).unwrap();
    let Node::Mustache(mustache) = &nodes[0] else {
        panic!("expected a mustache");
    };
    let Expr::Path(path) = &mustache.call.head else {
        panic!("expected a path");
    };
    assert_eq!(path.depth, 2);
    assert!(!path.this && !path.data);
    let segments: Vec<_> = path
        .segments
        .iter()
        .map(|segment| (segment.name.as_str(), segment.span.text(source)))
        .collect();
    assert_eq!(
        segments,
        vec![
            ("user", "user"),
            ("first name", "first name"),
            ("age", "age")
        ]
    );
    assert_eq!(path.span.text(source), "../../user.[first name]/age");
}

#[test]
fn data_paths_and_literals() {
    let source = "{{helper @root.a 1 -2.5 \"s\\\"q\" 'x' true false null undefined k=@index}}";
    let nodes = parse(source).unwrap();
    let Node::Mustache(mustache) = &nodes[0] else {
        panic!("expected a mustache");
    };
    let Expr::Path(data) = &mustache.call.params[0] else {
        panic!("expected a path");
    };
    assert!(data.data);
    assert_eq!(data.segments[0].name, "root");
    let literals: Vec<_> = mustache.call.params[1..]
        .iter()
        .map(|param| match param {
            Expr::Literal(literal) => literal.value.clone(),
            other => panic!("expected literal, got {other:?}"),
        })
        .collect();
    assert_eq!(
        literals,
        vec![
            Literal::Number("1".into()),
            Literal::Number("-2.5".into()),
            Literal::String("s\"q".into()),
            Literal::String("x".into()),
            Literal::Bool(true),
            Literal::Bool(false),
            Literal::Null,
            Literal::Undefined,
        ]
    );
    assert_eq!(mustache.call.hash[0].key, "k");
}

#[test]
fn blocks_carry_params_inverse_and_close() {
    let source = "{{#each xs as |x i|}}a{{else}}b{{/each}}";
    let nodes = parse(source).unwrap();
    let Node::Block(block) = &nodes[0] else {
        panic!("expected a block");
    };
    assert_eq!(block.block_params, vec!["x", "i"]);
    assert_eq!(block.open.text(source), "{{#each xs as |x i|}}");
    assert_eq!(block.close.text(source), "{{/each}}");
    assert!(block.inverse.is_some());
}

#[test]
fn else_chains_nest_in_the_inverse_and_share_the_close() {
    let source = "{{#if a}}1{{else if b}}2{{else}}3{{/if}}";
    let nodes = parse(source).unwrap();
    let Node::Block(outer) = &nodes[0] else {
        panic!("expected a block");
    };
    let Some(inverse) = &outer.inverse else {
        panic!("expected an inverse");
    };
    let Node::Block(chained) = &inverse[0] else {
        panic!("expected a chained block");
    };
    assert_eq!(chained.call.head.span().text(source), "if");
    assert_eq!(chained.close, outer.close);
    assert!(chained.inverse.is_some());
}

#[test]
fn inverse_section_swaps_into_the_inverse() {
    let source = "{{^items}}none{{/items}}";
    let nodes = parse(source).unwrap();
    let Node::Block(block) = &nodes[0] else {
        panic!("expected a block");
    };
    assert!(block.body.is_empty());
    assert!(block.inverse.is_some());
}

#[test]
fn comments_expose_their_text_span() {
    let source = "{{!-- long }} text --}}{{! short ~}}";
    let nodes = parse(source).unwrap();
    let texts: Vec<_> = nodes
        .iter()
        .map(|node| match node {
            Node::Comment(comment) => comment.text.text(source),
            other => panic!("expected comment, got {other:?}"),
        })
        .collect();
    assert_eq!(texts, vec![" long }} text ", " short "]);
}

#[test]
fn raw_blocks_and_partials_are_opaque_nodes() {
    let source = "{{{{raw}}}} {{x}} {{{{/raw}}}}{{> p a b=c}}{{#> layout}}{{y}}{{/layout}}";
    let nodes = parse(source).unwrap();
    assert!(matches!(nodes[0], Node::Raw(_)));
    assert!(matches!(nodes[1], Node::Partial(_)));
    assert!(matches!(nodes[2], Node::Partial(_)));
    assert_eq!(nodes.len(), 3);
}

#[test]
fn escapes_make_mustaches_literal() {
    assert!(parse("\\{{x}}").unwrap().is_empty());
    assert!(parse("\\{{x}} and {{y}}").unwrap().len() == 1);
    assert_eq!(parse("\\\\{{x}}").unwrap().len(), 1);
}

#[test]
fn mismatched_close_tag() {
    assert_eq!(
        error_text("{{#if a}}x{{/each}}"),
        (
            "Closing tag `each` does not match opening block `if`".to_string(),
            "{{/each}}".to_string()
        )
    );
}

#[test]
fn unclosed_block_points_at_the_open_tag() {
    assert_eq!(
        error_text("a {{#each xs}} b"),
        (
            "Block `each` is not closed".to_string(),
            "{{#each xs}}".to_string()
        )
    );
}

#[test]
fn unterminated_tag_points_at_the_open_braces() {
    assert_eq!(error_text("a {{name").1, "{{");
    assert_eq!(error_text("a {{name b=").1, "{{");
    assert_eq!(error_text("a {{!-- never closed").1, "{{");
    assert_eq!(error_text("a {{{{raw}}}} never").1, "{{{{raw}}}}");
}

#[test]
fn stray_close_and_else() {
    assert_eq!(error_text("x {{/if}}").1, "{{/if}}");
    assert_eq!(error_text("x {{else}}").1, "{{else}}");
}

#[test]
fn malformed_expressions() {
    assert_eq!(error_text("{{}}").1, "}");
    assert_eq!(error_text("{{a.}}").1, "}");
    assert_eq!(
        error_text("{{a..b}}").0,
        "Invalid path: `.` is only allowed at the start"
    );
    assert_eq!(error_text("{{(a}}").1, "}");
    assert_eq!(
        error_text("{{a b=1 c}}").0,
        "Positional parameter after hash argument"
    );
    assert_eq!(error_text("{{\"abc}}").1, "{{");
    assert_eq!(error_text("{{a}} {{b c").1, "{{");
}

#[test]
fn spans_are_valid_utf8_slices() {
    let source = "日本 {{#each 項目}}{{名前}}{{/each}}";
    let nodes = parse(source).unwrap();
    let Node::Block(block) = &nodes[0] else {
        panic!("expected a block");
    };
    assert_eq!(block.open.text(source), "{{#each 項目}}");
    let Span { start, end } = block.close;
    assert_eq!(&source[start..end], "{{/each}}");
}
