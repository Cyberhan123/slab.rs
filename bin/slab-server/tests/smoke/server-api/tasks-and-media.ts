import { resolve } from "node:path";

import { beforeAll, describe, expect, it } from "vitest";

import type { SlabServerTestHarness } from "../../support/slab-server";
import { expectError, expectJson, jsonInit, type Schema } from "./shared";

export function registerTasksAndMediaSmoke(getServer: () => SlabServerTestHarness): void {
  describe("slab-server smoke tasks and media", () => {
    let server: SlabServerTestHarness;

    beforeAll(() => {
      server = getServer();
    });

    it("covers tasks and media routes without running runtime work", async () => {
      const tasks = await expectJson<Schema["TaskResponse"][]>(server, "/v1/tasks");
      expect(tasks.response.ok).toBe(true);
      expect(Array.isArray(tasks.body)).toBe(true);

      await expectError(server, "/v1/tasks/missing-task", 404);
      await expectError(server, "/v1/tasks/missing-task/result", 404);
      await expectError(server, "/v1/tasks/missing-task/cancel", 404, { method: "POST" });

      const audioTasks = await expectJson<Schema["AudioTranscriptionTaskResponse"][]>(
        server,
        "/v1/audio/transcriptions"
      );
      expect(audioTasks.response.ok).toBe(true);
      expect(Array.isArray(audioTasks.body)).toBe(true);
      await expectError(server, "/v1/audio/transcriptions/missing-task", 404);
      await expectError(
        server,
        "/v1/audio/transcriptions",
        400,
        jsonInit({ path: "relative.wav" } satisfies Schema["AudioTranscriptionRequest"], {
          method: "POST"
        })
      );

      const imageTasks = await expectJson<Schema["ImageGenerationTaskResponse"][]>(
        server,
        "/v1/images/generations"
      );
      expect(imageTasks.response.ok).toBe(true);
      expect(Array.isArray(imageTasks.body)).toBe(true);
      await expectError(server, "/v1/images/generations/missing-task", 404);
      await expectError(server, "/v1/images/generations/missing-task/artifacts/0", 404);
      await expectError(server, "/v1/images/generations/missing-task/reference", 404);
      await expectError(
        server,
        "/v1/images/generations",
        400,
        jsonInit(
          {
            mode: "img2img",
            model: "missing-model",
            prompt: "smoke"
          } satisfies Schema["ImageGenerationRequest"],
          { method: "POST" }
        )
      );

      const videoTasks = await expectJson<Schema["VideoGenerationTaskResponse"][]>(
        server,
        "/v1/video/generations"
      );
      expect(videoTasks.response.ok).toBe(true);
      expect(Array.isArray(videoTasks.body)).toBe(true);
      await expectError(server, "/v1/video/generations/missing-task", 404);
      await expectError(server, "/v1/video/generations/missing-task/artifact", 404);
      await expectError(server, "/v1/video/generations/missing-task/reference", 404);
      await expectError(
        server,
        "/v1/video/generations",
        400,
        jsonInit(
          {
            model: "",
            prompt: "smoke"
          } satisfies Schema["VideoGenerationRequest"],
          { method: "POST" }
        )
      );

      await expectError(
        server,
        "/v1/ffmpeg/convert",
        400,
        jsonInit(
          {
            output_format: "mp3",
            source_path: resolve("missing-smoke-input.wav")
          } satisfies Schema["ConvertRequest"],
          { method: "POST" }
        )
      );
      await expectError(
        server,
        "/v1/subtitles/render",
        400,
        jsonInit(
          {
            entries: [{ end_ms: 1000, start_ms: 0, text: "hello" }],
            format: "srt",
            source_path: "relative.mp4",
            variant: "source"
          } satisfies Schema["RenderSubtitleRequest"],
          { method: "POST" }
        )
      );
    });
  });
}
