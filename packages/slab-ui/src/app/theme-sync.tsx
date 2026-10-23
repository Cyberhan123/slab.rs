import { useEffect } from "react";

import { applyThemePreference, ensureSystemThemeListener, getThemePreference } from "@slab/ui/lib/theme";

/**
 * Applies the persisted theme preference once the app mounts — belt-and-braces
 * with the shells' pre-paint boot scripts (they run before the bundle loads;
 * this one covers environments that never ran it, like tests and embeds).
 * Later toggles flow through setThemePreference in lib/theme.ts directly.
 */
export function ThemeSync() {
  useEffect(() => {
    applyThemePreference(getThemePreference());
    ensureSystemThemeListener();
  }, []);

  return null;
}
