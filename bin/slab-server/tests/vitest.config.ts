import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    include: [
      "integration/**/*.integration.test.ts",
      "unit/**/*.unit.test.ts"
    ],
    testTimeout: 30000,
    hookTimeout: 30000,
    env: {
      SLAB_SERVER_BASE_URL: "http://127.0.0.1:3000"
    }
  }
});
