import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

import { beforeAll, describe, expect, it } from "vitest";

import type { SlabServerTestHarness } from "../../support/slab-server";
import {
  expectError,
  expectJson,
  externalBaseUrl,
  jsonInit,
  waitForTask,
  type Schema
} from "./shared";

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

    it.skipIf(Boolean(externalBaseUrl))(
      "restarts a seeded failed model download task through public task APIs",
      async () => {
        const seeded = await server.seedFailedModelDownloadTask();
        const restarted = await expectJson<Schema["TaskResponse"]>(
          server,
          `/v1/tasks/${seeded.taskId}/restart`,
          { method: "POST" }
        );
        expect(restarted.response.ok).toBe(true);
        expect(restarted.body.id).toBe(seeded.taskId);
        expect(restarted.body.task_type).toBe("model_download");
        expect(["pending", "running", "failed"]).toContain(restarted.body.status);

        const visible = await waitForTask(
          server,
          seeded.taskId,
          (task) => task.task_type === "model_download" && task.id === seeded.taskId
        );
        expect(["pending", "running", "failed"]).toContain(visible.status);

        const modelDownloadTasks = await expectJson<Schema["TaskResponse"][]>(
          server,
          "/v1/tasks?type=model_download"
        );
        expect(modelDownloadTasks.body.some((task) => task.id === seeded.taskId)).toBe(true);
      }
    );

    it.skipIf(Boolean(externalBaseUrl))(
      "renders subtitles and accepts an ffmpeg conversion task for temp media",
      async () => {
        const mediaRoot = await mkdtemp(join(tmpdir(), "slab-server-media-smoke-"));
        try {
          const subtitleSourcePath = join(mediaRoot, "clip.mp4");
          await writeFile(subtitleSourcePath, "");
          const rendered = await expectJson<Schema["RenderSubtitleResponse"]>(
            server,
            "/v1/subtitles/render",
            jsonInit(
              {
                entries: [{ end_ms: 1500, start_ms: 0, text: "Hello world" }],
                format: "srt",
                overwrite: true,
                source_path: subtitleSourcePath,
                variant: "source"
              } satisfies Schema["RenderSubtitleRequest"],
              { method: "POST" }
            )
          );
          expect(rendered.response.ok).toBe(true);
          expect(rendered.body.entry_count).toBe(1);
          expect(rendered.body.format).toBe("srt");
          const subtitleBody = await readFile(rendered.body.output_path, "utf8");
          expect(subtitleBody).toContain("00:00:00,000 --> 00:00:01,500");
          expect(subtitleBody).toContain("Hello world");

          const sampleRate = 8000;
          const sampleCount = sampleRate / 10;
          const dataSize = sampleCount * 2;
          const wav = Buffer.alloc(44 + dataSize);
          wav.write("RIFF", 0, "ascii");
          wav.writeUInt32LE(36 + dataSize, 4);
          wav.write("WAVE", 8, "ascii");
          wav.write("fmt ", 12, "ascii");
          wav.writeUInt32LE(16, 16);
          wav.writeUInt16LE(1, 20);
          wav.writeUInt16LE(1, 22);
          wav.writeUInt32LE(sampleRate, 24);
          wav.writeUInt32LE(sampleRate * 2, 28);
          wav.writeUInt16LE(2, 32);
          wav.writeUInt16LE(16, 34);
          wav.write("data", 36, "ascii");
          wav.writeUInt32LE(dataSize, 40);

          const wavPath = join(mediaRoot, "input.wav");
          await writeFile(wavPath, wav);
          const converted = await expectJson<Schema["OperationAcceptedResponse"]>(
            server,
            "/v1/ffmpeg/convert",
            jsonInit(
              {
                output_format: "wav",
                output_path: join(mediaRoot, "converted.wav"),
                source_path: wavPath
              } satisfies Schema["ConvertRequest"],
              { method: "POST" }
            )
          );
          expect(converted.response.status).toBe(202);
          expect(converted.body.operation_id).toBeTypeOf("string");

          const convertedTask = await waitForTask(
            server,
            converted.body.operation_id,
            (task) => task.status === "succeeded" || task.status === "failed",
            30_000
          );
          expect(convertedTask.task_type).toBe("ffmpeg");
          expect(["succeeded", "failed"]).toContain(convertedTask.status);
        } finally {
          await rm(mediaRoot, { force: true, recursive: true });
        }
      }
    );
  });
}
