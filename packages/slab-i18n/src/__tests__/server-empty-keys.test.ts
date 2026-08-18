import { describe, expect, it } from "vitest";

import { enUS } from "../locales/en-US";
import { zhCN } from "../locales/zh-CN";

/**
 * `SERVER_I18N_KEYS` is locked to the backend `ServerI18nKey` enum, but the
 * message tables are Partial: keys without an entry are filled with '' by
 * `buildServerLocale`, and `translateServerField` falls back to the backend
 * field's own fallback text for empty translations.
 *
 * The keys below are INTENTIONALLY left blank (the backend owns that copy).
 * Any new empty server entry must either get a real translation or be added
 * here with a reason — otherwise this test fails.
 */
const INTENTIONALLY_EMPTY_SERVER_KEYS = [
  "server.modelConfig.sections.advanced.description",
  "server.modelConfig.sections.advanced.nonRuntimeDescription",
  "server.modelConfig.sections.inference.description",
  "server.modelConfig.sections.load.description",
  "server.modelConfig.sections.load.nonRuntimeDescription",
  "server.modelConfig.sections.source.description",
  "server.modelConfig.sections.summary.description",
  "server.modelConfig.fields.artifactPath.description",
  "server.modelConfig.fields.backend.productDescription",
  "server.modelConfig.fields.backend.runtimeDescription",
  "server.modelConfig.fields.capabilities.description",
  "server.modelConfig.fields.catalogStatus.description",
  "server.modelConfig.fields.chatTemplate.description",
  "server.modelConfig.fields.contextLength.description",
  "server.modelConfig.fields.diffusionAsset.description",
  "server.modelConfig.fields.diffusionDevice.description",
  "server.modelConfig.fields.diffusionPerformance.description",
  "server.modelConfig.fields.displayName.description",
  "server.modelConfig.fields.gbnf.description",
  "server.modelConfig.fields.localPath.description",
  "server.modelConfig.fields.modelId.description",
  "server.modelConfig.fields.nonRuntimeProjection.description",
  "server.modelConfig.fields.primaryArtifact.description",
  "server.modelConfig.fields.repoId.description",
  "server.modelConfig.fields.resolvedInferenceJson.description",
  "server.modelConfig.fields.resolvedInferenceJson.nonRuntimeDescription",
  "server.modelConfig.fields.resolvedLoadJson.description",
  "server.modelConfig.fields.runtimeLoadSupported.description",
  "server.modelConfig.fields.sourceKind.description",
  "server.modelConfig.fields.temperature.description",
  "server.modelConfig.fields.topP.description",
  "server.modelConfig.fields.workers.description",
] as const;

type LocaleTree = Record<string, unknown>;

function collectEmptyLeaves(value: unknown, prefix = "", out: string[] = []): string[] {
  if (value && typeof value === "object" && !Array.isArray(value)) {
    for (const [key, entry] of Object.entries(value as LocaleTree)) {
      collectEmptyLeaves(entry, prefix ? `${prefix}.${key}` : key, out);
    }
    return out;
  }
  if (value === "") out.push(prefix);
  return out;
}

describe("intentionally empty server keys", () => {
  it("keeps the blank server.* leaves exactly at the documented set", () => {
    // enUS.server is the built tree without the leading "server." segment,
    // so collect with that prefix to compare against SERVER_I18N_KEY paths.
    expect([...INTENTIONALLY_EMPTY_SERVER_KEYS].toSorted()).toEqual(
      collectEmptyLeaves(enUS.server, "server").toSorted(),
    );
    expect([...INTENTIONALLY_EMPTY_SERVER_KEYS].toSorted()).toEqual(
      collectEmptyLeaves(zhCN.server, "server").toSorted(),
    );
  });
});
