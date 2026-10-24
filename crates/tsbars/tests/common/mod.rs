#![allow(dead_code)]

use tsbars::{MapOptions, MapResult};

pub fn run(source: &str) -> MapResult {
    run_with_helpers(source, &[])
}

pub fn run_with_helpers(source: &str, helpers: &[&str]) -> MapResult {
    tsbars::map(
        source,
        &MapOptions {
            helpers: helpers.iter().map(|helper| helper.to_string()).collect(),
        },
    )
}

/// The right-hand side of `export type Context = …;`.
pub fn context(source: &str) -> String {
    context_with_helpers(source, &[])
}

pub fn context_with_helpers(source: &str, helpers: &[&str]) -> String {
    let result = run_with_helpers(source, helpers);
    assert!(
        result.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );
    extract_context(&result.text)
}

pub fn extract_context(text: &str) -> String {
    let start = text
        .find("export type Context = ")
        .expect("emitted text has a Context type")
        + "export type Context = ".len();
    let end = text[start..]
        .find(";\ndeclare const template")
        .expect("emitted text declares the template")
        + start;
    text[start..end].to_string()
}
