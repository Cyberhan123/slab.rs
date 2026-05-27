import { beforeAll, describe, expect, it } from "vitest";

import type { SlabServerTestHarness } from "../../support/slab-server";
import { expectError, expectJson, jsonInit, type Schema } from "./shared";

export function registerAgentsAndLspSmoke(getServer: () => SlabServerTestHarness): void {
  describe("slab-server smoke agents and workspace lsp", () => {
    let server: SlabServerTestHarness;

    beforeAll(() => {
      server = getServer();
    });

    it("covers unified agent responses and workspace LSP routes without running agent work", async () => {
      await expectError(
        server,
        "/v1/agents/responses",
        400,
        jsonInit(
          {
            messages: [],
            session_id: "",
            type: "agent.response.create"
          } satisfies Schema["AgentResponsesClientMessage"],
          { method: "POST" }
        )
      );

      const restored = await expectJson<Schema["AgentResponsesServerMessage"]>(
        server,
        "/v1/agents/responses",
        jsonInit(
          {
            session_id: "missing-session",
            type: "agent.session.restore"
          } satisfies Schema["AgentResponsesClientMessage"],
          { method: "POST" }
        )
      );
      expect(restored.response.ok).toBe(true);
      expect(restored.body).toMatchObject({
        messages: [],
        session_id: "missing-session",
        type: "agent.session.restored"
      });

      await expectError(
        server,
        "/v1/agents/responses",
        404,
        jsonInit(
          {
            content: "resume this agent",
            thread_id: "missing-agent",
            type: "agent.input"
          } satisfies Schema["AgentResponsesClientMessage"],
          { method: "POST" }
        )
      );

      const sseMissingThread = await expectError(
        server,
        "/v1/agents/responses?transport=sse",
        400
      );
      expect(sseMissingThread.message).toContain("thread_id");

      const events = await server.request(
        "/v1/agents/responses?transport=sse&thread_id=missing-agent"
      );
      expect(events.ok).toBe(true);
      expect(events.headers.get("content-type")).toContain("text/event-stream");
      await events.body?.cancel();

      const oldAgentRoute = await server.request("/v1/agents/missing-agent/events");
      expect(oldAgentRoute.status).toBe(404);

      const lspUpgradeMissing = await server.request("/v1/workspace/lsp/typescript");
      expect(lspUpgradeMissing.status).not.toBe(404);
    });
  });
}
