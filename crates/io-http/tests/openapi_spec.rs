use std::{collections::HashSet, fs, path::PathBuf};

fn openapi_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/io_adapter_openapi.json")
}

fn read_openapi() -> serde_json::Value {
    let text = fs::read_to_string(openapi_path()).expect("read io adapter openapi json");
    serde_json::from_str(&text).expect("parse io adapter openapi json")
}

fn placeholders(path: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_brace = false;
    for ch in path.chars() {
        match ch {
            '{' => {
                in_brace = true;
                cur.clear();
            }
            '}' if in_brace => {
                in_brace = false;
                if !cur.is_empty() {
                    out.push(cur.clone());
                }
            }
            _ if in_brace => cur.push(ch),
            _ => {}
        }
    }
    out
}

fn collect_param_names(params: Option<&serde_json::Value>) -> HashSet<String> {
    let mut names = HashSet::new();
    let Some(arr) = params.and_then(|v| v.as_array()) else {
        return names;
    };
    for p in arr {
        if p.get("in").and_then(|v| v.as_str()) == Some("path") {
            if let Some(name) = p.get("name").and_then(|v| v.as_str()) {
                names.insert(name.to_string());
            }
        }
    }
    names
}

#[test]
fn io_openapi_is_valid_json_object() {
    let doc = read_openapi();
    assert!(doc.is_object());
    assert_eq!(doc.get("openapi").and_then(|v| v.as_str()), Some("3.0.3"));
}

#[test]
fn templated_paths_have_declared_path_parameters() {
    let doc = read_openapi();
    let paths = doc
        .get("paths")
        .and_then(|v| v.as_object())
        .expect("paths object");

    for (path, item) in paths {
        let expected = placeholders(path);
        if expected.is_empty() {
            continue;
        }
        let path_names = collect_param_names(item.get("parameters"));
        for method in [
            "get", "post", "put", "patch", "delete", "options", "head", "trace",
        ] {
            let Some(op) = item.get(method) else {
                continue;
            };
            let mut names = path_names.clone();
            names.extend(collect_param_names(op.get("parameters")));
            for need in &expected {
                assert!(
                    names.contains(need),
                    "path '{path}' operation '{method}' missing path parameter '{need}'"
                );
            }
        }
    }
}

#[test]
fn every_operation_has_responses_object() {
    let doc = read_openapi();
    let paths = doc
        .get("paths")
        .and_then(|v| v.as_object())
        .expect("paths object");

    for (path, item) in paths {
        for method in [
            "get", "post", "put", "patch", "delete", "options", "head", "trace",
        ] {
            let Some(op) = item.get(method) else {
                continue;
            };
            let responses = op.get("responses").and_then(|v| v.as_object());
            assert!(
                responses.is_some_and(|r| !r.is_empty()),
                "path '{path}' operation '{method}' must define non-empty responses"
            );
        }
    }
}
