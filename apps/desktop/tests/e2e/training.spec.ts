import { test, expect, type APIRequestContext } from "@playwright/test";

// Martial-arts 18 §3 Phase 1: pick technique → run a set → summary → attempt → progress, on the
// simulator training rig. The simulator kicks only while the session is active.
const API = "http://127.0.0.1:17701/api/v1";

async function motion(request: APIRequestContext, m: string, speed = 1) {
  const r = await request.put(`${API}/simulation`, { data: { motion: m, speed } });
  expect(r.ok()).toBe(true);
}

test("pick a technique, run a set, review an attempt and see progress", async ({ page, request }) => {
  test.setTimeout(240_000);
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await motion(request, "standing");
  await page.goto("/#/train");
  await expect(page.getByText("Engine connected", { exact: false })).toBeVisible();

  // Athlete with motion-data consent.
  await page.getByLabel("Name", { exact: true }).fill("Ana");
  await page.getByRole("button", { name: "Create athlete", exact: true }).click();
  await expect(page.getByLabel("Active athlete")).toHaveValue("ana");

  // Pick: Taekwondo roundhouse kick, right, 1 × 3.
  await page.getByRole("combobox", { name: "Technique", exact: true }).selectOption("taekwondo.dollyeo_chagi");
  await page.getByLabel("Sets", { exact: true }).fill("1");
  await page.getByLabel("Repetitions", { exact: true }).fill("3");
  await page.getByLabel("Rest (s)", { exact: true }).fill("0");
  await page.getByRole("button", { name: "Prepare session", exact: true }).click();
  await expect(page).toHaveURL(/#\/train\/live$/);
  await expect(page.getByRole("region", { name: "Readiness" })).toBeVisible();
  // A new athlete acknowledges the technique's safety notes once (17 §4.3).
  await expect(page.getByRole("list", { name: "Safety notes" })).toContainText("Warm up");
  await page.getByRole("button", { name: "I have read the safety notes" }).click();
  const start = page.getByRole("button", { name: "▶ Start", exact: true });
  await expect(start).toBeEnabled({ timeout: 30_000 });
  await start.click();
  await expect(page.getByRole("heading", { level: 2, name: "active" })).toBeVisible({ timeout: 20_000 });
  await motion(request, "roundhouse_kick");
  await expect(page.getByTestId("rep-count")).toContainText("Rep 1 /", { timeout: 30_000 });
  await expect(page.getByRole("region", { name: "Last repetition" })).toBeVisible();
  await expect(page.getByRole("link", { name: "Open the session summary" })).toBeVisible({ timeout: 60_000 });
  await motion(request, "standing");
  await page.screenshot({ path: "test-results/train-live.png" });

  // Summary → attempt.
  await page.getByRole("link", { name: "Open the session summary" }).click();
  const reps = page.getByRole("table", { name: "Repetitions" });
  await expect(reps.getByRole("link", { name: /Open attempt/ })).toHaveCount(3, { timeout: 20_000 });
  await reps.getByRole("link", { name: "Open attempt 2" }).click();
  await expect(page.getByRole("heading", { level: 1 })).toContainText("Attempt 2");
  await expect(page.getByTestId("phase-timeline")).toBeVisible();
  await expect(page.getByTestId("metric-table")).toContainText("Total time");
  await expect(page.getByRole("toolbar", { name: "Clip player" })).toBeVisible({ timeout: 20_000 });
  await page.getByRole("button", { name: "▶ Play" }).click();
  await expect(page.getByRole("button", { name: "⏸ Pause" })).toBeVisible();
  await page.screenshot({ path: "test-results/technique-attempt.png" });

  // Progress: the simulated session appears as one series class.
  await page.getByRole("navigation", { name: "Workspace tabs" }).getByRole("link", { name: "Progress" }).click();
  await expect(page.getByText("SIMULATED").first()).toBeVisible();
  await page.getByRole("combobox", { name: "Technique", exact: true }).selectOption("taekwondo.dollyeo_chagi");
  await page.getByLabel("Metric", { exact: true }).selectOption("kick.total_time");
  await expect(page.getByTestId("series-class")).toHaveCount(1, { timeout: 20_000 });
  await page.screenshot({ path: "test-results/progress.png" });

  for (const width of [1440, 1024, 800]) {
    await page.setViewportSize({ width, height: 900 });
    for (const route of ["train", "technique", "progress"]) {
      await page.goto(`/#/${route}`);
      await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    }
  }
  expect(errors).toEqual([]);
});

async function activateAthlete(request: APIRequestContext, id: string, displayName: string) {
  for (const [path, data] of [
    ["athletes", { id, display_name: displayName, height_m: 1.75 }],
    [`athletes/${id}/consent`, { motion_data: true }],
    ["athletes/active", { athlete_id: id }],
  ] as const) {
    const response = await request.fetch(`${API}/${path}`, { method: path === "athletes/active" ? "PUT" : "POST", data });
    expect(response.ok()).toBe(true);
  }
}

test("run and save a follow-along class from Train", async ({ page, request }) => {
  await activateAthlete(request, "robin", "Robin");
  const staleAthlete = await request.post(`${API}/training/sessions`, {
    data: { athlete_id: "other", plan: { kind: "workout", workout: "general.workout.full_body_in_20" } },
  });
  expect(staleAthlete.status()).toBe(409);
  await page.goto("/#/train");
  await expect(page.getByText("Engine connected", { exact: false })).toBeVisible();
  await expect(page.getByLabel("Active athlete")).toHaveValue("robin");
  const pick = page.getByRole("button", { name: "Full Body in 20", exact: true });
  await expect(pick).toBeVisible();
  await pick.click();
  const staleCommand = await request.post(`${API}/training/command`, {
    data: { session_id: "20261118T120000Z-abcd", command: "end" },
  });
  expect(staleCommand.status()).toBe(409);
  await expect(page.getByRole("heading", { level: 1, name: "Full Body in 20" })).toBeVisible();
  await expect(page.getByRole("region", { name: "Class outline" })).toContainText("Cooldown");
  await page.getByRole("button", { name: "Start class" }).click();
  await expect(page.getByText("Easier option", { exact: false })).toContainText("Keep the steps small");
  await page.screenshot({ path: "test-results/follow-along.png" });
  const track = page.locator("audio");
  await expect.poll(() => track.evaluate((a: HTMLAudioElement) => a.currentTime)).toBeGreaterThan(0);
  await page.getByRole("button", { name: "Mute music" }).click();
  await expect.poll(() => track.evaluate((a: HTMLAudioElement) => a.paused)).toBe(true);
  await expect(page.getByTestId("class-timer")).not.toHaveText("1:00");
  await page.getByRole("button", { name: "Pause class", exact: true }).click();
  const timer = page.getByTestId("class-timer");
  await expect(page.getByRole("status").filter({ hasText: "Paused" })).toBeVisible();
  const frozen = await timer.innerText();
  await page.waitForTimeout(1150);
  await expect(timer).toHaveText(frozen);
  await page.getByRole("button", { name: "Resume class", exact: true }).click();
  await expect(timer).not.toHaveText(frozen);
  await page.getByRole("button", { name: "End class" }).click();
  await page.getByRole("link", { name: "Open the session summary" }).click();
  await expect(page.getByRole("heading", { name: "Class ended early" })).toBeVisible();
  await expect(page.getByTestId("class-active-time")).not.toHaveText("0:00");
  const elapsed = await page.getByTestId("class-active-time").innerText();
  await page.reload();
  await expect(page.getByTestId("class-active-time")).toHaveText(elapsed);
});

for (const kick of [
  { id: "taekwondo.dollyeo_chagi", motion: "roundhouse_kick", speed: 1 },
  // The front-kick preview runs slowly at 1×. Accelerate the script for the normal-stage extension gate.
  { id: "taekwondo.ap_chagi", motion: "front_kick", speed: 1.25 },
]) {
  test(`run a two-set Taekwondo clinic and compare ${kick.motion} attempts`, async ({ page, request }) => {
    test.setTimeout(120_000);
    const athlete = `clinic_${kick.motion}`;
    await activateAthlete(request, athlete, "Jo");
    await motion(request, "standing");
    await page.goto("/#/train");
    await expect(page.getByLabel("Active athlete")).toHaveValue(athlete);
    await page.getByRole("combobox", { name: "Technique", exact: true }).selectOption(kick.id);
    const side = page.getByRole("combobox", { name: "Side", exact: true });
    const clinic = page.getByRole("button", { name: "Prepare Taekwondo clinic" });
    await side.selectOption("alternate");
    await expect(clinic).toBeDisabled();
    await side.selectOption("right");
    await clinic.click();
    await expect(page.getByRole("list", { name: "Movement phases" })).toContainText("Retraction");
    await page.getByRole("button", { name: "I have read the safety notes" }).click();
    const start = page.getByRole("button", { name: "▶ Start", exact: true });
    await expect(start).toBeEnabled({ timeout: 20_000 });
    await start.click();
    await expect(page.getByRole("heading", { level: 2, name: "active" })).toBeVisible();
    await expect(page.getByTestId("set-count")).toHaveText("Set 1 / 2");
    await expect(page.getByTestId("rep-count")).toHaveText("Rep 0 / 4");

    await motion(request, kick.motion, kick.speed);
    const rest = page.getByRole("region", { name: "Set summary" });
    await expect(rest).toContainText("Set 1 complete", { timeout: 30_000 });
    await expect(rest).toContainText("Next set in");
    await expect(rest).toContainText(/Focus for the next set|No correction was available/);
    await motion(request, "standing");
    await page.getByRole("button", { name: "⏭ Skip rest" }).click();
    await expect(page.getByTestId("set-count")).toHaveText("Set 2 / 2");
    await motion(request, kick.motion, kick.speed);
    await page.getByRole("link", { name: "Open the session summary" }).click({ timeout: 30_000 });
    await motion(request, "standing");

    const comparison = page.getByRole("region", { name: "Set comparison" });
    const attempt = comparison.getByRole("link", { name: /^Set 2 · Attempt/ });
    await expect(attempt).toBeVisible();
    await expect(comparison.getByRole("heading")).toContainText("right");
    await expect(comparison.getByRole("row", { name: /Retraction time/i })).toContainText("Estimated");
    await attempt.click();
    await expect(page.getByTestId("phase-timeline")).toBeVisible();
    await expect(comparison.getByRole("link", { name: /^Set 1 · Attempt/ })).toBeVisible();
    await page.setViewportSize({ width: 800, height: 900 });
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await page.goto("/#/train/history");
    await expect(page.getByRole("heading", { name: "Training history" })).toBeVisible();
    await page.getByRole("region", { name: "Saved sessions" }).getByRole("link", { name: /kick/i }).click();
    await expect(comparison).toBeVisible();
  });
}
