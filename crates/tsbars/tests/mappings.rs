mod common;

use common::run;
use tsbars::{MapResult, MappingKind, features};

const RICH: &str = "\
{{!-- @import type { User } from \"./user\" --}}
{{!-- @param owner User --}}
{{!-- @param stats.count number --}}
Héllo {{name}} {{user.日本語}} {{[foo bar]}} {{foo-bar}}
{{#each items as |item|}}{{item.title}} {{../shop}} {{@key}}{{/each}}
{{#if admin}}✓ {{perk}}{{else}}{{#with guest}}{{nick}}{{/with}}{{/if}}
{{upper (lower shout)}} {{> partial}} \\{{literal}}
";

fn assert_mappings_are_well_formed(source: &str, result: &MapResult) {
    let mut last_virtual_end = 0;
    for mapping in &result.mappings {
        let virtual_end = mapping.virtual_start + mapping.virtual_len;
        let original_end = mapping.original_start + mapping.original_len;
        assert!(virtual_end <= result.text.len(), "{mapping:?}");
        assert!(original_end <= source.len(), "{mapping:?}");
        for (text, offsets) in [
            (&result.text, [mapping.virtual_start, virtual_end]),
            (&source.to_string(), [mapping.original_start, original_end]),
        ] {
            for offset in offsets {
                assert!(text.is_char_boundary(offset), "{mapping:?} splits a char");
            }
        }
        assert!(
            mapping.virtual_start >= last_virtual_end,
            "mappings overlap or are unsorted: {mapping:?}"
        );
        last_virtual_end = virtual_end;
    }
}

#[test]
fn verbatim_mappings_have_identical_virtual_and_original_text() {
    let result = run(RICH);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_mappings_are_well_formed(RICH, &result);

    let verbatim: Vec<_> = result
        .mappings
        .iter()
        .filter(|mapping| mapping.kind == MappingKind::Verbatim)
        .collect();
    assert!(verbatim.len() > 8);
    for mapping in verbatim {
        assert_eq!(mapping.virtual_len, mapping.original_len);
        assert_eq!(
            &result.text[mapping.virtual_start..mapping.virtual_start + mapping.virtual_len],
            &RICH[mapping.original_start..mapping.original_start + mapping.original_len],
            "{mapping:?}"
        );
    }
}

#[test]
fn every_property_key_maps_to_its_first_reference() {
    let source = "{{#each items}}{{title}} {{../shop.owner}}{{/each}} {{name}} {{name}}";
    let result = run(source);
    let mapped: Vec<(&str, &str)> = result
        .mappings
        .iter()
        .filter(|mapping| {
            mapping.features == features::HOVER | features::DEFINITION | features::REFERENCES
        })
        .map(|mapping| {
            (
                &result.text[mapping.virtual_start..mapping.virtual_start + mapping.virtual_len],
                &source[mapping.original_start..mapping.original_start + mapping.original_len],
            )
        })
        .collect();
    assert_eq!(
        mapped,
        vec![
            ("items", "items"),
            ("title", "title"),
            ("shop", "shop"),
            ("owner", "owner"),
            ("name", "name"),
        ]
    );
    let name = result
        .mappings
        .iter()
        .find(|mapping| source[mapping.original_start..].starts_with("name"))
        .unwrap();
    assert_eq!(name.original_start, source.find("name").unwrap());
}

#[test]
fn key_mappings_use_hover_definition_references() {
    let result = run("{{name}}");
    let key = &result.mappings[0];
    assert_eq!(key.kind, MappingKind::Verbatim);
    assert_eq!(key.features, 73);
}

#[test]
fn quoted_keys_map_the_inner_text() {
    let source = "{{[foo bar]}} {{foo-bar}}";
    let result = run(source);
    let texts: Vec<_> = result
        .mappings
        .iter()
        .take(2)
        .map(|mapping| {
            assert_eq!(mapping.kind, MappingKind::Verbatim);
            (
                &result.text[mapping.virtual_start..mapping.virtual_start + mapping.virtual_len],
                &source[mapping.original_start..mapping.original_start + mapping.original_len],
            )
        })
        .collect();
    assert_eq!(texts, vec![("foo bar", "foo bar"), ("foo-bar", "foo-bar")]);
}

#[test]
fn keys_that_change_when_escaped_map_as_atoms() {
    let source = "{{[a\"b]}}";
    let result = run(source);
    assert!(result.text.contains("\"a\\\"b\": Printable"));
    let key = &result.mappings[0];
    assert_eq!(key.kind, MappingKind::Atom);
    assert_eq!(
        &source[key.original_start..key.original_start + key.original_len],
        "a\"b"
    );
    assert_mappings_are_well_formed(source, &result);
}

#[test]
fn param_type_text_maps_verbatim_with_all_features() {
    let source = "{{!--   @param user   Array<{ id: string }>   --}}";
    let result = run(source);
    let mapping = result
        .mappings
        .iter()
        .find(|mapping| mapping.features == features::ALL)
        .unwrap();
    assert_eq!(mapping.kind, MappingKind::Verbatim);
    assert_eq!(
        &source[mapping.original_start..mapping.original_start + mapping.original_len],
        "Array<{ id: string }>"
    );
    assert_eq!(
        &result.text[mapping.virtual_start..mapping.virtual_start + mapping.virtual_len],
        "Array<{ id: string }>"
    );
}

#[test]
fn import_clause_maps_verbatim_with_all_features() {
    let source = "{{!-- @import type { User } from \"./user\" --}}";
    let result = run(source);
    let mapping = &result.mappings[0];
    assert_eq!(mapping.kind, MappingKind::Verbatim);
    assert_eq!(mapping.features, features::ALL);
    assert_eq!(
        &result.text[mapping.virtual_start..mapping.virtual_start + mapping.virtual_len],
        "type { User } from \"./user\""
    );
    assert!(
        result
            .text
            .starts_with("import type { User } from \"./user\";\n")
    );
}

#[test]
fn export_default_maps_to_the_whole_template_as_an_atom() {
    let source = "héllo {{name}}";
    let result = run(source);
    let mapping = result.mappings.last().unwrap();
    assert_eq!(mapping.kind, MappingKind::Atom);
    assert_eq!(
        mapping.features,
        features::DEFINITION | features::TYPE_DEFINITION
    );
    assert_eq!(mapping.features, 24);
    assert_eq!(mapping.original_start, 0);
    assert_eq!(mapping.original_len, source.len());
    assert_eq!(
        &result.text[mapping.virtual_start..mapping.virtual_start + mapping.virtual_len],
        "export default template"
    );
}

#[test]
fn fallback_module_still_maps_the_default_export() {
    let source = "{{#if}}";
    let result = run(source);
    assert_eq!(result.diagnostics.len(), 1);
    let mapping = &result.mappings[0];
    assert_eq!(mapping.kind, MappingKind::Atom);
    assert_eq!(mapping.original_len, source.len());
}

#[test]
fn parse_error_spans_sit_on_char_boundaries() {
    for source in [
        "é {{é",
        "日本 {{/x}}",
        "{{#é}} {{/ü}}",
        "{{é.}}",
        "{{\"é}}",
        "{{é é=}}",
    ] {
        let result = run(source);
        assert_eq!(result.diagnostics.len(), 1, "{source}");
        let diagnostic = &result.diagnostics[0];
        assert!(source.is_char_boundary(diagnostic.start), "{source}");
        assert!(
            source.is_char_boundary(diagnostic.start + diagnostic.length),
            "{source}"
        );
    }
}

#[test]
fn mapping_is_deterministic() {
    let first = run(RICH);
    let second = run(RICH);
    assert_eq!(first.text, second.text);
    assert_eq!(first.mappings, second.mappings);
}

#[test]
fn emitted_text_for_rich_template() {
    let result = run(RICH);
    assert_eq!(
        result.text,
        "\
import type { User } from \"./user\";
type Printable = string | number | bigint | boolean;
export type Context = {
  name: Printable;
  user: {
    \"日本語\": Printable;
  };
  \"foo bar\": Printable;
  \"foo-bar\": Printable;
  items: Readonly<Record<string, {
    title: Printable;
  }>>;
  shop: Printable;
  admin?: unknown;
  perk: Printable;
  guest: {
    nick: Printable;
  };
  shout: unknown;
  owner: User;
  stats: {
    count: number;
  };
};
declare const template: (context: Context) => string;
export default template;
"
    );
}

#[test]
fn every_prefix_of_a_template_maps_without_panicking() {
    for (end, _) in RICH.char_indices().chain([(RICH.len(), ' ')]) {
        let source = &RICH[..end];
        let result = run(source);
        assert_mappings_are_well_formed(source, &result);
        for diagnostic in &result.diagnostics {
            assert!(source.is_char_boundary(diagnostic.start));
            assert!(source.is_char_boundary(diagnostic.start + diagnostic.length));
        }
    }
}
