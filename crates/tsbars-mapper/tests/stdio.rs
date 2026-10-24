use std::io::{BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

struct Client {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

impl Client {
    fn spawn() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_tsbars-mapper"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("spawn tsbars-mapper");
        let stdin = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
        Self {
            child,
            stdin,
            stdout,
        }
    }

    fn send_raw(&mut self, body: &str) {
        let stdin = self.stdin.as_mut().expect("stdin open");
        write!(stdin, "Content-Length: {}\r\n\r\n{body}", body.len()).unwrap();
        stdin.flush().unwrap();
    }

    fn read(&mut self) -> Value {
        let body = frames::read_frame(&mut self.stdout)
            .unwrap()
            .expect("response before EOF");
        serde_json::from_slice(&body).unwrap()
    }

    fn call(&mut self, id: Value, method: &str, params: Value) -> Value {
        let request = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        self.send_raw(&request.to_string());
        self.read()
    }

    fn finish(mut self) -> std::process::ExitStatus {
        drop(self.stdin.take());
        self.child.wait().unwrap()
    }
}

/// The binary crate has no lib target, so the test reads frames with its own small parser.
mod frames {
    use std::io::{self, BufRead};

    pub fn read_frame(reader: &mut impl BufRead) -> io::Result<Option<Vec<u8>>> {
        let mut length = None;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line)? == 0 {
                return Ok(None);
            }
            let line = line.trim_end();
            if line.is_empty() {
                break;
            }
            if let Some(value) = line.strip_prefix("Content-Length:") {
                length = Some(value.trim().parse::<usize>().unwrap());
            }
        }
        let mut body = vec![0; length.expect("Content-Length header")];
        reader.read_exact(&mut body)?;
        Ok(Some(body))
    }
}

#[test]
fn full_session() {
    let mut client = Client::spawn();

    let init = client.call(
        json!("1"),
        "initialize",
        json!({ "positionEncodings": ["utf-8", "utf-16"] }),
    );
    assert_eq!(init["id"], "1");
    assert_eq!(init["result"]["protocolVersion"], 1);
    assert_eq!(init["result"]["positionEncoding"], "utf-8");
    assert_eq!(init["result"]["diagnosticSource"], "tsbars");

    let open = client.call(
        json!("2"),
        "openProject",
        json!({
            "configFileName": "/p/tsconfig.json",
            "projectHandle": "h1",
            "options": { "helpers": ["now"] },
            "compilerOptions": {},
        }),
    );
    assert_eq!(open["result"], json!({}));

    let source = "Hi {{name}} {{now}}";
    let transform = client.call(
        json!(3),
        "transform",
        json!({ "fileName": "/p/a.hbs", "content": source, "projectHandle": "h1" }),
    );
    assert_eq!(transform["id"], 3);
    let result = &transform["result"];
    assert_eq!(result["extension"], ".ts");
    let text = result["text"].as_str().unwrap();
    assert!(text.contains("name: Printable"), "{text}");
    assert!(
        !text.contains("now"),
        "helper must not become a property: {text}"
    );
    assert_eq!(result["diagnostics"], json!([]));
    let mappings = result["mappings"].as_array().unwrap();
    assert!(!mappings.is_empty());
    for mapping in mappings {
        let tuple = mapping.as_array().unwrap();
        assert!(tuple.len() == 5 || tuple.len() == 6, "{tuple:?}");
    }

    let closed = client.call(json!("4"), "closeProject", json!({ "projectHandle": "h1" }));
    assert_eq!(closed["result"], json!({}));

    // Closed handle behaves like default options: `now` is a context property again.
    let reopened = client.call(
        json!("5"),
        "transform",
        json!({ "fileName": "/p/a.hbs", "content": source, "projectHandle": "h1" }),
    );
    assert!(
        reopened["result"]["text"]
            .as_str()
            .unwrap()
            .contains("now: Printable")
    );

    assert!(client.finish().success());
}

#[test]
fn bad_template_yields_diagnostics_not_an_error() {
    let mut client = Client::spawn();
    let response = client.call(
        json!("1"),
        "transform",
        json!({ "fileName": "/p/a.hbs", "content": "{{#if a}}oops", "projectHandle": "nope" }),
    );
    assert!(response.get("error").is_none(), "{response}");
    let diagnostics = response["result"]["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["code"], tsbars::DIAGNOSTIC_PARSE_ERROR);
    client.finish();
}

#[test]
fn malformed_helpers_reports_option_diagnostics() {
    let mut client = Client::spawn();
    let response = client.call(
        json!("1"),
        "openProject",
        json!({ "projectHandle": "h", "options": { "helpers": "now" }, "compilerOptions": {} }),
    );
    let diagnostics = response["result"]["optionDiagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["path"], json!(["helpers"]));
    assert!(diagnostics[0]["messageText"].is_string());
    client.finish();
}

#[test]
fn unknown_method_and_invalid_params() {
    let mut client = Client::spawn();
    let unknown = client.call(json!("a"), "nope", json!({}));
    assert_eq!(unknown["error"]["code"], -32601);
    assert_eq!(unknown["id"], "a");

    let invalid = client.call(json!(7), "transform", json!({ "fileName": "x" }));
    assert_eq!(invalid["error"]["code"], -32602);
    assert_eq!(invalid["id"], 7);

    client.send_raw("not json");
    let parse = client.read();
    assert_eq!(parse["error"]["code"], -32700);
    assert!(parse["id"].is_null());
    client.finish();
}
