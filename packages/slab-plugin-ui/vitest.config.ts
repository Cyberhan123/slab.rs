import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    name: "plugin-ui",
    environment: "node",
    include: ["src/**/*.test.ts"],
  },
});
