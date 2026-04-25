import { describe, expect, it } from "vitest";

import { requiredSlabApiPermission } from "../permissions";

describe("requiredSlabApiPermission", () => {
  it("matches the plugin API permission surface enforced by the Tauri host", () => {
    expect(requiredSlabApiPermission("GET", "/v1/models?capability=chat_generation")).toBe(
      "models:read",
    );
    expect(requiredSlabApiPermission("POST", "/v1/ffmpeg/convert")).toBe("ffmpeg:convert");
    expect(requiredSlabApiPermission("POST", "/v1/audio/transcriptions")).toBe(
      "audio:transcribe",
    );
    expect(requiredSlabApiPermission("POST", "/v1/subtitles/render")).toBe("subtitle:render");
    expect(requiredSlabApiPermission("POST", "/v1/chat/completions")).toBe("chat:complete");
    expect(requiredSlabApiPermission("GET", "/v1/tasks/task-1/result")).toBe("tasks:read");
    expect(requiredSlabApiPermission("POST", "/v1/tasks/task-1/cancel")).toBe("tasks:cancel");
  });

  it("rejects unknown or undeclared plugin API paths", () => {
    expect(requiredSlabApiPermission("GET", "/v1/settings")).toBeNull();
    expect(requiredSlabApiPermission("DELETE", "/v1/models/model-1")).toBeNull();
  });
});
