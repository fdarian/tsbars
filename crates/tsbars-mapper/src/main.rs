mod rpc;
mod server;

use std::io::{self, BufReader};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::process::ExitCode;

use rpc::{INTERNAL_ERROR, PARSE_ERROR, Request, Response, RpcError};
use serde_json::Value;
use server::Server;

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_owned()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "panic".to_owned()
    }
}

fn respond(server: &mut Server, body: &[u8]) -> Option<Response> {
    let request: Request = match serde_json::from_slice(body) {
        Ok(request) => request,
        Err(error) => {
            return Some(Response::error(
                Value::Null,
                RpcError::new(PARSE_ERROR, format!("invalid request: {error}")),
            ));
        }
    };
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        server.handle(&request.method, request.params)
    }));
    let outcome = outcome.unwrap_or_else(|payload| {
        let message = panic_message(payload.as_ref());
        eprintln!("tsbars-mapper: panic in {}: {message}", request.method);
        Err(RpcError::new(
            INTERNAL_ERROR,
            format!("internal error: {message}"),
        ))
    });
    let id = request.id?;
    Some(match outcome {
        Ok(result) => Response::result(id, result),
        Err(error) => Response::error(id, error),
    })
}

fn run() -> io::Result<()> {
    let mut reader = BufReader::new(io::stdin().lock());
    let mut writer = io::stdout().lock();
    let mut server = Server::default();
    while let Some(body) = rpc::read_frame(&mut reader)? {
        if let Some(response) = respond(&mut server, &body) {
            rpc::write_response(&mut writer, &response)?;
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("tsbars-mapper: {error}");
            ExitCode::FAILURE
        }
    }
}
