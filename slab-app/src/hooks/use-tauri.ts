import { useState } from "react";

export default function useIsTauri() {
    const [isTauri, _setIsTauri] = useState(() => {
        return !!(window as any).__TAURI_INTERNALS__;
    });
    return isTauri;
}