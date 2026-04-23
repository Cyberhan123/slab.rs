# @slab/plugin-ui

Stable React UI ABI for Slab WebView plugins.

This package intentionally exposes only a safe subset of `@slab/components` plus plugin-scoped global styles. Plugin authors should import `@slab/plugin-ui/globals.css` in their Vite entry and rely on `@slab/plugin-sdk` theme mirroring for host token values.

