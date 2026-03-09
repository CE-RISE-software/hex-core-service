# Model Onboarding

This page explains how to add a new model/version so it becomes available through the service API.

## Required Artifact Files

Each model version must expose a generated artifact directory containing:

- `route.json` (required)
- `schema.json` (optional, for JSON Schema validation)
- `shacl.ttl` (optional, for SHACL validation)
- `owl.ttl` (optional, for OWL validation)

At minimum, `route.json` must exist for the model/version to be routable.

## Catalog Entry Format

`hex-core-service` reads model definitions from a catalog (URL/file/inline JSON).  
Each entry points to the base URL of one model/version artifact directory.

Example `catalog.json`:

```json
{
  "models": [
    {
      "model": "re-indicators-specification",
      "version": "0.0.3",
      "base_url": "https://codeberg.org/CE-RISE-models/re-indicators-specification/src/tag/pages-v0.0.3/generated/"
    }
  ]
}
```

Rules:

- `model + version` must be unique in the catalog.
- `base_url` must resolve to the generated artifact folder.
- If HTTPS enforcement is enabled, catalog/artifact URLs must be HTTPS.

## Onboarding Flow

1. Publish model artifacts (`route.json` required) for the new model/version.
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
- `GET /models/{model}/versions/{version}/route` returns `200`.
- Optional artifacts (`schema`, `shacl`, `owl`) return `200` if expected.
- `POST /models/{model}/versions/{version}:validate` returns a valid response for a known-good payload.

## Typical Onboarding Errors

- `404 model not found`: catalog not refreshed or wrong `model/version` pair.
- Refresh returns entry errors: invalid `base_url`, missing `route.json`, or blocked host/HTTP policy.
- Validation missing expected checks: corresponding artifact (`schema.json`, `shacl.ttl`, `owl.ttl`) not published.
