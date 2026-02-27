import { useMemo } from "react";

type TauriWindow = Window & {
  __TAURI_INTERNALS__?: unknown;
};

export function isTauri(): boolean {
  return typeof window !== "undefined" && Boolean((window as TauriWindow).__TAURI_INTERNALS__);
}

export default function useIsTauri() {
  return useMemo(() => isTauri(), []);
}
