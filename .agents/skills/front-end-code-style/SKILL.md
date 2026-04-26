---
name: front-end-code-style
description: Use when working on front-end including any JavaScript, TypeScript, HTML, or CSS code in the repo to ensure consistent style and formatting.
---

- When call slab-server API use api.useQuery, api.useMutation, or any other hooks provided by the api package instead of calling fetch or other HTTP clients directly.

- Do not create small helper methods that are referenced only once.

- use `bun run gen:schemas` to generate the schemas.

- use `bun run gen:api` to generate the API types. Instead of Change it manually.

- When work finished on the front-end code, make sure to run `bun run lint:fix` to automatically fix any linting issues and maintain consistent code style across the codebase.

- use existing hooks or packages instead of custom implementations when possible.