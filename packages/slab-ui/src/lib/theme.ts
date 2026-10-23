import { useSyncExternalStore } from "react";

/**
 * App theme controller: light / dark / system, persisted as a bare string in
 * localStorage ("slab-theme") so the shells' pre-paint boot scripts (both
 * index.html files) can read it without JSON parsing. Hand-rolled on purpose —
 * next-themes is only consumed inside the sonner toaster and has no provider
 * mounted, and a Vite SPA gets no pre-paint application from it anyway.
 *
 * The resolved theme lands as `html.dark` + `html.style.colorScheme`.
 * PluginThemeSync (App.tsx) already MutationObserves documentElement and
 * republishes the snapshot to plugin webviews, so toggling here propagates
 * there for free.
 */

export type ThemePreference = "light" | "dark" | "system";
export type ResolvedTheme = "light" | "dark";

export const THEME_STORAGE_KEY = "slab-theme";

const SYSTEM_MEDIA_QUERY = "(prefers-color-scheme: dark)";

export function isThemePreference(value: unknown): value is ThemePreference {
  return value === "light" || value === "dark" || value === "system";
}

/** Reads the raw stored value (boot-time source; the store caches after first read). */
export function getStoredPreference(): ThemePreference {
  if (typeof window === "undefined") {
    return "system";
  }
  try {
    const stored = window.localStorage.getItem(THEME_STORAGE_KEY);
    return isThemePreference(stored) ? stored : "system";
  } catch {
    // Private-mode storage access throws; fall through to the default.
    return "system";
  }
}

function systemPrefersDark(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia(SYSTEM_MEDIA_QUERY).matches
  );
}

export function resolveTheme(preference: ThemePreference): ResolvedTheme {
  if (preference === "system") {
    return systemPrefersDark() ? "dark" : "light";
  }
  return preference;
}

function applyResolvedTheme(resolved: ResolvedTheme): void {
  if (typeof document === "undefined") {
    return;
  }
  document.documentElement.classList.toggle("dark", resolved === "dark");
  document.documentElement.style.colorScheme = resolved;
}

/** External store — one listener set, two snapshot shapes. */
const listeners = new Set<() => void>();

let currentPreference: ThemePreference | null = null;
let currentResolved: ResolvedTheme | null = null;

function emit(): void {
  for (const listener of listeners) {
    listener();
  }
}

function ensureSnapshot(): void {
  if (currentPreference === null) {
    currentPreference = getStoredPreference();
  }
  if (currentResolved === null) {
    currentResolved = resolveTheme(currentPreference);
  }
}

function notify(): void {
  ensureSnapshot();
  emit();
}

let systemListenerAttached = false;

/** Follows OS appearance changes while the preference is "system". */
export function ensureSystemThemeListener(): void {
  if (systemListenerAttached || typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return;
  }
  systemListenerAttached = true;
  window.matchMedia(SYSTEM_MEDIA_QUERY).addEventListener("change", () => {
    if (getThemePreference() !== "system") {
      return;
    }
    currentResolved = resolveTheme("system");
    applyResolvedTheme(currentResolved);
    emit();
  });
}

export function getThemePreference(): ThemePreference {
  ensureSnapshot();
  return currentPreference as ThemePreference;
}

export function getResolvedTheme(): ResolvedTheme {
  ensureSnapshot();
  return currentResolved as ResolvedTheme;
}

/** Applies the stored preference to the document (idempotent with the boot script). */
export function applyThemePreference(preference: ThemePreference): void {
  currentPreference = preference;
  currentResolved = resolveTheme(preference);
  applyResolvedTheme(currentResolved);
  ensureSystemThemeListener();
}

/** Persists + applies + notifies subscribers. */
export function setThemePreference(preference: ThemePreference): void {
  if (typeof window === "undefined") {
    return;
  }
  try {
    window.localStorage.setItem(THEME_STORAGE_KEY, preference);
  } catch {
    // Storage may be unavailable; still apply for this session.
  }
  applyThemePreference(preference);
  notify();
}

function subscribeTheme(listener: () => void): () => void {
  listeners.add(listener);
  ensureSystemThemeListener();
  return () => {
    listeners.delete(listener);
  };
}

export function useThemePreference(): ThemePreference {
  return useSyncExternalStore(
    subscribeTheme,
    getThemePreference,
    () => "system" satisfies ThemePreference,
  );
}

export function useResolvedTheme(): ResolvedTheme {
  return useSyncExternalStore(
    subscribeTheme,
    getResolvedTheme,
    () => "light" satisfies ResolvedTheme,
  );
}
