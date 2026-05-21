import { describe, it } from "vitest";

import { futureCompatibilityScenarios, todoSmokeOperations } from "./shared";

export function registerSmokeTodoSuites(): void {
  describe("slab-server current smoke TODOs", () => {
    for (const operation of todoSmokeOperations) {
      it.todo(`${operation.method.toUpperCase()} ${operation.path} has an executable smoke test`);
    }
  });

  describe("slab-server future compatibility smoke TODOs", () => {
    for (const scenario of futureCompatibilityScenarios) {
      it.todo(scenario);
    }
  });
}
