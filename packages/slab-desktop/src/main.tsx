import { waitForApiServer } from "@slab/api";

import { assembleDesktopPlatform } from "./platform/desktop-ports";
import "@slab/i18n";

// Install the Tauri platform adapters into @slab/core's seams before any
// harness/image code runs.
assembleDesktopPlatform();

/**
 * Desktop shell entry.
 *
 * slab-server is spawned as a sidecar right before this webview opens, and
 * its HTTP listener takes a few seconds to bind. Booting the app immediately
 * races the first request burst (ui-state hydration, boot queries) against
 * the listener, which surfaces "unable to load UI preferences" toasts on
 * every cold start. So: gate bootstrap on the `/health` probe, then
 * dynamically import the app — the import boundary is what actually defers
 * the store modules, since zustand's `persist` hydrates at import time.
 */
async function boot(): Promise<void> {
  // Bounded wait; on timeout we boot anyway so behavior degrades to the
  // pre-gate state (failed requests + toasts) instead of a dead window.
  const ready = await waitForApiServer();
  if (!ready) {
    console.warn("[slab-desktop] API server did not become ready; booting anyway.");
  }
  const { renderApp } = await import("./boot");
  renderApp();
}

void boot();
