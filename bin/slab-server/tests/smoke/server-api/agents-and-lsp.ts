import { beforeAll, describe, expect, it } from "vitest";

import type { SlabServerTestHarness } from "../../support/slab-server";
import {
  expectError,
  expectJson,
  expectWebSocketJsonReply,
  expectWebSocketOpens,
  jsonInit,
  type Schema
} from "./shared";

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

      const workspaceState = await expectJson<Schema["WorkspaceStateResponse"]>(
        server,
        "/v1/workspace"
      );
      expect(workspaceState.response.ok).toBe(true);
      expect(Array.isArray(workspaceState.body.recent)).toBe(true);

      await expectError(
        server,
        "/v1/workspace/open",
        400,
        jsonInit({ rootPath: "" } satisfies Schema["WorkspaceOpenCommand"], { method: "POST" })
      );
      const closedWorkspace = await expectJson<Schema["WorkspaceStateResponse"]>(
        server,
        "/v1/workspace/close",
        { method: "POST" }
      );
      expect(closedWorkspace.response.ok).toBe(true);
      expect(closedWorkspace.body.current).toBeNull();

      await Promise.all([
        expectError(server, "/v1/workspace/directory", 400),
        expectError(server, "/v1/workspace/files?relativePath=smoke.txt", 400),
        expectError(server, "/v1/workspace/path/stat?relativePath=smoke.txt", 400),
        expectError(server, "/v1/workspace/search?query=smoke", 400),
        expectError(server, "/v1/workspace/search/text?query=smoke", 400),
        expectError(server, "/v1/workspace/git/status", 400)
      ]);
      await Promise.all([
        expectError(
          server,
          "/v1/workspace/files",
          400,
          jsonInit(
            {
              content: "smoke",
              expectedHash: null,
              relativePath: "smoke.txt"
            } satisfies Schema["WorkspaceWriteFileCommand"],
            { method: "PUT" }
          )
        ),
        expectError(
          server,
          "/v1/workspace/files",
          400,
          jsonInit(
            { relativePath: "smoke.txt" } satisfies Schema["WorkspaceCreateFileCommand"],
            { method: "POST" }
          )
        ),
        expectError(
          server,
          "/v1/workspace/directories",
          400,
          jsonInit(
            { relativePath: "smoke-dir" } satisfies Schema["WorkspaceCreateDirectoryCommand"],
            { method: "POST" }
          )
        ),
        expectError(
          server,
          "/v1/workspace/path",
          400,
          jsonInit(
            {
              fromRelativePath: "smoke.txt",
              toRelativePath: "renamed.txt"
            } satisfies Schema["WorkspaceRenamePathCommand"],
            { method: "PATCH" }
          )
        ),
        expectError(
          server,
          "/v1/workspace/path",
          400,
          jsonInit(
            {
              recursive: false,
              relativePath: "smoke.txt"
            } satisfies Schema["WorkspaceDeletePathCommand"],
            { method: "DELETE" }
          )
        )
      ]);
      await Promise.all([
        expectError(
          server,
          "/v1/workspace/git/stage",
          400,
          jsonInit({ path: "smoke.txt" } satisfies Schema["WorkspaceGitPathCommand"], {
            method: "POST"
          })
        ),
        expectError(
          server,
          "/v1/workspace/git/unstage",
          400,
          jsonInit({ path: "smoke.txt" } satisfies Schema["WorkspaceGitPathCommand"], {
            method: "POST"
          })
        ),
        expectError(
          server,
          "/v1/workspace/git/discard",
          400,
          jsonInit({ path: "smoke.txt" } satisfies Schema["WorkspaceGitPathCommand"], {
            method: "POST"
          })
        ),
        expectError(
          server,
          "/v1/workspace/git/commit",
          400,
          jsonInit({ message: "smoke" } satisfies Schema["WorkspaceGitCommitCommand"], {
            method: "POST"
          })
        ),
        expectError(
          server,
          "/v1/workspace/git/diff",
          400,
          jsonInit(
            {
              path: "smoke.txt",
              staged: false
            } satisfies Schema["WorkspaceGitDiffCommand"],
            { method: "POST" }
          )
        ),
        expectError(
          server,
          "/v1/workspace/console/run",
          400,
          jsonInit({ command: "pwd" } satisfies Schema["WorkspaceConsoleRunCommand"], {
            method: "POST"
          })
        )
      ]);

      const wsError = await expectWebSocketJsonReply<Schema["AgentResponsesServerMessage"]>(
        server,
        "/v1/agents/responses",
        "not json"
      );
      expect(wsError).toMatchObject({
        code: "bad_request",
        type: "agent.error"
      });

      await expectWebSocketOpens(server, "/v1/workspace/lsp/smoke-no-provider");
    });
  });
}
