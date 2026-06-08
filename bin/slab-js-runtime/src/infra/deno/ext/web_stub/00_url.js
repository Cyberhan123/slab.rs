(function () {
// Minimal URL implementation for web_stub.
// Uses native globals when the embedder provides them.

const { URL, URLSearchParams } = globalThis;

return { URL, URLSearchParams };
})();
