import type { ReactNode } from "react";

/**
 * Mobile chrome: safe-area padding, full-height column, no sidebar — pages
 * render directly like a native app view.
 */
export function MobileShell({ children }: { children: ReactNode }) {
  return (
    <div
      className="flex min-h-[100dvh] flex-col bg-background text-foreground"
      style={{
        paddingTop: "env(safe-area-inset-top)",
        paddingBottom: "env(safe-area-inset-bottom)",
        paddingLeft: "env(safe-area-inset-left)",
        paddingRight: "env(safe-area-inset-right)",
      }}
    >
      {children}
    </div>
  );
}
