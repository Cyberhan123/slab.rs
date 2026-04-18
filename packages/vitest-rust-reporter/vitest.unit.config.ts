import { defineProject } from "vitest/config";

export default defineProject({
  test: {
    name: "vitest-rust-reporter-unit",
    environment: "node",
    include: ["src/**/*.unit.test.ts"],
    fileParallelism: false,
  },
});
