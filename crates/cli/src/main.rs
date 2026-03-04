use std::{path::PathBuf, process::ExitCode};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::Serialize;

#[derive(Debug, Parser)]
#[command(name = "hex", about = "CE-RISE Hex Core CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Library mode (no HTTP): run local validators directly.
    Lib {
        #[command(subcommand)]
        command: LibCommand,
    },
    /// Client mode: call the hex-core-service REST API.
    Client {
        #[arg(long, default_value = "http://127.0.0.1:8080")]
        base_url: String,
        #[arg(long)]
        token: Option<String>,
        #[command(subcommand)]
        command: ClientCommand,
    },
}

#[derive(Debug, Subcommand)]
enum LibCommand {
    /// Validate payload against a local JSON Schema file.
    Validate {
        #[arg(long)]
        schema_file: PathBuf,
        #[arg(long)]
        payload_file: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum ClientCommand {
    /// List models from the service.
    Models,
    /// Call :validate endpoint.
    Validate {
        #[arg(long)]
        model: String,
        #[arg(long)]
        version: String,
        #[arg(long)]
        payload_file: PathBuf,
    },
    /// Call :create endpoint.
    Create {
        #[arg(long)]
        model: String,
        #[arg(long)]
        version: String,
        #[arg(long)]
        payload_file: PathBuf,
        #[arg(long)]
        idempotency_key: String,
    },
    /// Call :query endpoint.
    Query {
        #[arg(long)]
        model: String,
        #[arg(long)]
        version: String,
        #[arg(long)]
        filter_file: PathBuf,
    },
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(code) => ExitCode::from(code),
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::from(1)
        }
    }
}

async fn run() -> Result<u8> {
    let cli = Cli::parse();

    match cli.command {
        Command::Lib { command } => run_lib(command).await,
        Command::Client {
            base_url,
            token,
            command,
        } => run_client(base_url, token, command).await,
    }
}

async fn run_lib(command: LibCommand) -> Result<u8> {
    match command {
        LibCommand::Validate {
            schema_file,
            payload_file,
        } => {
            let schema = read_json_file(&schema_file)?;
            let payload = read_json_file(&payload_file)?;
            let result = validate_with_jsonschema(&schema, &payload)?;

            print_validation_result(&result)?;
            if result.passed {
                Ok(0)
            } else {
                Ok(2)
            }
        }
    }
}

async fn run_client(base_url: String, token: Option<String>, command: ClientCommand) -> Result<u8> {
    let client = reqwest::Client::new();
    match command {
        ClientCommand::Models => {
            let url = format!("{}/models", base_url.trim_end_matches('/'));
            let req = with_auth(client.get(url), token.as_deref());
            let resp = req.send().await.context("request failed")?;
            print_json_response(resp).await?;
            Ok(0)
        }
        ClientCommand::Validate {
            model,
            version,
            payload_file,
        } => {
            let payload = read_json_file(&payload_file)?;
            let url = format!(
                "{}/models/{}/versions/{}:validate",
                base_url.trim_end_matches('/'),
                model,
                version
            );
            let req = with_auth(
                client
                    .post(url)
                    .json(&serde_json::json!({ "payload": payload })),
                token.as_deref(),
            );
            let resp = req.send().await.context("request failed")?;
            print_json_response(resp).await?;
            Ok(0)
        }
        ClientCommand::Create {
            model,
            version,
            payload_file,
            idempotency_key,
        } => {
            let payload = read_json_file(&payload_file)?;
            let url = format!(
                "{}/models/{}/versions/{}:create",
                base_url.trim_end_matches('/'),
                model,
                version
            );
            let req = with_auth(
                client
                    .post(url)
                    .header("Idempotency-Key", idempotency_key)
                    .json(&serde_json::json!({ "payload": payload })),
                token.as_deref(),
            );
            let resp = req.send().await.context("request failed")?;
            print_json_response(resp).await?;
            Ok(0)
        }
        ClientCommand::Query {
            model,
            version,
            filter_file,
        } => {
            let filter = read_json_file(&filter_file)?;
            let url = format!(
                "{}/models/{}/versions/{}:query",
                base_url.trim_end_matches('/'),
                model,
                version
            );
            let req = with_auth(
                client
                    .post(url)
                    .json(&serde_json::json!({ "filter": filter })),
                token.as_deref(),
            );
            let resp = req.send().await.context("request failed")?;
            print_json_response(resp).await?;
            Ok(0)
        }
    }
}

fn read_json_file(path: &PathBuf) -> Result<serde_json::Value> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("failed reading {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("invalid JSON in file {}", path.display()))
}

fn with_auth(req: reqwest::RequestBuilder, token: Option<&str>) -> reqwest::RequestBuilder {
    match token {
        Some(t) if !t.is_empty() => req.bearer_auth(t),
        _ => req,
    }
}

async fn print_json_response(resp: reqwest::Response) -> Result<()> {
    let status = resp.status();
    let body = resp.text().await.context("failed reading response body")?;
    let parsed: serde_json::Value =
        serde_json::from_str(&body).with_context(|| format!("response is not JSON ({status})"))?;
    println!("{}", serde_json::to_string_pretty(&parsed)?);
    Ok(())
}

fn print_validation_result(result: &ValidationResult) -> Result<()> {
    let json = serde_json::to_string_pretty(result)?;
    println!("{json}");
    Ok(())
}

fn validate_with_jsonschema(
    schema: &serde_json::Value,
    payload: &serde_json::Value,
) -> Result<ValidationResult> {
    let compiled = jsonschema::JSONSchema::compile(schema)
        .map_err(|err| anyhow::anyhow!("failed compiling JSON Schema: {err}"))?;
    let violations = match compiled.validate(payload) {
        Ok(()) => Vec::new(),
        Err(errors) => errors
            .map(|err| ValidationViolation {
                path: Some(err.instance_path.to_string()),
                message: err.to_string(),
                severity: "error".to_string(),
            })
            .collect(),
    };

    Ok(ValidationResult {
        kind: "jsonschema".to_string(),
        passed: violations.is_empty(),
        violations,
    })
}

#[derive(Debug, Serialize)]
struct ValidationResult {
    kind: String,
    passed: bool,
    violations: Vec<ValidationViolation>,
}

#[derive(Debug, Serialize)]
struct ValidationViolation {
    path: Option<String>,
    message: String,
    severity: String,
}
