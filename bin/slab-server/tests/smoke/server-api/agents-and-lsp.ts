import { beforeAll, describe, expect, it } from "vitest";

import type { SlabServerTestHarness } from "../../support/slab-server";
import {
  expectError,
  expectJson,
  expectOpenAiError,
  expectWebSocketJsonReply,
  expectWorkspaceLspInitializeReply,
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
      await expectOpenAiError(
        server,
        "/v1/agents/responses",
        400,
        jsonInit(
          {
            input: ""
          } satisfies Schema["OpenAICreateRequest"],
          { method: "POST" }
        )
      );

      const history = await expectJson<Schema["AgentHistoryResponse"]>(
        server,
        "/v1/sessions/missing-session/agent-history"
      );
      expect(history.response.ok).toBe(true);
      expect(history.body).toMatchObject({
        messages: [],
        session_id: "missing-session"
      });
      expect(Array.isArray(history.body.responses)).toBe(true);

      const approval = await expectJson<Schema["AgentControlResponse"]>(
        server,
        "/v1/agents/control/approval",
        jsonInit(
          {
            approved: false,
            call_id: "missing-call",
            thread_id: "missing-agent"
          } satisfies Schema["AgentApprovalResolveRequest"],
          { method: "POST" }
        )
      );
      expect(approval.body).toMatchObject({
        delivered: false,
        thread_id: "missing-agent"
      });

      await expectError(
        server,
        "/v1/agents/control/interrupt",
        404,
        jsonInit(
          { thread_id: "missing-agent" } satisfies Schema["AgentThreadControlRequest"],
          { method: "POST" }
        )
      );

      await expectError(
        server,
        "/v1/agents/control/shutdown",
        404,
        jsonInit(
          { thread_id: "missing-agent" } satisfies Schema["AgentThreadControlRequest"],
          { method: "POST" }
        )
      );

      await expectOpenAiError(
        server,
        "/v1/agents/responses",
        400,
        jsonInit(
          {
            content: "resume this agent",
            thread_id: "missing-agent",
            type: "agent.input"
          },
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

      const wsError = await expectWebSocketJsonReply<{
        error: { code?: string; type?: string };
        type: string;
      }>(
        server,
        "/v1/agents/responses",
        "not json"
      );
      expect(wsError).toMatchObject({
        error: {
          code: "bad_request"
        },
        type: "error"
      });

      await expectWebSocketOpens(server, "/v1/workspace/lsp/smoke-no-provider");

      const { workspaceRoot } = server;
      if (workspaceRoot) {
        await expectJsonWorkspaceLsp(server, workspaceRoot);
      }
    });
  });
}

async function expectJsonWorkspaceLsp(
  server: SlabServerTestHarness,
  workspaceRoot: string
): Promise<void> {
  const openedWorkspace = await expectJson<Schema["WorkspaceStateResponse"]>(
    server,
    "/v1/workspace/open",
    jsonInit(
      { rootPath: workspaceRoot } satisfies Schema["WorkspaceOpenCommand"],
      { method: "POST" }
    )
  );
  expect(openedWorkspace.body.current?.rootPath).toBeTypeOf("string");

  const initializedJsonLsp = await expectWorkspaceLspInitializeReply(server, "json", workspaceRoot);
  expect(initializedJsonLsp).toMatchObject({
    id: 1,
    jsonrpc: "2.0"
  });
  expect(initializedJsonLsp.result?.capabilities?.textDocumentSync).toBeDefined();
}
