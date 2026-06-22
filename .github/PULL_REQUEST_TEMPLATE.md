## Validation

- [ ] I inspected the current code path and kept the change scoped to this PR.
- [ ] Contract drift: if backend routes, OpenAPI schemas, or shared API types changed, I ran `bun run gen:api` and committed both `packages/api/src/v1.d.ts` and Python client updates.
- [ ] Settings/schema drift: if settings, manifests, or generated schemas changed, I ran `bun run gen:schemas` and committed generated manifest/schema files.
- [ ] Frontend/i18n: new user-visible text uses i18n keys and both `en-US` / `zh-CN` locales were updated.
- [ ] Browser coverage: relevant browser regression coverage was added or updated under `bun run test:browser`.
- [ ] Fullstack coverage: release-risk flows were covered by `bun run test:e2e` or marked for release/manual validation with a reason.
- [ ] Bundle budget: desktop changes that can affect bundle size were checked with `bun run check:bundle-budget` after `bun run build:desktop`.
- [ ] Rollback: Plan F guardrail flags remain default-enabled, and any rollback behavior is documented.

## Notes

Link the plan/audit item, describe any manual platform/a11y checks, and call out intentionally deferred work.
