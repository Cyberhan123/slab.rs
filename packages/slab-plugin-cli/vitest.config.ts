import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    name: "plugin-cli",
    environment: "node",
    include: ["tests/**/*.test.ts"],
  },
});
