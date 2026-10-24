// Builds the e2e binary into its own target directory. In the shared
// `target/debug`, any `cargo test` or `tauri dev` relinks the exe with the
// normal identifier, and the e2e runs then share a WebView2 profile with the
// installed app and fail with 0x8007139F.
import { spawnSync } from "node:child_process";
import path from "node:path";

const { status } = spawnSync(
  "npx tauri build --debug --no-bundle --config e2e/tauri.e2e.conf.json",
  {
    shell: true,
    stdio: "inherit",
    env: { ...process.env, CARGO_TARGET_DIR: path.resolve("src-tauri/target/e2e") },
  },
);
process.exit(status ?? 1);
