// Minimal URL implementation for web_stub
// Uses the native V8 URL implementation

// V8 has built-in URL and URLSearchParams
// We just need to export them properly

const URL = globalThis.URL;
const URLSearchParams = globalThis.URLSearchParams;

export { URL, URLSearchParams };
