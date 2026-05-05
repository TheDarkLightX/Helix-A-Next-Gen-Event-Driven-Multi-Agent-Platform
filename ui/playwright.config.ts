import { defineConfig, devices } from "@playwright/test";

const chromeExecutablePath = process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH;
const browserUse = chromeExecutablePath
  ? { launchOptions: { executablePath: chromeExecutablePath } }
  : { channel: process.env.PLAYWRIGHT_CHANNEL ?? "chrome" };

export default defineConfig({
  testDir: "./e2e",
  timeout: 30_000,
  expect: {
    timeout: 10_000,
  },
  fullyParallel: false,
  reporter: [["list"]],
  use: {
    baseURL: process.env.PLAYWRIGHT_BASE_URL ?? "http://127.0.0.1:4174",
    screenshot: "only-on-failure",
    trace: "on-first-retry",
    ...browserUse,
  },
  webServer: {
    command: "npm run dev -- --host 127.0.0.1 --port 4174",
    url: "http://127.0.0.1:4174",
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
    env: {
      VITE_API_BASE_URL: "http://127.0.0.1:3000",
    },
  },
  projects: [
    {
      name: "chrome",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
});
