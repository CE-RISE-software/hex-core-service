use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn lib_validate_returns_zero_for_valid_payload() {
    let dir = tempdir().expect("tempdir");
    let schema_path = dir.path().join("schema.json");
    let payload_path = dir.path().join("payload.json");

    fs::write(
        &schema_path,
        r#"{"type":"object","required":["id"],"properties":{"id":{"type":"string"}}}"#,
    )
    .expect("write schema");
    fs::write(&payload_path, r#"{"id":"ok"}"#).expect("write payload");

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_hex"));
    cmd.args([
        "lib",
        "validate",
        "--schema-file",
        schema_path.to_str().expect("utf8 path"),
        "--payload-file",
        payload_path.to_str().expect("utf8 path"),
    ]);
    cmd.assert().success();
}

#[test]
fn root_help_is_available() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_hex"));
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("CE-RISE Hex Core CLI"));
}

#[test]
fn client_create_requires_idempotency_key() {
    let dir = tempdir().expect("tempdir");
    let payload_path = dir.path().join("payload.json");
    fs::write(&payload_path, r#"{"id":"ok"}"#).expect("write payload");

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_hex"));
    cmd.args([
        "client",
        "--base-url",
        "http://127.0.0.1:9",
        "create",
        "--model",
        "m",
        "--version",
        "1",
        "--payload-file",
        payload_path.to_str().expect("utf8 path"),
    ]);
    cmd.assert().code(2);
}

#[test]
fn client_validate_requires_payload_file() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_hex"));
    cmd.args([
        "client",
        "--base-url",
        "http://127.0.0.1:9",
        "validate",
        "--model",
        "m",
        "--version",
        "1",
    ]);
    cmd.assert().code(2);
}

#[test]
fn lib_validate_returns_two_for_invalid_payload() {
    let dir = tempdir().expect("tempdir");
    let schema_path = dir.path().join("schema.json");
    let payload_path = dir.path().join("payload.json");

    fs::write(
        &schema_path,
        r#"{"type":"object","required":["id"],"properties":{"id":{"type":"string"}}}"#,
    )
    .expect("write schema");
    fs::write(&payload_path, r#"{}"#).expect("write payload");

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_hex"));
    cmd.args([
        "lib",
        "validate",
        "--schema-file",
        schema_path.to_str().expect("utf8 path"),
        "--payload-file",
        payload_path.to_str().expect("utf8 path"),
    ]);
    cmd.assert().code(2);
}

#[test]
fn lib_validate_returns_one_for_invalid_schema_json() {
    let dir = tempdir().expect("tempdir");
    let schema_path = dir.path().join("schema.json");
    let payload_path = dir.path().join("payload.json");

    fs::write(&schema_path, r#"{"type":"object""#).expect("write broken schema");
    fs::write(&payload_path, r#"{"id":"ok"}"#).expect("write payload");

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_hex"));
    cmd.args([
        "lib",
        "validate",
        "--schema-file",
        schema_path.to_str().expect("utf8 path"),
        "--payload-file",
        payload_path.to_str().expect("utf8 path"),
    ]);
    cmd.assert()
        .code(1)
        .stderr(predicate::str::contains("invalid JSON in file"));
}

#[test]
fn client_models_returns_one_for_invalid_base_url() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_hex"));
    cmd.args(["client", "--base-url", "://bad-url", "models"]);
    cmd.assert()
        .code(1)
        .stderr(predicate::str::contains("request failed"));
}

#[test]
fn client_create_returns_one_when_payload_file_is_missing() {
    let dir = tempdir().expect("tempdir");
    let payload_path = dir.path().join("payload.json");
    // Do not create payload file on purpose.

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_hex"));
    cmd.args([
        "client",
        "--base-url",
        "http://127.0.0.1:9",
        "create",
        "--model",
        "m",
        "--version",
        "1",
        "--payload-file",
        payload_path.to_str().expect("utf8 path"),
        "--idempotency-key",
        "idem-123",
    ]);
    cmd.assert()
        .code(1)
        .stderr(predicate::str::contains("failed reading"));
}
