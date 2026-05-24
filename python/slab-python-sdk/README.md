# slab-python-sdk

Python SDK for Slab plugin authors.

The generated `slab_api_client` package is produced from Slab's OpenAPI
contract. Inside a Python plugin, use the runtime-injected `slab` module to get
a bridge-backed generated client:

```python
import slab
from slab_api_client.api.models import list_models


def run(params):
    client = slab.api.client()
    models = list_models.sync(client=client)
    return {"models": models.to_dict() if models else None}
```

The client does not use direct network access in the plugin runtime. Requests
are routed through `slab.api.request`, so `permissions.slabApi` remains the
authorization boundary.

When packaging a third-party Python plugin, add this package to
`python/requirements.txt` so `slab-plugin-cli` can include it in the `.slabpy`
bundle.
