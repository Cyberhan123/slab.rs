import { describe, expect, it } from "vitest";

import * as pluginUi from "./index.ts";

describe("@slab/plugin-ui", () => {
  it("re-exports the curated plugin component surface", () => {
    expect(pluginUi).toMatchObject({
      Badge: expect.any(Function),
      Button: expect.any(Function),
      Card: expect.any(Function),
      Checkbox: expect.any(Function),
      CompactConfigSummary: expect.any(Function),
      Input: expect.any(Function),
      Progress: expect.any(Function),
      Select: expect.any(Function),
      SoftPanel: expect.any(Function),
      Spinner: expect.any(Function),
      StageEmptyState: expect.any(Function),
      Switch: expect.any(Function),
      Tabs: expect.any(Function),
      Textarea: expect.any(Function),
      UploadDropzone: expect.any(Function),
      WorkspaceStage: expect.any(Function),
      cn: expect.any(Function),
    });
  });

  it("keeps the shared cn helper available to plugin UIs", () => {
    expect(pluginUi.cn("alpha", undefined, "beta")).toBe("alpha beta");
  });
});
