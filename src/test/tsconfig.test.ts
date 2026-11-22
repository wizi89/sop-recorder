import { describe, it, expect } from "vitest";
import fs from "node:fs";
import path from "node:path";

// CODING_STANDARDS.md used to ask the reviewer to keep these compiler flags on.
// A flag turned off silently widens what `npx tsc --noEmit` lets through, so
// it is a check instead.
describe("tsconfig.json keeps the strict flags on", () => {
  const raw = fs.readFileSync(path.resolve(__dirname, "../../tsconfig.json"), "utf8");
  // tsconfig is JSONC: drop the block comments it uses before parsing.
  const config = JSON.parse(raw.replace(/\/\*[\s\S]*?\*\//g, ""));

  it.each(["strict", "noUnusedLocals", "noUnusedParameters", "noFallthroughCasesInSwitch"])(
    "%s is true",
    (flag) => {
      expect(config.compilerOptions[flag]).toBe(true);
    },
  );
});
