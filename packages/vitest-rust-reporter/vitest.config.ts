import { defineProject } from "vitest/config";
import { createRustVitestProject } from "./src/index.ts";

export default defineProject(
  createRustVitestProject({
    cwd: process.cwd(),
    name: "rust",
    testCommand: ["cargo", "test", "--workspace", "--color", "never"],
    coverage: {
      enabled: "if-available",
      command: [
        "cargo",
        "llvm-cov",
        "--workspace",
        "--json",
        "--summary-only",
        "--color",
        "never",
      ],
    },
  }),
);
