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
            let url = models_url(&base_url);
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
            let url = validate_url(&base_url, &model, &version);
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
            let url = create_url(&base_url, &model, &version);
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
            let url = query_url(&base_url, &model, &version);
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

fn models_url(base_url: &str) -> String {
    format!("{}/models", base_url.trim_end_matches('/'))
}

fn validate_url(base_url: &str, model: &str, version: &str) -> String {
    format!(
        "{}/models/{}/versions/{}:validate",
        base_url.trim_end_matches('/'),
        model,
        version
    )
}

fn create_url(base_url: &str, model: &str, version: &str) -> String {
    format!(
        "{}/models/{}/versions/{}:create",
        base_url.trim_end_matches('/'),
        model,
        version
    )
}

fn query_url(base_url: &str, model: &str, version: &str) -> String {
    format!(
        "{}/models/{}/versions/{}:query",
        base_url.trim_end_matches('/'),
        model,
        version
    )
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

#[cfg(test)]
mod tests {
    use super::{create_url, models_url, query_url, validate_url, with_auth};

    #[test]
    fn endpoint_url_builders_trim_trailing_slashes() {
        let base = "http://example.test/";
        assert_eq!(models_url(base), "http://example.test/models");
        assert_eq!(
            validate_url(base, "m", "1"),
            "http://example.test/models/m/versions/1:validate"
        );
        assert_eq!(
            create_url(base, "m", "1"),
            "http://example.test/models/m/versions/1:create"
        );
        assert_eq!(
            query_url(base, "m", "1"),
            "http://example.test/models/m/versions/1:query"
        );
    }

    #[test]
    fn with_auth_sets_bearer_authorization_header() {
        let client = reqwest::Client::new();
        let req = with_auth(client.get("http://example.test/models"), Some("token-123"))
            .build()
            .expect("build request");
        let auth = req
            .headers()
            .get("authorization")
            .expect("authorization header")
            .to_str()
            .expect("header string");
        assert_eq!(auth, "Bearer token-123");
    }

    #[test]
    fn with_auth_omits_authorization_when_token_is_missing_or_empty() {
        let client = reqwest::Client::new();
        let no_token = with_auth(client.get("http://example.test/models"), None)
            .build()
            .expect("build request");
        assert!(!no_token.headers().contains_key("authorization"));

        let empty_token = with_auth(client.get("http://example.test/models"), Some(""))
            .build()
            .expect("build request");
        assert!(!empty_token.headers().contains_key("authorization"));
    }
}
