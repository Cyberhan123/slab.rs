import { describe, expect, it } from "vitest";

import { applySlabThemeToDocument, type SlabThemeSnapshot } from "../src/index";

describe("applySlabThemeToDocument", () => {
  it("writes mode class and mirrored CSS variables", () => {
    const document = window.document.implementation.createHTMLDocument("plugin");
    const snapshot: SlabThemeSnapshot = {
      mode: "dark",
      tokens: {
        background: "oklch(20% 0.01 240)",
        foreground: "oklch(98% 0.01 240)",
        radius: "1rem",
      },
    };

    applySlabThemeToDocument(snapshot, document);

    expect(document.documentElement.classList.contains("dark")).toBe(true);
    expect(document.documentElement.style.getPropertyValue("--background")).toBe(
      "oklch(20% 0.01 240)",
    );
    expect(document.documentElement.style.getPropertyValue("--foreground")).toBe(
      "oklch(98% 0.01 240)",
    );
    expect(document.documentElement.style.getPropertyValue("--radius")).toBe("1rem");
  });

  it("removes dark mode for light snapshots", () => {
    const document = window.document.implementation.createHTMLDocument("plugin");
    document.documentElement.classList.add("dark");

    applySlabThemeToDocument({ mode: "light", tokens: {} }, document);

    expect(document.documentElement.classList.contains("dark")).toBe(false);
  });
});
