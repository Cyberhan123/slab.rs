import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    name: "plugin-sdk",
    environment: "jsdom",
    include: ["tests/**/*.test.ts"],
  },
});
