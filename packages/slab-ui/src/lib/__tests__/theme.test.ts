import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  applyThemePreference,
  getResolvedTheme,
  getStoredPreference,
  getThemePreference,
  resolveTheme,
  setThemePreference,
  THEME_STORAGE_KEY,
} from "../theme";

/** The controller caches snapshots at module level; tests drive it back to a
 *  known state through the public API instead of resetting modules. */
function makeMediaQuery(matches: boolean) {
  return {
    matches,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
  };
}

describe("theme controller", () => {
  const originalMatchMedia = window.matchMedia;

  beforeEach(() => {
    window.localStorage.clear();
    document.documentElement.classList.remove("dark");
    document.documentElement.style.colorScheme = "";
    setThemePreference("system");
  });

  afterEach(() => {
    window.matchMedia = originalMatchMedia;
    vi.restoreAllMocks();
  });

  it("falls back to system for missing or invalid storage values", () => {
    expect(getStoredPreference()).toBe("system");
    window.localStorage.setItem(THEME_STORAGE_KEY, "neon");
    expect(getStoredPreference()).toBe("system");
    window.localStorage.setItem(THEME_STORAGE_KEY, "dark");
    expect(getStoredPreference()).toBe("dark");
  });

  it("resolves system through matchMedia", () => {
    window.matchMedia = vi.fn().mockReturnValue(makeMediaQuery(true)) as unknown as typeof window.matchMedia;
    expect(resolveTheme("system")).toBe("dark");
    window.matchMedia = vi.fn().mockReturnValue(makeMediaQuery(false)) as unknown as typeof window.matchMedia;
    expect(resolveTheme("system")).toBe("light");
    expect(resolveTheme("dark")).toBe("dark");
    expect(resolveTheme("light")).toBe("light");
  });

  it("setThemePreference persists, applies the html class, and updates snapshots", () => {
    setThemePreference("dark");
    expect(window.localStorage.getItem(THEME_STORAGE_KEY)).toBe("dark");
    expect(document.documentElement.classList.contains("dark")).toBe(true);
    expect(document.documentElement.style.colorScheme).toBe("dark");
    expect(getThemePreference()).toBe("dark");
    expect(getResolvedTheme()).toBe("dark");

    setThemePreference("light");
    expect(document.documentElement.classList.contains("dark")).toBe(false);
    expect(document.documentElement.style.colorScheme).toBe("light");
  });

  it("applyThemePreference re-applies without touching storage", () => {
    window.localStorage.setItem(THEME_STORAGE_KEY, "dark");
    applyThemePreference("light");
    expect(window.localStorage.getItem(THEME_STORAGE_KEY)).toBe("dark");
    expect(document.documentElement.classList.contains("dark")).toBe(false);
    expect(getThemePreference()).toBe("light");
  });
});
