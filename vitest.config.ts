import { configDefaults, defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: {
    globals: true,
    environment: "happy-dom",
    setupFiles: ["./src/test/setup.ts"],
    // Tests that render the whole App import it inside the test, and under a
    // full parallel run that alone came close to the 5 s default and failed
    // at random.
    testTimeout: 15_000,
    // Agent worktrees are full checkouts; without this their tests run too.
    exclude: [...configDefaults.exclude, ".claude/worktrees/**"],
  },
});
