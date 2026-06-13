import { beforeAll, describe, expect, it } from "vitest";

import type { SlabServerTestHarness } from "../../support/slab-server";
import {
  expectError,
  expectJson,
  expectOpenAiError,
  jsonInit,
  type DeletedModelResponse,
  type Schema
} from "./shared";

export function registerModelsAndChatSmoke(
  getServer: () => SlabServerTestHarness
): void {
  describe("slab-server smoke models and chat", () => {
    let server: SlabServerTestHarness;

    beforeAll(() => {
      server = getServer();
    });

    it("covers model catalog and chat compatibility routes without inference", async () => {
      const createRequest: Schema["CreateModelRequest"] = {
        backend_id: "ggml.llama",
        capabilities: ["chat_generation", "text_generation"],
        display_name: "Vitest Smoke Local Chat Model",
        kind: "local"
      };
      const created = await expectJson<Schema["UnifiedModelResponse"]>(
        server,
        "/v1/models",
        jsonInit(createRequest, { method: "POST" })
      );
      expect(created.response.ok).toBe(true);
      expect(created.body.display_name).toBe(createRequest.display_name);
      expect(created.body.kind).toBe("local");
      expect(created.body.backend_id).toBe("ggml.llama");
      expect(created.body.capabilities).toContain("chat_generation");

      try {
        const listed = await expectJson<Schema["UnifiedModelResponse"][]>(server, "/v1/models");
        expect(listed.response.ok).toBe(true);
        expect(listed.body.some((model) => model.id === created.body.id)).toBe(true);

        const filtered = await expectJson<Schema["UnifiedModelResponse"][]>(
          server,
          "/v1/models?capability=chat_generation"
        );
        expect(filtered.body.some((model) => model.id === created.body.id)).toBe(true);

        const chatModels = await expectJson<Schema["ChatModelOption"][]>(server, "/v1/chat/models");
        expect(chatModels.response.headers.get("deprecation")).toBe("true");
        expect(chatModels.response.headers.get("sunset")).toBe("Tue, 08 Jun 2027 00:00:00 GMT");
        const chatModel = chatModels.body.find((model) => model.id === created.body.id);
        expect(chatModel).toBeDefined();
        expect(chatModel?.source).toBe("local");
        expect(chatModel?.downloaded).toBe(false);

        const fetched = await expectJson<Schema["UnifiedModelResponse"]>(
          server,
          `/v1/models/${created.body.id}`
        );
        expect(fetched.body.id).toBe(created.body.id);

        const updated = await expectJson<Schema["UnifiedModelResponse"]>(
          server,
          `/v1/models/${created.body.id}`,
          jsonInit(
            { display_name: "Vitest Smoke Renamed Model" } satisfies Schema["UpdateModelRequest"],
            {
              method: "PUT"
            }
          )
        );
        expect(updated.body.display_name).toBe("Vitest Smoke Renamed Model");

        await expectError(server, `/v1/models/${created.body.id}/config-document`, 400);
        await expectError(
          server,
          `/v1/models/${created.body.id}/config-selection`,
          400,
          jsonInit({} satisfies Schema["UpdateModelConfigSelectionRequest"], { method: "PUT" })
        );
      } finally {
        const deleted = await expectJson<DeletedModelResponse>(
          server,
          `/v1/models/${created.body.id}`,
          { method: "DELETE" }
        );
        expect(deleted.response.ok).toBe(true);
        expect(deleted.body).toEqual({
          id: created.body.id,
          status: "deleted"
        });
      }

      await expectOpenAiError(
        server,
        "/v1/chat/completions",
        400,
        jsonInit(
          {
            messages: [{ content: "hello", role: "user" }],
            model: "missing-model",
            n: 2,
            stream: true
          } satisfies Schema["ChatCompletionRequest"],
          { method: "POST" }
        )
      );
      await expectOpenAiError(
        server,
        "/v1/completions",
        400,
        jsonInit(
          {
            model: "missing-model",
            n: 2,
            prompt: "hello",
            stream: true
          } satisfies Schema["CompletionRequest"],
          { method: "POST" }
        )
      );
      await expectError(server, "/v1/models/available?repo_id=", 400);
      await expectError(
        server,
        "/v1/models/load",
        400,
        jsonInit({} satisfies Schema["LoadModelRequest"], { method: "POST" })
      );
      await expectError(
        server,
        "/v1/models/switch",
        400,
        jsonInit({} satisfies Schema["SwitchModelRequest"], { method: "POST" })
      );
      await expectError(
        server,
        "/v1/models/unload",
        400,
        jsonInit({} satisfies Schema["UnloadModelRequest"], { method: "POST" })
      );
      await expectError(
        server,
        "/v1/models/download",
        404,
        jsonInit({ model_id: "missing-model" } satisfies Schema["DownloadModelRequest"], {
          method: "POST"
        })
      );

      const emptyPack = new FormData();
      const importPack = await server.requestFormData("/v1/models/import-pack", emptyPack, {
        method: "POST"
      });
      expect(importPack.status).toBe(400);
    });
  });
}
