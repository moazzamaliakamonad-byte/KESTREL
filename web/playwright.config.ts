import { defineConfig } from "@playwright/test";

// End-to-end tests against the static export (run `npm run build` first; the
// `e2e` script does). Opt-in only: they download and drive a real browser.
// E2E_BASE_URL=http://localhost:3000 runs them against a running dev server
// instead (no build, no server started here).
const external = process.env.E2E_BASE_URL;
// PW_BROWSER=firefox runs the tests in Firefox (Chromium's flags are left out).
const firefox = process.env.PW_BROWSER === "firefox";

export default defineConfig({
  testDir: "e2e",
  timeout: 300_000,
  fullyParallel: false,
  workers: 1,
  reporter: [["list"]],
  use: {
    baseURL: external ?? "http://localhost:4174",
    viewport: { width: 1440, height: 900 },
    browserName: firefox ? "firefox" : "chromium",
    launchOptions: firefox ? {} : {
      // Default: software WebGL (works everywhere, slow). PW_GPU=1 uses the
      // machine's GPU via Direct3D (Windows) for realistic measurements.
      args: process.env.PW_GPU
        ? ["--use-angle=d3d11", "--enable-gpu", "--ignore-gpu-blocklist"]
        : ["--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"],
    },
  },
  webServer: external
    ? undefined
    : {
        command: "npx --no-install serve out -l 4174",
        url: "http://localhost:4174",
        reuseExistingServer: false,
        timeout: 60_000,
      },
});
