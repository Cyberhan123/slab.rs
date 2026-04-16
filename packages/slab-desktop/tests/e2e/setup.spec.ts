import { expect, test } from "@playwright/test";

const API_BASE_URL = process.env.PLAYWRIGHT_API_BASE_URL ?? "http://127.0.0.1:3300";

type SetupStatus = {
  initialized: boolean;
  runtime_payload_installed: boolean;
  ffmpeg: {
    name: string;
    installed: boolean;
    version?: string | null;
  };
  backends: Array<{
    name: string;
    installed: boolean;
    version?: string | null;
  }>;
};

async function fetchSetupStatus(request: Parameters<typeof test>[0]["request"]): Promise<SetupStatus> {
  const response = await request.get(`${API_BASE_URL}/v1/setup/status`);
  expect(response.ok()).toBeTruthy();
  return (await response.json()) as SetupStatus;
}

async function setSetupInitialized(
  request: Parameters<typeof test>[0]["request"],
  initialized: boolean,
) {
  const response = await request.post(`${API_BASE_URL}/v1/setup/complete`, {
    data: { initialized },
  });
  expect(response.ok()).toBeTruthy();
}

async function expectSetupInitialized(
  request: Parameters<typeof test>[0]["request"],
  initialized: boolean,
) {
  await expect
    .poll(async () => {
      const status = await fetchSetupStatus(request);
      return status.initialized;
    })
    .toBe(initialized);
}

function ffmpegRow(page: Parameters<typeof test>[0]["page"]) {
  return page
    .locator("div")
    .filter({
      has: page.getByText("Ffmpeg", { exact: true }),
    })
    .filter({
      has: page.getByText("Core Media Engine", { exact: true }),
    })
    .first();
}

test.beforeEach(async ({ request }) => {
  await setSetupInitialized(request, false);
});

test("redirects incomplete users from the app shell to setup", async ({ page }) => {
  await page.goto("/");

  await page.waitForURL("**/setup");
  await expect(page.getByText("System Dependencies", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Continue to App" })).toBeVisible();
});

test("renders the dependency status reported by slab-server", async ({ page, request }) => {
  const status = await fetchSetupStatus(request);
  expect(status.backends.some((backend) => backend.installed)).toBe(true);

  await page.goto("/setup");

  const row = ffmpegRow(page);
  await expect(row).toBeVisible();

  if (status.ffmpeg.installed) {
    await expect(row.getByText("Installed", { exact: true }).first()).toBeVisible();
  } else {
    await expect(row.getByRole("button", { name: "Download" })).toBeVisible();
  }

  if (status.backends.every((backend) => !backend.installed)) {
    await expect(
      page.getByText(/AI backends can be added later from Settings -> Backends/i),
    ).toBeVisible();
  }
});

test("marks setup complete from the UI and persists it on the server", async ({
  page,
  request,
}) => {
  await page.goto("/setup");
  await page.getByRole("button", { name: "Continue to App" }).click();

  await page.waitForURL((url) => url.pathname === "/");

  const status = await fetchSetupStatus(request);
  expect(status.initialized).toBe(true);
});

test("redirects away from setup after the server reports initialization complete", async ({
  page,
  request,
}) => {
  await setSetupInitialized(request, true);
  await expectSetupInitialized(request, true);

  const statusResponsePromise = page.waitForResponse((response) =>
    response.url().endsWith("/v1/setup/status") && response.request().method() === "GET",
  );

  await page.goto("/setup");
  const statusResponse = await statusResponsePromise;
  expect(statusResponse.ok()).toBeTruthy();
  expect(((await statusResponse.json()) as SetupStatus).initialized).toBe(true);
  await page.waitForURL((url) => url.pathname === "/");

  const status = await fetchSetupStatus(request);
  expect(status.initialized).toBe(true);
});
