import { isTauri } from "@/hooks/use-tauri";

/**
 * Pick a workspace folder using the host's native dialog when running inside the
 * Tauri shell, or return `null` in the browser so callers can fall back to a
 * manual path input.
 *
 * This is the single place in the workspace feature that knows about Tauri.
 * Every caller stays browser-first and free of `isTauri` checks — the smoothing
 * between the native dialog and the browser's manual input lives here.
 */
export async function pickFolder(): Promise<string | null> {
  if (!isTauri()) {
    return null;
  }

  const { open } = await import("@tauri-apps/plugin-dialog");
  const selected = await open({ directory: true, multiple: false });
  return typeof selected === "string" ? selected : null;
}
