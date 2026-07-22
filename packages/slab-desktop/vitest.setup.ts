// The jsdom global setup (jest-dom matchers, afterEach cleanup,
// IntersectionObserver / ResizeObserver / matchMedia stubs) now lives in the
// shared `@slab/test-utils` package. Load it for its side effects so the
// desktop project's `setupFiles` entry stays unchanged.
import "@slab/test-utils/setup/jsdom";
