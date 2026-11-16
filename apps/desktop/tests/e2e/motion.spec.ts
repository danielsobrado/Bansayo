import { test, expect } from "@playwright/test";

test("calibration, live motion, faults, recording, replay and responsive workspaces", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto("/#/studio");
  await expect(
    page.getByText("Engine connected", { exact: false }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Calibrate mounting", exact: true })
    .click();
  await page.getByRole("button", { name: "Use reference motion" }).click();
  await page
    .getByRole("button", { name: "Capture mounting", exact: true })
    .click();
  await expect(
    page.getByText("Reference pose in 3…", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText("Mounting captured for 12 trackers.", { exact: true }),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).not.toBeVisible();
  await page.getByLabel("Motion", { exact: true }).selectOption("squat");
  await page.getByRole("button", { name: /Inspect Left knee/i }).click();
  await expect(page.getByTestId("selected-angle")).not.toHaveText("—");
  const firstAngle = await page.getByTestId("selected-angle").innerText();
  await expect
    .poll(() => page.getByTestId("selected-angle").innerText())
    .not.toBe(firstAngle);
  await page.getByRole("link", { name: /Devices/ }).click();
  const tracker = page.getByRole("article", {
    name: "Device SIM-T04",
    exact: true,
  });
  await tracker
    .getByRole("button", { name: "Disconnect", exact: true })
    .click();
  await expect(
    tracker.getByText("Disconnected", { exact: true }),
  ).toBeVisible();
  await page.getByRole("link", { name: /Studio/ }).click();
  await page.getByRole("button", { name: /Inspect Left knee/i }).click();
  await expect(page.getByTestId("selected-angle")).toHaveText("—");
  await page.getByRole("link", { name: /Devices/ }).click();
  await tracker.getByRole("button", { name: "Reconnect", exact: true }).click();
  await expect(tracker.getByText("Tracking", { exact: true })).toBeVisible();
  await page.getByRole("link", { name: /Studio/ }).click();
  await page.getByRole("button", { name: /Inspect Left knee/i }).click();
  await expect(page.getByTestId("selected-angle")).not.toHaveText("—");
  await page.getByRole("button", { name: "● Record", exact: true }).click();
  await page.getByLabel("Session name").fill("E2E motion evidence");
  await page
    .getByRole("button", { name: "Start recording", exact: true })
    .click();
  await expect(page.getByText(/REC/)).toBeVisible();
  await expect
    .poll(async () => {
      const text = await page.locator(".recording-indicator").innerText();
      return Number(/(\d+) events/.exec(text)?.[1] ?? 0);
    })
    .toBeGreaterThan(60);
  await page
    .getByRole("button", { name: "■ Stop recording", exact: true })
    .click();
  await page.getByRole("link", { name: /Sessions/ }).click();
  await page
    .getByRole("button", { name: "Replay E2E motion evidence", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "← Exit replay", exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Ⅱ Pause", exact: true }).click();
  const slider = page.getByRole("slider", { name: "Playback position" });
  await slider.focus();
  await page.keyboard.press("Home");
  await page
    .getByRole("button", { name: "← Exit replay", exact: true })
    .click();
  for (const width of [1440, 1024, 800]) {
    await page.setViewportSize({ width, height: 900 });
    for (const workspace of [
      "overview",
      "studio",
      "devices",
      "cameras",
      "sessions",
    ]) {
      await page.goto(`/#/${workspace}`);
      await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
      if (workspace === "cameras")
        await expect(
          page.getByRole("region", { name: "3D reconstruction" }),
        ).toBeVisible();
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth,
        ),
      ).toBe(true);
    }
    await page.goto("/#/studio");
    await page.screenshot({ path: `test-results/studio-${width}.png` });
  }
  expect(errors).toEqual([]);
});
