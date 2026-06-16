/* eslint-disable vitest/no-conditional-tests, vitest/valid-title, vitest/warn-todo -- This suite intentionally records future coverage placeholders. */
import { describe, it } from "vitest";

import { futureCompatibilityScenarios, todoSmokeOperations } from "./shared";

export function registerSmokeTodoSuites(): void {
  if (todoSmokeOperations.length > 0) {
    describe("slab-server current smoke TODOs", () => {
      for (const operation of todoSmokeOperations) {
        it.todo(`${operation.method.toUpperCase()} ${operation.path} has an executable smoke test`);
      }
    });
  }

  describe("slab-server future compatibility smoke TODOs", () => {
    for (const scenario of futureCompatibilityScenarios) {
      it.todo(scenario);
    }
  });
}
