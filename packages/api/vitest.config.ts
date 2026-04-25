import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    name: "api",
    environment: "jsdom",
    include: ["src/**/*.test.ts"],
  },
});
