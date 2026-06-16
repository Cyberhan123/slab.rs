import { beforeAll, describe, expect, it } from "vitest";

import type { SlabServerTestHarness } from "../../support/slab-server";
import {
  documentedOperationKeys,
  externalBaseUrl,
  executableSmokeOperations,
  expectError,
  expectJson,
  jsonInit,
  operationKey,
  todoSmokeOperations,
  waitForTask,
  type HealthResponse,
  type OpenApiDocument,
  type Schema
} from "./shared";

export function registerCoreAndSetupSmoke(
  getServer: () => SlabServerTestHarness
): void {
  describe("slab-server smoke core and setup", () => {
    let server: SlabServerTestHarness;

    beforeAll(() => {
      server = getServer();
    });

    it("serves health, OpenAPI docs, and a complete smoke coverage map", async () => {
      const health = await expectJson<HealthResponse>(server, "/health");
      expect(health.response.ok).toBe(true);
      expect(health.body.status).toBe("ok");
      expect(typeof health.body.version).toBe("string");
      expect(health.body.version?.length ?? 0).toBeGreaterThan(0);

      const openapi = await expectJson<OpenApiDocument>(server, "/api-docs/openapi.json");
      expect(openapi.response.ok).toBe(true);
      expect(openapi.body.openapi).toBeTypeOf("string");
      expect(openapi.body.paths).toBeTypeOf("object");
      expect(openapi.body.paths).toHaveProperty("/health");
      expect(openapi.body.paths).toHaveProperty("/v1/models");
      expect(openapi.body.paths).toHaveProperty("/v1/tasks/{id}/restart");

      const covered = [...executableSmokeOperations, ...todoSmokeOperations]
        .map(operationKey)
        .toSorted();
      expect(new Set(covered).size).toBe(covered.length);
      expect(documentedOperationKeys(openapi.body)).toEqual(covered);
    });

    it("returns CORS headers for browser preflight requests", async () => {
      const origin = "http://localhost:1420";
      const response = await server.request("/v1/setup/status", {
        headers: {
          "Access-Control-Request-Headers": "Authorization",
          "Access-Control-Request-Method": "GET",
          Origin: origin
        },
        method: "OPTIONS"
      });

      expect(response.status).toBeLessThan(300);
      expect(response.headers.get("access-control-allow-origin")).toBe(origin);
      expect(response.headers.get("access-control-allow-methods")).toBeTruthy();
      expect(response.headers.get("access-control-allow-headers")).toBeTruthy();
    });

    it("covers setup, settings, backend, and system endpoints without provisioning", async () => {
      const initial = await expectJson<Schema["SetupStatusResponse"]>(server, "/v1/setup/status");
      expect(initial.response.ok).toBe(true);
      expect(initial.body.ffmpeg.name).toBe("ffmpeg");
      expect(Array.isArray(initial.body.backends)).toBe(true);

      const completeRequest: Schema["CompleteSetupRequest"] = { initialized: true };
      const completed = await expectJson<Schema["SetupStatusResponse"]>(
        server,
        "/v1/setup/complete",
        jsonInit(completeRequest, { method: "POST" })
      );
      expect(completed.response.ok).toBe(true);
      expect(completed.body.initialized).toBe(true);

      const restored = await expectJson<Schema["SetupStatusResponse"]>(
        server,
        "/v1/setup/complete",
        jsonInit({ initialized: initial.body.initialized } satisfies Schema["CompleteSetupRequest"], {
          method: "POST"
        })
      );
      expect(restored.response.ok).toBe(true);
      expect(restored.body.initialized).toBe(initial.body.initialized);

      const settings = await expectJson<Schema["SettingsDocumentView"]>(server, "/v1/settings");
      expect(settings.response.ok).toBe(true);
      expect(Array.isArray(settings.body.sections)).toBe(true);

      const originalLanguage = await expectJson<Schema["SettingPropertyView"]>(
        server,
        "/v1/settings/general.language"
      );
      const nextLanguage =
        originalLanguage.body.effective_value === "zh-CN" ? "en-US" : "zh-CN";
      try {
        const updatedLanguage = await expectJson<Schema["SettingPropertyView"]>(
          server,
          "/v1/settings/general.language",
          jsonInit(
            {
              op: "set",
              value: nextLanguage
            } satisfies Schema["UpdateSettingCommand"],
            { method: "PUT" }
          )
        );
        expect(updatedLanguage.body.pmid).toBe("general.language");
        expect(updatedLanguage.body.effective_value).toBe(nextLanguage);
        expect(updatedLanguage.body.is_overridden).toBe(true);

        const fetchedLanguage = await expectJson<Schema["SettingPropertyView"]>(
          server,
          "/v1/settings/general.language"
        );
        expect(fetchedLanguage.body.effective_value).toBe(nextLanguage);
      } finally {
        const restoreCommand = originalLanguage.body.is_overridden
          ? ({
              op: "set",
              value: originalLanguage.body.override_value ?? originalLanguage.body.effective_value
            } satisfies Schema["UpdateSettingCommand"])
          : ({ op: "unset" } satisfies Schema["UpdateSettingCommand"]);
        await expectJson<Schema["SettingPropertyView"]>(
          server,
          "/v1/settings/general.language",
          jsonInit(restoreCommand, { method: "PUT" })
        );
      }

      await expectError(server, "/v1/settings/smoke.missing", 404);
      await expectError(
        server,
        "/v1/settings/smoke.missing",
        404,
        jsonInit({ op: "unset" } satisfies Schema["UpdateSettingCommand"], { method: "PUT" })
      );

      const backends = await expectJson<Schema["BackendListResponse"]>(server, "/v1/backends");
      expect(backends.response.ok).toBe(true);
      expect(Array.isArray(backends.body.backends)).toBe(true);

      const backendStatus = await expectJson<Schema["BackendStatusResponse"]>(
        server,
        "/v1/backends/status?backend_id=ggml.llama"
      );
      expect(backendStatus.response.ok).toBe(true);
      expect(backendStatus.body.backend).toBe("ggml.llama");
      expect(backendStatus.body.status).toBeTypeOf("string");

      const gpu = await expectJson<Schema["GpuStatusResponse"]>(server, "/v1/system/gpu");
      expect(gpu.response.ok).toBe(true);
      expect(gpu.body.available).toBeTypeOf("boolean");
      expect(gpu.body.backend).toBeTypeOf("string");
      expect(Array.isArray(gpu.body.devices)).toBe(true);

      const diagnostics = await expectJson<Schema["SystemDiagnosticsResponse"]>(
        server,
        "/v1/system/diagnostics"
      );
      expect(diagnostics.response.ok).toBe(true);
      expect(diagnostics.body.status).toBe("ok");
      expect(diagnostics.body.admin_token_configured).toBeTypeOf("boolean");
      expect(Array.isArray(diagnostics.body.paths)).toBe(true);
      expect(diagnostics.body.paths.some((entry) => entry.label === "settings_file")).toBe(true);
    });

    it.skipIf(Boolean(externalBaseUrl))(
      "accepts setup provisioning and exposes the created task",
      async () => {
        const accepted = await expectJson<Schema["OperationAcceptedResponse"]>(
          server,
          "/v1/setup/provision",
          { method: "POST" }
        );
        expect(accepted.response.status).toBe(202);
        expect(accepted.body.operation_id).toBeTypeOf("string");
        expect(accepted.body.operation_id.length).toBeGreaterThan(0);

        const task = await waitForTask(
          server,
          accepted.body.operation_id,
          (provisionTask) => provisionTask.task_type === "setup_provision"
        );
        expect(task.id).toBe(accepted.body.operation_id);
        expect(["pending", "running", "succeeded", "failed", "interrupted"]).toContain(
          task.status
        );
      }
    );
  });
}
