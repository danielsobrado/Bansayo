import { test, expect } from "@playwright/test";

test("calibration, live motion, faults, recording, replay and responsive workspaces", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto("/#/studio");
  const tabs = page.getByRole("navigation", { name: "Workspace tabs" });
  const explorer = page.getByRole("region", { name: "Scene explorer" });
  const inspectKnee = async () => {
    await explorer.getByLabel("Filter scene objects").fill("left knee");
    await explorer.getByRole("button", { name: /Left knee flexion/i }).click();
  };
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
  await inspectKnee();
  await expect(page.getByTestId("selected-angle")).not.toHaveText("—");
  const firstAngle = await page.getByTestId("selected-angle").innerText();
  await expect
    .poll(() => page.getByTestId("selected-angle").innerText())
    .not.toBe(firstAngle);
  await tabs.getByRole("link", { name: "Devices" }).click();
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
  await tabs.getByRole("link", { name: "Studio" }).click();
  await inspectKnee();
  await expect(page.getByTestId("selected-angle")).toHaveText("—");
  await tabs.getByRole("link", { name: "Devices" }).click();
  await tracker.getByRole("button", { name: "Reconnect", exact: true }).click();
  await expect(tracker.getByText("Tracking", { exact: true })).toBeVisible();
  await tabs.getByRole("link", { name: "Studio" }).click();
  await inspectKnee();
  await expect(page.getByTestId("selected-angle")).not.toHaveText("—");
  await page.getByRole("button", { name: "Record", exact: true }).click();
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
    .getByRole("button", { name: "Stop recording", exact: true })
    .click();
  await tabs.getByRole("link", { name: "Sessions" }).click();
  await page
    .getByRole("button", { name: "Replay E2E motion evidence", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "Exit replay", exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Pause", exact: true }).click();
  const slider = page.getByRole("slider", { name: "Playback position" });
  await slider.focus();
  await page.keyboard.press("Home");
  await page
    .getByRole("button", { name: "Exit replay", exact: true })
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

test("workbench docks, selection and themes preserve the viewport", async ({ page }) => {
  await page.goto("/#/studio");
  const canvas = page.getByRole("img", { name: /Motion skeleton/ });
  await expect(canvas).toBeVisible();
  await canvas.evaluate((node) => node.setAttribute("data-original-canvas", ""));
  const tabs = page.getByRole("navigation", { name: "Workspace tabs" });
  const explorer = page.getByRole("region", { name: "Scene explorer" });
  const inspector = page.getByRole("region", { name: "Properties inspector" });
  const storePath = "/src/engine/stores.ts";
  await page.getByLabel("Motion", { exact: true }).selectOption("standing");
  await page.getByRole("button", { name: "Front", exact: true }).click();
  await expect(page.getByText("No pose data")).toBeHidden();
  const knee = await canvas.evaluate(async (node, path) => {
    const { frameStore, skeletonStore } = await import(/* @vite-ignore */ path);
    const skeleton = skeletonStore.get();
    const index = skeleton?.segments.findIndex((segment: { id: string }) => segment.id === "left_shin");
    const point = frameStore.frame?.pose?.segments[index]?.position_m;
    if (!point || !(node instanceof HTMLCanvasElement)) throw new Error("Skeleton knee position is missing");
    const bounds = node.getBoundingClientRect();
    const scale = bounds.height / (2 * Math.tan(19 * Math.PI / 180) * (4.2 - point[2]));
    return { x: bounds.width / 2 + point[0] * scale, y: bounds.height / 2 - (point[1] - skeleton.height_m / 2) * scale };
  }, storePath);
  await canvas.click({ position: knee });
  await expect(explorer.getByRole("button", { name: "Left knee flexion" })).toHaveAttribute("aria-pressed", "true");
  await expect(inspector.getByTestId("selected-angle")).toBeVisible();
  await page.getByRole("button", { name: "3D", exact: true }).click();
  const right = page.getByRole("separator", { name: "Resize right panel" });
  const split = page.getByRole("separator", { name: "Resize scene explorer" });
  await right.press("ArrowLeft");
  await expect(right).toHaveAttribute("aria-valuenow", "336");
  const edge = await split.boundingBox();
  if (!edge) throw new Error("Scene explorer splitter is missing");
  await page.mouse.move(edge.x + edge.width / 2, edge.y + edge.height / 2);
  await page.mouse.down();
  await page.mouse.move(edge.x + edge.width / 2, edge.y + edge.height / 2 + 45);
  await page.mouse.up();
  await expect(split).not.toHaveAttribute("aria-valuenow", "42");
  await page.getByLabel("Interface theme").selectOption("silver");
  await page.getByLabel("Interface density").selectOption("comfortable");
  await expect(canvas).toHaveAttribute("data-original-canvas", "");
  await explorer.getByLabel("Filter scene objects").fill("SIM-CAM2");
  const camera = explorer.getByRole("button", { name: /SIM-CAM2/ });
  await camera.click();
  await expect(camera).toHaveAttribute("aria-pressed", "true");
  await expect(inspector.getByRole("heading", { name: "SIM-CAM2" })).toBeVisible();
  await expect(inspector.getByText("1280 × 720")).toBeVisible();
  await page.getByLabel("Layout preset").selectOption("focus");
  await expect(inspector).toBeHidden();
  await expect(canvas).toHaveAttribute("data-original-canvas", "");
  await page.getByRole("button", { name: "Explorer and inspector" }).click();
  await expect(inspector.getByRole("heading", { name: "SIM-CAM2" })).toBeVisible();
  await page.getByLabel("Layout preset").selectOption("analysis");
  await expect(right).toHaveAttribute("aria-valuenow", "320");
  await right.press("ArrowLeft");
  await page.screenshot({ path: "test-results/workbench-studio-silver.png" });

  await tabs.getByRole("link", { name: "Cameras" }).click();
  await expect(right).toHaveAttribute("aria-valuenow", "340");
  const reconstruction = page.getByRole("img", { name: /Triangulated 3D landmarks/ });
  await expect(reconstruction).toBeVisible();
  await reconstruction.evaluate((node) => node.setAttribute("data-original-canvas", ""));
  await explorer.getByRole("button", { name: /SIM-CAM2/ }).click();
  await expect(page.getByLabel("Active camera")).toHaveValue("SIM-CAM2");
  await explorer.getByLabel("Filter scene objects").fill("left shin");
  await explorer.getByRole("button", { name: "left shin" }).click();
  await expect(inspector.getByRole("heading", { name: "left shin" })).toBeVisible();
  await explorer.getByLabel("Filter scene objects").clear();
  const feed = page.getByRole("img", { name: /2D landmarks from SIM-CAM2/ });
  const head = await feed.evaluate(async (node, path) => {
    const { freshLandmarks, skeletonStore } = await import(/* @vite-ignore */ path);
    const index = skeletonStore.get()?.segments.findIndex((segment: { id: string }) => segment.id === "head");
    const point = freshLandmarks(performance.now()).get("SIM-CAM2")?.points[index];
    if (!point || !(node instanceof HTMLCanvasElement)) throw new Error("Camera head landmark is missing");
    const bounds = node.getBoundingClientRect();
    return { x: point.x_px * bounds.width / node.width, y: point.y_px * bounds.height / node.height };
  }, storePath);
  await feed.click({ position: head });
  await expect(inspector.getByRole("heading", { name: "head" })).toBeVisible();
  await expect(explorer.getByRole("button", { name: "head" })).toHaveAttribute("aria-pressed", "true");
  const modes = page.getByRole("group", { name: "Camera workspace view" });
  await modes.getByRole("button", { name: "Camera", exact: true }).click();
  await expect(reconstruction).toBeHidden();
  await modes.getByRole("button", { name: "Grid", exact: true }).click();
  await expect(page.getByRole("img", { name: /2D landmarks from SIM-CAM[12]/ })).toHaveCount(2);
  await page.getByRole("region", { name: "2D camera view SIM-CAM1" })
    .getByRole("button", { name: "SIM-CAM1" }).click();
  await expect(inspector.getByRole("heading", { name: "SIM-CAM1" })).toBeVisible();
  await modes.getByRole("button", { name: "Split", exact: true }).click();
  await expect(reconstruction).toHaveAttribute("data-original-canvas", "");
  await page.screenshot({ path: "test-results/workbench-cameras-silver.png" });

  await tabs.getByRole("link", { name: "Studio" }).click();
  await expect(right).toHaveAttribute("aria-valuenow", "336");
  await page.reload();
  await expect(page.getByLabel("Interface theme")).toHaveValue("silver");
  await expect(canvas).toBeVisible();
  await page.getByLabel("Interface theme").selectOption("onyx");
  await expect(canvas).toBeVisible();
  await page.setViewportSize({ width: 800, height: 600 });
  await expect(canvas).toBeVisible();
  await page.getByRole("button", { name: "Tools panel" }).click();
  const closeTools = page.getByRole("button", { name: "Close tools" });
  await expect(closeTools).toBeFocused();
  await page.getByRole("checkbox", { name: "Floor grid" }).uncheck();
  await closeTools.click();
  await expect(page.getByRole("button", { name: "Floor grid" })).toHaveAttribute("aria-pressed", "false");
  const toggle = page.getByRole("button", { name: "Explorer and inspector" });
  await expect(toggle).toHaveAttribute("aria-expanded", "false");
  await page.getByRole("button", { name: "Inspect joint angles" }).click();
  await expect(page.getByRole("button", { name: "Close scene explorer" })).toBeFocused();
  await page.keyboard.press("Shift+Tab");
  await expect(toggle).toBeFocused();
  await expect(explorer).toBeVisible();
  await expect(inspector.getByTestId("selected-angle")).toBeVisible();
  await explorer.getByLabel("Filter scene objects").click();
  await page.keyboard.press("Escape");
  await expect(explorer).toBeHidden();
  await expect(toggle).toBeFocused();
});

test("camera grid preserves preview height and toggles markers across mixed feeds", async ({ page }) => {
  const headers = { "access-control-allow-origin": "*" };
  await page.route("**/api/v1/cameras/LIVE-CAM/**", async (route) => {
    if (route.request().url().endsWith("/fiducials")) {
      await route.fulfill({ headers, json: { ok: true, data: { tags: [] } } });
    } else {
      await route.fulfill({ headers, status: 404 });
    }
  });
  await page.goto("/#/cameras");
  await expect(page.getByLabel("Active camera")).toBeVisible();
  await page.evaluate(async ([storePath, connectionPath]) => {
    const { systemStateStore } = await import(/* @vite-ignore */ storePath);
    const { engineConnection } = await import(/* @vite-ignore */ connectionPath);
    const state = systemStateStore.get();
    const camera = state?.devices.find((device: { descriptor: { kind: string } }) => device.descriptor.kind === "camera");
    if (!camera) throw new Error("The simulator did not provide a camera");
    engineConnection.stop();
    systemStateStore.set({
      ...state,
      devices: [
        { ...camera, descriptor: { ...camera.descriptor, id: "LIVE-CAM", origin: "live" } },
        ...Array.from({ length: 7 }, (_, i) => ({
          ...camera, descriptor: { ...camera.descriptor, id: `SIM-GRID${i + 1}` },
        })),
      ],
    });
  }, ["/src/engine/stores.ts", "/src/engine/connection.ts"]);

  await page.getByRole("button", { name: "Grid", exact: true }).click();
  const markers = page.getByRole("checkbox", { name: "AprilTag markers" });
  await Promise.all([
    page.waitForRequest("**/cameras/LIVE-CAM/fiducials"),
    markers.check(),
  ]);
  await page.getByLabel("Active camera").selectOption("SIM-GRID1");
  await expect(markers).toBeEnabled();
  await expect(markers).toBeChecked();
  await markers.uncheck();

  await page.setViewportSize({ width: 800, height: 600 });
  const feeds = page.getByRole("region", { name: /2D camera view/ });
  await expect(feeds).toHaveCount(8);
  const cards = await feeds.evaluateAll((sections) => sections.map((section) => {
    const card = section.getBoundingClientRect();
    const preview = section.querySelector(".camera-preview")!.getBoundingClientRect();
    const canvas = section.querySelector("canvas")!.getBoundingClientRect();
    return { top: card.top, bottom: card.bottom, previewTop: preview.top, previewBottom: preview.bottom,
      canvasTop: canvas.top, canvasBottom: canvas.bottom };
  }));
  for (const [index, card] of cards.entries()) {
    expect(card.canvasTop).toBeGreaterThanOrEqual(card.previewTop - 1);
    expect(card.canvasBottom).toBeLessThanOrEqual(card.previewBottom + 1);
    if (index > 0) expect(card.top).toBeGreaterThanOrEqual(cards[index - 1]!.bottom);
  }
  await feeds.last().getByRole("button", { name: "SIM-GRID7" }).click();
  await expect(page.getByLabel("Active camera")).toHaveValue("SIM-GRID7");
});
