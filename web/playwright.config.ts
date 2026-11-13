import { defineConfig } from "@playwright/test";

// End-to-end tests against the static export (run `npm run build` first; the
// `e2e` script does). Opt-in only: they download and drive a real browser.
export default defineConfig({
  testDir: "e2e",
  timeout: 120_000,
  fullyParallel: false,
  workers: 1,
  reporter: [["list"]],
  use: {
    baseURL: "http://localhost:4174",
    viewport: { width: 1440, height: 900 },
    launchOptions: {
      // Default: software WebGL (works everywhere, slow). PW_GPU=1 uses the
      // machine's GPU via Direct3D (Windows) for realistic measurements.
      args: process.env.PW_GPU
        ? ["--use-angle=d3d11", "--enable-gpu", "--ignore-gpu-blocklist"]
        : ["--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"],
    },
  },
  webServer: {
    command: "npx --no-install serve out -l 4174",
    url: "http://localhost:4174",
    reuseExistingServer: false,
    timeout: 60_000,
  },
});
