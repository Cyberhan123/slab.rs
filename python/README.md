# Python Packages

Python-facing Slab packages live here.

- `slab-python-sdk` provides the Python plugin-author SDK. It contains a
  generated `slab_api_client` package from the `/v1` OpenAPI contract plus
  `slab` runtime bridge typings for Python plugins.

Regenerate the Python API client with `bun run gen:api` from the repo root.
