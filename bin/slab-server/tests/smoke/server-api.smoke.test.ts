import { afterAll, beforeAll, describe } from "vitest";

import {
  startSlabServerHarness,
  type SlabServerTestHarness
} from "../support/slab-server";
import { registerAdminAuthSmoke } from "./server-api/admin-auth";
import { registerAgentsAndLspSmoke } from "./server-api/agents-and-lsp";
import { registerCoreAndSetupSmoke } from "./server-api/core-and-setup";
import { registerModelsAndChatSmoke } from "./server-api/models-and-chat";
import { registerPluginsSmoke } from "./server-api/plugins";
import { registerSessionsAndUiStateSmoke } from "./server-api/sessions-and-ui-state";
import { externalBaseUrl } from "./server-api/shared";
import { registerTasksAndMediaSmoke } from "./server-api/tasks-and-media";
import { registerSmokeTodoSuites } from "./server-api/todos";

describe("slab-server smoke API", () => {
  let server: SlabServerTestHarness | undefined;

  beforeAll(async () => {
    server = await startSlabServerHarness({
      externalBaseUrl
    });
  });

  afterAll(async () => {
    await server?.stop();
  });

  registerCoreAndSetupSmoke(() => server!);
  registerModelsAndChatSmoke(() => server!);
  registerSessionsAndUiStateSmoke(() => server!);
  registerPluginsSmoke(() => server!);
  registerTasksAndMediaSmoke(() => server!);
  registerAgentsAndLspSmoke(() => server!);
});

registerAdminAuthSmoke();
registerSmokeTodoSuites();
