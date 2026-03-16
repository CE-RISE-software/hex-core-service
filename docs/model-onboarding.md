# Model Onboarding

This page explains how to add a new model/version so it becomes available through the service API.

## Artifact References

Each model version may publish any subset of these artifacts:

- `route.json` (required only for routable create/query/dispatch operations)
- `schema.json` (optional, for JSON Schema validation)
- `shacl.ttl` (optional, for SHACL validation)
- `owl.ttl` (optional, for OWL validation)
- `openapi.json` (optional, for model-level API description)

Validation-only models do not need `route.json`.

## Catalog Entry Format

`hex-core-service` reads model definitions from a catalog (URL/file/inline JSON).  
Each entry should point directly to the artifacts it publishes.

Example `catalog.json`:

```json
{
  "models": [
    {
      "model": "re-indicators-specification",
      "version": "0.0.3",
      "route_url": "https://codeberg.org/CE-RISE-models/re-indicators-specification/raw/tag/pages-v0.0.3/generated/route.json",
      "schema_url": "https://codeberg.org/CE-RISE-models/re-indicators-specification/raw/tag/pages-v0.0.3/generated/schema.json",
      "shacl_url": "https://codeberg.org/CE-RISE-models/re-indicators-specification/raw/tag/pages-v0.0.3/generated/shacl.ttl"
    }
  ]
}
```

Rules:

- `model + version` must be unique in the catalog.
- Each entry must declare at least one artifact reference.
- Artifact references must be direct runtime-fetchable file URLs.
- If HTTPS enforcement is enabled, catalog/artifact URLs must be HTTPS.
- Artifact hosts must be permitted by `REGISTRY_ALLOWED_HOSTS`.

## Onboarding Flow

1. Publish the artifact files required by your runtime use case.
2. Add the model/version entry to your catalog source.
3. Ensure the service is configured to load that catalog.
4. Trigger registry refresh:

```bash
curl -X POST http://localhost:8080/admin/registry/refresh \
  -H "Authorization: Bearer <admin-token>"
```

5. Confirm the model/version is now available:

```bash
curl http://localhost:8080/models
```

## Verification Checklist

- `GET /models` returns your model/version.
- `GET /models/{model}/versions/{version}/route` returns `200` only for routable models.
- Optional artifacts (`schema`, `shacl`, `owl`) return `200` if expected.
- `POST /models/{model}/versions/{version}:validate` returns a valid response for a known-good payload.

## Typical Onboarding Errors

- `404 model not found`: catalog not refreshed or wrong `model/version` pair.
- Refresh returns entry errors: invalid artifact URL, unreadable artifact, or blocked host/HTTP policy.
- Validation missing expected checks: corresponding artifact (`schema.json`, `shacl.ttl`, `owl.ttl`) not published.
