import { defineProject } from "vitest/config";

export default defineProject({
  test: {
    name: "slab-server-tests",
    include: [
      "integration/**/*.integration.test.ts",
      "unit/**/*.unit.test.ts"
    ],
    testTimeout: 120000,
    hookTimeout: 120000,
    fileParallelism: false,
    env: {
      SLAB_SERVER_BASE_URL: "http://127.0.0.1:3000"
    }
  }
});
