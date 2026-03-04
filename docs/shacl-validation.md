# SHACL Validation

This page describes how SHACL validation is currently executed in `hex-core-service`.

## Scope and Current Behavior

SHACL is the preferred validation path for model payloads, but the current implementation is profile-based.

- Adapter crate: `crates/validator-shacl`
- Core contract: `ValidatorPort`
- Runtime result kind: `ValidationKind::Shacl`

At this stage, the SHACL adapter validates payload constraints aligned with the dp-record-metadata profile. It does not yet execute a full generic SHACL engine over arbitrary RDF graphs.

## When SHACL Runs

For `POST /models/{model}/versions/{version}:validate`:

1. Core resolves artifacts for `(model, version)` from the registry.
2. Core executes configured validators.
3. SHACL validation runs when a `shacl.ttl` artifact is available in the resolved `ArtifactSet`.

If `shacl.ttl` is absent, SHACL is skipped by orchestration and does not block other validator results.

## Supported Checks (Current Adapter)

The current SHACL adapter enforces:

- `record_scope` must be one of `product` or `material`
- `related_passports[*].relation_type` must be one of:
  - `derived_from`
  - `contributes_to`
  - `split_from`
  - `merged_into`
  - `recycled_into`
  - `manufactured_from`
- `metadata_versioning.metadata_created` must be RFC3339 timestamp
- `metadata_versioning.metadata_modified` must be RFC3339 timestamp
- `applied_schemas[*].composition_info.sequence_order` must be integer
- `applied_schemas[*].schema_usage.completeness_percentage` must be numeric
- `applied_schemas[*]` is treated as closed shape for keys:
  - `schema_reference`
  - `schema_usage`
  - `composition_info`

Violations are returned as `severity = error`, with a JSON path and message.

## Registry Requirements

To enable SHACL validation for a model version:

- Include a catalog entry for that `(model, version)`.
- Ensure `base_url` points to an artifact folder containing:
  - required: `route.json`
  - optional but needed for SHACL: `shacl.ttl`

Example CE-RISE artifact base URL:

`https://codeberg.org/CE-RISE-models/<model>/src/tag/pages-v<version>/generated/`

## API Result Shape

Validation responses merge all validator outputs:

```json
{
  "passed": false,
  "results": [
    {
      "kind": "shacl",
      "passed": false,
      "violations": [
        {
          "path": "$.record_scope",
          "message": "record_scope must be one of: product, material",
          "severity": "error"
        }
      ]
    }
  ]
}
```

## Refresh and Operations

- Update model artifacts or catalog source.
- Trigger `POST /admin/registry/refresh`.
- New SHACL artifacts become active after successful atomic index swap.

## Known Limitations

- Not a full RDF/SHACL graph validation engine yet.
- Focused on currently supported SHACL profile checks.
- OWL validation is tracked separately (Phase 6b).
