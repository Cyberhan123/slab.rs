/**
 * Public barrel for `@slab/test-utils`.
 *
 * Exports the React-free utilities (mock factories, monaco stubs, fixtures).
 * `renderWithProviders` is intentionally NOT re-exported here — it pulls React,
 * and non-React consumers (e.g. node-environment i18n/api tests) should not
 * transitively load it. Import it via the dedicated subpath:
 *
 *   import { renderWithProviders } from '@slab/test-utils/providers/render-with-providers';
 */
export * from './mocks';
export * from './fixtures';
