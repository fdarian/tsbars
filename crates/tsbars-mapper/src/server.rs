use std::collections::HashMap;

use serde::Deserialize;
use serde_json::{Value, json};
use tsbars::{MapOptions, MapResult, features};

use crate::rpc::{INVALID_PARAMS, METHOD_NOT_FOUND, RpcError};

const PROTOCOL_VERSION: u32 = 1;
const DIAGNOSTIC_SOURCE: &str = "tsbars";
const VIRTUAL_EXTENSION: &str = ".ts";

#[derive(Default)]
pub struct Server {
    projects: HashMap<String, MapOptions>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenProjectParams {
    project_handle: Value,
    #[serde(default)]
    options: Option<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TransformParams {
    content: String,
    #[serde(default)]
    project_handle: Option<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CloseProjectParams {
    project_handle: Value,
}

fn parse_params<T: for<'de> Deserialize<'de>>(params: Value) -> Result<T, RpcError> {
    serde_json::from_value(params)
        .map_err(|error| RpcError::new(INVALID_PARAMS, format!("invalid params: {error}")))
}

fn handle_key(handle: &Value) -> String {
    handle.to_string()
}

fn option_diagnostic(path: Vec<Value>, message: &str) -> Value {
    json!({ "path": path, "messageText": message })
}

fn parse_helpers(options: &Value) -> (MapOptions, Vec<Value>) {
    let mut map_options = MapOptions::default();
    let mut diagnostics = Vec::new();

    let Some(object) = options.as_object() else {
        diagnostics.push(option_diagnostic(
            vec![],
            "tsbars: contentMappers options must be an object",
        ));
        return (map_options, diagnostics);
    };
    let Some(helpers) = object.get("helpers") else {
        return (map_options, diagnostics);
    };
    let Some(items) = helpers.as_array() else {
        diagnostics.push(option_diagnostic(
            vec![json!("helpers")],
            "tsbars: \"helpers\" must be an array of strings",
        ));
        return (map_options, diagnostics);
    };
    for (index, item) in items.iter().enumerate() {
        match item.as_str() {
            Some(name) => map_options.helpers.push(name.to_owned()),
            None => diagnostics.push(option_diagnostic(
                vec![json!("helpers"), json!(index)],
                "tsbars: helper names must be strings",
            )),
        }
    }
    (map_options, diagnostics)
}

fn mapping_tuple(mapping: &tsbars::Mapping) -> Value {
    let mut tuple = vec![
        json!(mapping.virtual_start),
        json!(mapping.virtual_len),
        json!(mapping.original_start),
        json!(mapping.original_len),
        json!(mapping.kind as u8),
    ];
    if mapping.features != features::ALL {
        tuple.push(json!(mapping.features));
    }
    Value::Array(tuple)
}

fn transform_result(result: &MapResult) -> Value {
    let mappings: Vec<Value> = result.mappings.iter().map(mapping_tuple).collect();
    let diagnostics: Vec<Value> = result
        .diagnostics
        .iter()
        .map(|diagnostic| {
            json!({
                "messageText": diagnostic.message,
                "start": diagnostic.start,
                "length": diagnostic.length,
                "code": diagnostic.code,
            })
        })
        .collect();
    json!({
        "text": result.text,
        "extension": VIRTUAL_EXTENSION,
        "mappings": mappings,
        "diagnostics": diagnostics,
    })
}

impl Server {
    pub fn handle(&mut self, method: &str, params: Value) -> Result<Value, RpcError> {
        match method {
            "initialize" => Ok(json!({
                "protocolVersion": PROTOCOL_VERSION,
                "positionEncoding": "utf-8",
                "diagnosticSource": DIAGNOSTIC_SOURCE,
            })),
            "openProject" => self.open_project(parse_params(params)?),
            "transform" => self.transform(parse_params(params)?),
            "closeProject" => {
                let params: CloseProjectParams = parse_params(params)?;
                self.projects.remove(&handle_key(&params.project_handle));
                Ok(json!({}))
            }
            _ => Err(RpcError::new(
                METHOD_NOT_FOUND,
                format!("method not found: {method}"),
            )),
        }
    }

    fn open_project(&mut self, params: OpenProjectParams) -> Result<Value, RpcError> {
        let (map_options, diagnostics) = match &params.options {
            Some(options) => parse_helpers(options),
            None => (MapOptions::default(), Vec::new()),
        };
        self.projects
            .insert(handle_key(&params.project_handle), map_options);
        if diagnostics.is_empty() {
            Ok(json!({}))
        } else {
            Ok(json!({ "optionDiagnostics": diagnostics }))
        }
    }

    fn transform(&self, params: TransformParams) -> Result<Value, RpcError> {
        let default_options = MapOptions::default();
        let options = params
            .project_handle
            .as_ref()
            .and_then(|handle| self.projects.get(&handle_key(handle)))
            .unwrap_or(&default_options);
        Ok(transform_result(&tsbars::map(&params.content, options)))
    }
}
