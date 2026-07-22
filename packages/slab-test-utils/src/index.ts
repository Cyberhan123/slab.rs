/**
 * Public barrel for `@slab/test-utils`.
 *
 * Exports the React-free utilities (mock factories, monaco stubs, fixtures).
 * `renderWithProviders` is intentionally NOT re-exported here — it pulls React,
 * and non-React consumers (e.g. node-environment i18n/api tests) should not
 * transitively load it. Import it via the dedicated subpath:
 *
 *   import { renderWithProviders } from '@slab/test-utils/providers/render-with-providers';
 *
 * The jsdom global setup (`./setup/jsdom`) is also intentionally not
 * re-exported — it has load-time side effects and belongs only in a project's
 * `setupFiles`, imported via the `@slab/test-utils/setup/jsdom` subpath.
 */
export * from './mocks';
export * from './fixtures';
