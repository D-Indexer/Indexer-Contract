import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    // Transactions run against one shared local network; keep them ordered.
    fileParallelism: false,
    testTimeout: 120_000,
    hookTimeout: 120_000,
  },
});
