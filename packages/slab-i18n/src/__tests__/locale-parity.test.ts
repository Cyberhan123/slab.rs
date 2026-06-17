import { describe, expect, it } from "vitest";

import { enUS } from "../locales/en-US";
import { zhCN } from "../locales/zh-CN";

type LocaleTree = Record<string, unknown>;
type LocaleLeafKind = "array" | "boolean" | "null" | "number" | "object" | "string";

function getLeafKind(value: unknown): LocaleLeafKind {
  if (Array.isArray(value)) {
    return "array";
  }

  if (value === null) {
    return "null";
  }

  return typeof value as LocaleLeafKind;
}

function extractPlaceholders(message: string): string[] {
  return [...message.matchAll(/\{\{\s*([^}]+?)\s*\}\}/g)]
    .map((match) => match[1]?.trim() ?? "")
    .filter(Boolean)
    .toSorted();
}

function collectLocaleLeaves(
  value: unknown,
  path: string[] = [],
  leaves = new Map<string, { kind: LocaleLeafKind; placeholders: string[] }>(),
) {
  const kind = getLeafKind(value);
  const joinedPath = path.join(".");

  if (kind === "string") {
    leaves.set(joinedPath, {
      kind,
      placeholders: extractPlaceholders(value),
    });
    return leaves;
  }

  if (kind === "array") {
    value.forEach((entry, index) => {
      collectLocaleLeaves(entry, [...path, String(index)], leaves);
    });
    return leaves;
  }

  if (kind === "object") {
    Object.entries(value as LocaleTree).forEach(([key, entry]) => {
      collectLocaleLeaves(entry, [...path, key], leaves);
    });
    return leaves;
  }

  leaves.set(joinedPath, {
    kind,
    placeholders: [],
  });
  return leaves;
}

describe("locale parity", () => {
  it("keeps the runtime locale leaf paths aligned", () => {
    const enLeaves = collectLocaleLeaves(enUS);
    const zhLeaves = collectLocaleLeaves(zhCN);

    expect([...zhLeaves.keys()].toSorted()).toEqual([...enLeaves.keys()].toSorted());
  });

  it("keeps matching leaf kinds and placeholder shapes across locales", () => {
    const enLeaves = collectLocaleLeaves(enUS);
    const zhLeaves = collectLocaleLeaves(zhCN);

    expect(zhLeaves.size).toBe(enLeaves.size);

    for (const [path, enLeaf] of enLeaves) {
      const zhLeaf = zhLeaves.get(path);

      expect(zhLeaf, `missing locale leaf at ${path}`).toBeDefined();
      expect(zhLeaf?.kind, `leaf kind mismatch at ${path}`).toBe(enLeaf.kind);
      expect(zhLeaf?.placeholders, `placeholder mismatch at ${path}`).toEqual(
        enLeaf.placeholders,
      );
    }
  });
});
