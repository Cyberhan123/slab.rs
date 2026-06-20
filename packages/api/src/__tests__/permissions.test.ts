import { describe, expect, it } from "vitest";

import {
  SLAB_API_PERMISSIONS,
  describeSlabApiPermission,
  isKnownSlabApiPermission,
  requiredSlabApiPermission,
} from "../permissions";

describe("requiredSlabApiPermission", () => {
  it("matches the plugin API permission surface enforced by the Tauri host", () => {
    expect(requiredSlabApiPermission("GET", "/v1/models?capability=chat_generation")).toBe(
      "models:read",
    );
    expect(requiredSlabApiPermission("POST", "/v1/models/load")).toBe("models:load");
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

describe("plugin permission labels", () => {
  it("labels every permission on the Slab API surface", () => {
    for (const permission of Object.values(SLAB_API_PERMISSIONS)) {
      expect(isKnownSlabApiPermission(permission)).toBe(true);
      const label = describeSlabApiPermission(permission);
      expect(label.title.length).toBeGreaterThan(0);
      expect(label.description.length).toBeGreaterThan(0);
      expect(["low", "medium", "high"]).toContain(label.severity);
    }
  });

  it("flags unknown permissions as high severity so reviewers are warned", () => {
    expect(isKnownSlabApiPermission("admin:all")).toBe(false);
    const label = describeSlabApiPermission("admin:all");
    expect(label.severity).toBe("high");
    expect(label.title).toContain("admin:all");
  });

  it("treats sensitive operations as high severity", () => {
    expect(describeSlabApiPermission(SLAB_API_PERMISSIONS.chatComplete).severity).toBe("high");
    expect(describeSlabApiPermission(SLAB_API_PERMISSIONS.modelsLoad).severity).toBe("high");
    expect(describeSlabApiPermission(SLAB_API_PERMISSIONS.modelsRead).severity).toBe("low");
  });
});
