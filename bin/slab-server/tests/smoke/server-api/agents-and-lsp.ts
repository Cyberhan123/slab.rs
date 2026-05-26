import { beforeAll, describe, expect, it } from "vitest";

import type { SlabServerTestHarness } from "../../support/slab-server";
import { expectError, expectJson, jsonInit, type Schema } from "./shared";

export function registerAgentsAndLspSmoke(getServer: () => SlabServerTestHarness): void {
  describe("slab-server smoke agents and workspace lsp", () => {
    let server: SlabServerTestHarness;

    beforeAll(() => {
      server = getServer();
    });

    it("covers agent control and workspace LSP routes without running agent work", async () => {
      await expectError(
        server,
        "/v1/agents/spawn",
        400,
        jsonInit(
          {
            session_id: "",
            messages: []
          } satisfies Schema["SpawnAgentRequest"],
          { method: "POST" }
        )
      );
      await expectError(server, "/v1/agents/missing-agent/status", 404);
      await expectError(
        server,
        "/v1/agents/missing-agent/input",
        404,
        jsonInit(
          {
            content: "resume this agent"
          } satisfies Schema["AgentInputRequest"],
          { method: "POST" }
        )
      );
      await expectError(server, "/v1/agents/missing-agent/shutdown", 404, { method: "POST" });
      await expectError(server, "/v1/agents/missing-agent/interrupt", 404, { method: "POST" });

      const approval = await expectJson<Schema["AgentApproveResponse"]>(
        server,
        "/v1/agents/missing-agent/approve",
        jsonInit(
          {
            approved: true,
            call_id: "missing-call"
          } satisfies Schema["AgentApproveRequest"],
          { method: "POST" }
        )
      );
      expect(approval.response.ok).toBe(true);
      expect(approval.body.delivered).toBe(false);

      const events = await server.request("/v1/agents/missing-agent/events");
      expect(events.ok).toBe(true);
      expect(events.headers.get("content-type")).toContain("text/event-stream");
      await events.body?.cancel();

      const lspUpgradeMissing = await server.request("/v1/workspace/lsp/typescript");
      expect(lspUpgradeMissing.status).not.toBe(404);
    });
  });
}
