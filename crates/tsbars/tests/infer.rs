mod common;

use common::{context, context_with_helpers, run};

#[test]
fn printed_variables_are_printable() {
    assert_eq!(
        context("Hello {{name}}, {{{bio}}} {{&motto}} {{name}}"),
        "{\n  name: Printable;\n  bio: Printable;\n  motto: Printable;\n}"
    );
}

#[test]
fn property_order_is_first_seen_order() {
    assert_eq!(
        context("{{zeta}} {{alpha}} {{zeta}} {{mid}}"),
        "{\n  zeta: Printable;\n  alpha: Printable;\n  mid: Printable;\n}"
    );
}

#[test]
fn dotted_paths_become_nested_objects() {
    assert_eq!(
        context("{{user.name}} {{user.address/city}} {{user.age}}"),
        "{\n  user: {\n    name: Printable;\n    address: {\n      city: Printable;\n    };\n    age: Printable;\n  };\n}"
    );
}

#[test]
fn object_wins_over_printed_leaf_in_either_order() {
    let expected = "{\n  user: {\n    name: Printable;\n  };\n}";
    assert_eq!(context("{{user}} {{user.name}}"), expected);
    assert_eq!(context("{{user.name}} {{user}}"), expected);
}

#[test]
fn each_makes_an_array_of_the_element_shape() {
    assert_eq!(
        context("{{#each items}}{{title}} {{price}}{{/each}}"),
        "{\n  items: ReadonlyArray<{\n    title: Printable;\n    price: Printable;\n  }>;\n}"
    );
}

#[test]
fn each_with_key_is_a_record() {
    assert_eq!(
        context("{{#each scores}}{{@key}}: {{this}}{{/each}}"),
        "{\n  scores: Readonly<Record<string, Printable>>;\n}"
    );
    assert_eq!(
        context("{{#each people}}{{#if @key}}{{name}}{{/if}}{{/each}}"),
        "{\n  people: Readonly<Record<string, {\n    name: Printable;\n  }>>;\n}"
    );
}

#[test]
fn key_of_the_inner_each_does_not_make_the_outer_one_a_record() {
    assert_eq!(
        context("{{#each rows}}{{#each cells}}{{@key}}{{/each}}{{/each}}"),
        "{\n  rows: ReadonlyArray<{\n    cells: Readonly<Record<string, unknown>>;\n  }>;\n}"
    );
}

#[test]
fn each_over_primitives_prints_this() {
    assert_eq!(
        context("{{#each tags}}{{this}}, {{/each}}"),
        "{\n  tags: ReadonlyArray<Printable>;\n}"
    );
    assert_eq!(
        context("{{#each tags}}{{.}}{{/each}}"),
        "{\n  tags: ReadonlyArray<Printable>;\n}"
    );
}

#[test]
fn each_without_usage_has_unknown_elements() {
    assert_eq!(
        context("{{#each items}}x{{/each}}"),
        "{\n  items: ReadonlyArray<unknown>;\n}"
    );
}

#[test]
fn each_block_params_name_the_element_and_ignore_the_index() {
    assert_eq!(
        context("{{#each items as |item idx|}}{{idx}} {{item.title}} {{item}}{{/each}}"),
        "{\n  items: ReadonlyArray<{\n    title: Printable;\n  }>;\n}"
    );
    assert_eq!(
        context("{{#each tags as |tag|}}{{tag}}{{/each}}"),
        "{\n  tags: ReadonlyArray<Printable>;\n}"
    );
}

#[test]
fn each_block_params_keep_the_element_as_context() {
    assert_eq!(
        context("{{#each items as |item|}}{{title}}{{item.price}}{{/each}}"),
        "{\n  items: ReadonlyArray<{\n    title: Printable;\n    price: Printable;\n  }>;\n}"
    );
}

#[test]
fn each_else_uses_the_outer_scope() {
    assert_eq!(
        context("{{#each items}}{{title}}{{else}}{{fallback}}{{/each}}"),
        "{\n  items: ReadonlyArray<{\n    title: Printable;\n  }>;\n  fallback: Printable;\n}"
    );
}

#[test]
fn each_is_optional_only_when_guarded() {
    assert_eq!(
        context("{{#if items}}{{#each items}}{{this}}{{/each}}{{/if}}"),
        "{\n  items?: ReadonlyArray<Printable>;\n}"
    );
}

#[test]
fn nested_each_builds_nested_arrays() {
    assert_eq!(
        context("{{#each groups}}{{name}}{{#each members}}{{email}}{{/each}}{{/each}}"),
        "{\n  groups: ReadonlyArray<{\n    name: Printable;\n    members: ReadonlyArray<{\n      email: Printable;\n    }>;\n  }>;\n}"
    );
}

#[test]
fn each_wins_over_property_access_in_either_order() {
    let expected = "{\n  items: ReadonlyArray<{\n    title: Printable;\n  }>;\n}";
    assert_eq!(
        context("{{items.length}}{{#each items}}{{title}}{{/each}}"),
        expected
    );
    assert_eq!(
        context("{{#each items}}{{title}}{{/each}}{{items.length}}"),
        expected
    );
}

#[test]
fn if_makes_the_condition_optional_and_unknown() {
    assert_eq!(
        context("{{#if admin}}yes{{/if}}"),
        "{\n  admin?: unknown;\n}"
    );
    assert_eq!(
        context("{{#unless hidden}}yes{{/unless}}"),
        "{\n  hidden?: unknown;\n}"
    );
}

#[test]
fn if_keeps_the_scope_and_constrains_by_usage() {
    assert_eq!(
        context("{{#if user}}{{user.name}}{{else}}{{guest}}{{/if}}"),
        "{\n  user?: {\n    name: Printable;\n  };\n  guest: Printable;\n}"
    );
}

#[test]
fn else_if_chains_guard_every_condition() {
    assert_eq!(
        context("{{#if a}}1{{else if b}}2{{else if c}}3{{else}}{{d}}{{/if}}"),
        "{\n  a?: unknown;\n  b?: unknown;\n  c?: unknown;\n  d: Printable;\n}"
    );
}

#[test]
fn inverse_sections_and_caret_else() {
    assert_eq!(
        context("{{#if a}}x{{^}}{{b}}{{/if}}"),
        "{\n  a?: unknown;\n  b: Printable;\n}"
    );
    assert_eq!(
        context("{{^items}}none{{/items}}"),
        "{\n  items?: unknown;\n}"
    );
}

#[test]
fn guarded_intermediate_segments_stay_required() {
    assert_eq!(
        context("{{#if user.admin}}x{{/if}}"),
        "{\n  user: {\n    admin?: unknown;\n  };\n}"
    );
}

#[test]
fn with_scopes_into_a_required_object() {
    assert_eq!(
        context("{{#with user}}{{name}} {{email}}{{/with}}"),
        "{\n  user: {\n    name: Printable;\n    email: Printable;\n  };\n}"
    );
}

#[test]
fn with_else_makes_the_object_optional_and_else_uses_the_outer_scope() {
    assert_eq!(
        context("{{#with user}}{{name}}{{else}}{{anon}}{{/with}}"),
        "{\n  user?: {\n    name: Printable;\n  };\n  anon: Printable;\n}"
    );
}

#[test]
fn with_block_params_name_the_object() {
    assert_eq!(
        context("{{#with user as |u|}}{{u.name}}{{email}}{{/with}}"),
        "{\n  user: {\n    name: Printable;\n    email: Printable;\n  };\n}"
    );
}

#[test]
fn parent_paths_pop_one_scope_per_segment() {
    assert_eq!(
        context(
            "{{#each items}}{{title}} {{../shop}}{{#each tags}}{{../../owner}} {{../title}} {{label}}{{/each}}{{/each}}"
        ),
        "{\n  items: ReadonlyArray<{\n    title: Printable;\n    tags: ReadonlyArray<{\n      label: Printable;\n    }>;\n  }>;\n  shop: Printable;\n  owner: Printable;\n}"
    );
}

#[test]
fn if_does_not_push_a_scope_for_parent_paths() {
    assert_eq!(
        context("{{#each items}}{{#if show}}{{../shop}}{{/if}}{{/each}}"),
        "{\n  items: ReadonlyArray<{\n    show?: unknown;\n  }>;\n  shop: Printable;\n}"
    );
}

#[test]
fn parent_paths_beyond_the_root_are_ignored() {
    assert_eq!(context("{{../../x}}{{a}}"), "{\n  a: Printable;\n}");
}

#[test]
fn root_paths_resolve_from_the_root_scope() {
    assert_eq!(
        context("{{#each items}}{{@root.site.name}} {{title}}{{/each}}"),
        "{\n  items: ReadonlyArray<{\n    title: Printable;\n  }>;\n  site: {\n    name: Printable;\n  };\n}"
    );
}

#[test]
fn this_prefixes_are_transparent() {
    assert_eq!(
        context("{{this.a}} {{./b}} {{this/c}} {{a}}"),
        "{\n  a: Printable;\n  b: Printable;\n  c: Printable;\n}"
    );
}

#[test]
fn data_variables_contribute_nothing() {
    assert_eq!(
        context("{{#each items}}{{@index}}{{@first}}{{@last}}{{/each}}"),
        "{\n  items: ReadonlyArray<unknown>;\n}"
    );
}

#[test]
fn helper_params_are_required_unknowns() {
    assert_eq!(
        context("{{upper name}} {{fmt price currency=cur}}"),
        "{\n  name: unknown;\n  price: unknown;\n  cur: unknown;\n}"
    );
}

#[test]
fn printed_wins_over_helper_param_usage() {
    assert_eq!(
        context("{{upper name}} {{name}}"),
        "{\n  name: Printable;\n}"
    );
}

#[test]
fn helper_params_ignore_literals_and_the_helper_name() {
    assert_eq!(
        context("{{join items \", \" 3 true null undefined}}"),
        "{\n  items: unknown;\n}"
    );
}

#[test]
fn bare_names_are_variables_unless_declared_helpers() {
    assert_eq!(context("{{now}}"), "{\n  now: Printable;\n}");
    assert_eq!(
        context_with_helpers("{{now}} {{name}}", &["now"]),
        "{\n  name: Printable;\n}"
    );
}

#[test]
fn declared_helpers_as_blocks_do_not_become_properties() {
    assert_eq!(
        context_with_helpers(
            "{{#admin}}{{secret}}{{else}}{{public}}{{/admin}}",
            &["admin"]
        ),
        "{\n  public: Printable;\n}"
    );
}

#[test]
fn unknown_block_helper_bodies_record_nothing() {
    assert_eq!(
        context("{{#custom a b=c}}{{inside}}{{/custom}}"),
        "{\n  a: unknown;\n  c: unknown;\n}"
    );
}

#[test]
fn parent_paths_out_of_an_unknown_scope_reach_the_enclosing_scope() {
    assert_eq!(
        context("{{#custom a}}{{../up}}{{/custom}}"),
        "{\n  a: unknown;\n  up: Printable;\n}"
    );
}

#[test]
fn unknown_block_helper_else_uses_the_outer_scope() {
    assert_eq!(
        context("{{#custom a}}{{inside}}{{else}}{{outside}}{{/custom}}"),
        "{\n  a: unknown;\n  outside: Printable;\n}"
    );
}

#[test]
fn unknown_block_helper_params_are_bound_to_unknown() {
    assert_eq!(
        context("{{#custom a as |x y|}}{{x.foo}}{{y}}{{/custom}}"),
        "{\n  a: unknown;\n}"
    );
}

#[test]
fn bare_variable_sections_are_guards_with_untracked_bodies() {
    assert_eq!(
        context("{{#flag}}{{inside}}{{else}}{{outside}}{{/flag}}"),
        "{\n  flag?: unknown;\n  outside: Printable;\n}"
    );
}

#[test]
fn sub_expressions_reference_their_params() {
    assert_eq!(
        context("{{#if (eq a (not b))}}x{{/if}}"),
        "{\n  a: unknown;\n  b: unknown;\n}"
    );
    assert_eq!(
        context("{{greet (upper name) (lower user.first)}}"),
        "{\n  name: unknown;\n  user: {\n    first: unknown;\n  };\n}"
    );
}

#[test]
fn sub_expression_as_each_target_has_an_untracked_body() {
    assert_eq!(
        context("{{#each (sort items)}}{{title}}{{/each}}"),
        "{\n  items: unknown;\n}"
    );
}

#[test]
fn lookup_references_the_target_and_prints_the_key() {
    assert_eq!(
        context("{{lookup people who}}"),
        "{\n  people: unknown;\n  who: Printable;\n}"
    );
    assert_eq!(
        context("{{#if (lookup flags name)}}x{{/if}}"),
        "{\n  flags: unknown;\n  name: Printable;\n}"
    );
}

#[test]
fn log_params_are_unknown() {
    assert_eq!(context("{{log debug}}"), "{\n  debug: unknown;\n}");
}

#[test]
fn partials_and_raw_blocks_contribute_nothing() {
    assert_eq!(
        context(
            "{{> header title}} {{#> layout}}{{inner}}{{/layout}} {{{{raw}}}}{{notAVar}}{{{{/raw}}}} {{x}}"
        ),
        "{\n  x: Printable;\n}"
    );
}

#[test]
fn comments_and_escaped_mustaches_contribute_nothing() {
    assert_eq!(
        context("{{! note }} {{!-- {{inner}} --}} \\{{literal}} {{x}}"),
        "{\n  x: Printable;\n}"
    );
    assert_eq!(context("\\\\{{real}}"), "{\n  real: Printable;\n}");
}

#[test]
fn whitespace_control_is_accepted_everywhere() {
    assert_eq!(
        context("{{~#each items~}} {{~title~}} {{~/each~}} {{~{raw}~}} {{~&amp~}}"),
        "{\n  items: ReadonlyArray<{\n    title: Printable;\n  }>;\n  raw: Printable;\n  amp: Printable;\n}"
    );
}

#[test]
fn quoted_keys_for_non_identifiers() {
    assert_eq!(
        context("{{[foo bar]}} {{foo-bar}} {{a.[b c]}} {{9lives}}"),
        "{\n  \"foo bar\": Printable;\n  \"foo-bar\": Printable;\n  a: {\n    \"b c\": Printable;\n  };\n  \"9lives\": Printable;\n}"
    );
}

#[test]
fn empty_context_is_a_closed_record_with_an_optional_parameter() {
    let result = run("Just text, {{! a comment }} nothing else.");
    assert!(result.diagnostics.is_empty());
    assert_eq!(
        result.text,
        "export type Context = Record<string, never>;\n\
         declare const template: (context?: Context) => string;\n\
         export default template;\n"
    );
}

#[test]
fn full_module_layout() {
    let result = run("{{name}}");
    assert_eq!(
        result.text,
        "type Printable = string | number | bigint | boolean;\n\
         export type Context = {\n  name: Printable;\n};\n\
         declare const template: (context: Context) => string;\n\
         export default template;\n"
    );
}

#[test]
fn printable_is_declared_only_when_used() {
    assert!(!run("{{#if a}}x{{/if}}").text.contains("Printable"));
    assert!(run("{{a}}").text.contains("type Printable"));
}

#[test]
fn param_replaces_the_inferred_shape() {
    let source = "{{!-- @param user User --}}{{user.name}}";
    assert_eq!(context(source), "{\n  user: User;\n}");
}

#[test]
fn param_with_dotted_path_creates_missing_parents() {
    assert_eq!(
        context("{{!-- @param user.age number --}}{{user.name}}"),
        "{\n  user: {\n    name: Printable;\n    age: number;\n  };\n}"
    );
    assert_eq!(
        context("{{!-- @param a.b.c Foo --}}"),
        "{\n  a: {\n    b: {\n      c: Foo;\n    };\n  };\n}"
    );
}

#[test]
fn param_replaces_arrays_and_keeps_optionality() {
    assert_eq!(
        context(
            "{{#if items}}{{#each items}}{{title}}{{/each}}{{/if}}{{!-- @param items Item[] --}}"
        ),
        "{\n  items?: Item[];\n}"
    );
}

#[test]
fn param_works_in_short_comments_and_with_complex_types() {
    assert_eq!(
        context("{{! @param opts { a: string; b?: Array<number> } }}"),
        "{\n  opts: { a: string; b?: Array<number> };\n}"
    );
}

#[test]
fn param_can_appear_after_its_usage_and_anywhere_at_top_level() {
    assert_eq!(
        context("{{#each rows}}{{x}}{{/each}}\n{{!-- @param rows Row[] --}}"),
        "{\n  rows: Row[];\n}"
    );
}

#[test]
fn param_inside_blocks_is_just_a_comment() {
    assert_eq!(
        context("{{#if a}}{{!-- @param b B --}}{{/if}}"),
        "{\n  a?: unknown;\n}"
    );
}

#[test]
fn param_through_an_array_is_a_diagnostic() {
    let result = run("{{#each items}}{{/each}}{{!-- @param items.x number --}}");
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].code,
        tsbars::DIAGNOSTIC_INVALID_DIRECTIVE
    );
}

#[test]
fn malformed_directives_are_diagnostics() {
    for source in [
        "{{!-- @param --}}",
        "{{!-- @param user --}}",
        "{{!-- @param a..b T --}}",
        "{{!-- @import --}}",
    ] {
        let result = run(source);
        assert_eq!(result.diagnostics.len(), 1, "{source}");
        assert_eq!(
            result.diagnostics[0].code,
            tsbars::DIAGNOSTIC_INVALID_DIRECTIVE,
            "{source}"
        );
    }
}

#[test]
fn other_comments_are_not_directives() {
    let result = run("{{!-- @todo fix --}}{{!-- @importance high --}}");
    assert!(result.diagnostics.is_empty());
}

#[test]
fn imports_are_copied_as_import_lines() {
    let result = run(
        "{{!-- @import type { User } from \"./user\" --}}\n{{!-- @import { z } from 'zod'; --}}\n{{!-- @param user User --}}",
    );
    assert!(result.diagnostics.is_empty());
    assert_eq!(
        result.text,
        "import type { User } from \"./user\";\n\
         import { z } from 'zod';\n\
         export type Context = {\n  user: User;\n};\n\
         declare const template: (context: Context) => string;\n\
         export default template;\n"
    );
}

#[test]
fn parse_errors_fall_back_to_an_any_context() {
    let source = "Hi {{#if a}}unclosed";
    let result = run(source);
    assert_eq!(
        result.text,
        "export type Context = any;\n\
         declare const template: (context: Context) => string;\n\
         export default template;\n"
    );
    assert_eq!(result.diagnostics.len(), 1);
    let diagnostic = &result.diagnostics[0];
    assert_eq!(diagnostic.code, tsbars::DIAGNOSTIC_PARSE_ERROR);
    assert_eq!(
        &source[diagnostic.start..diagnostic.start + diagnostic.length],
        "{{#if a}}"
    );
    assert_eq!(result.mappings.len(), 1);
}
