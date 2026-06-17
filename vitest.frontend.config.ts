import { defineConfig } from "vitest/config";

import { frontendVitestProjects } from "./vitest.projects";

export default defineConfig({
  test: {
    projects: [...frontendVitestProjects],
    reporters: ["default"],
  },
});
