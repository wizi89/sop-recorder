import { defineConfig } from "@playwright/test";

// The app's own tests (`e2e/`), run against the real binary. `*.e2e.ts`
// rather than `*.spec.ts` so vitest does not collect them too.
export default defineConfig({
  testDir: "e2e",
  testMatch: "*.e2e.ts",
  // One app at a time: they share the debug build's keyring entries.
  workers: 1,
  timeout: 60_000,
  reporter: [["list"]],
});
