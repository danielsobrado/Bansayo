import { test,expect } from "@playwright/test";
test("shared dialog and drawer contain focus, support Escape, and restore their trigger",async({page})=>{
  await page.goto("/#/overview");
  await page.evaluate(async()=>{const path="/tests/fixtures/primitives.tsx";const fixture=await import(path);fixture.mount();});
  const trigger=page.getByRole("button",{name:"Open dialog",exact:true});await trigger.click();
  const dialog=page.getByRole("dialog",{name:"Capture details"});await expect(dialog).toBeVisible();await expect(page.getByLabel("Height",{exact:true})).toBeFocused();
  await page.keyboard.press("Shift+Tab");await expect(dialog.getByRole("button",{name:"Close",exact:true})).toBeFocused();
  await page.keyboard.press("Tab");await expect(page.getByLabel("Height",{exact:true})).toBeFocused();
  await page.getByLabel("Side",{exact:true}).focus();await page.getByLabel("Side",{exact:true}).selectOption("right");await expect(page.getByLabel("Side",{exact:true})).toHaveValue("right");
  await page.screenshot({path:"test-results/phase0-dialog.png"});
  await page.keyboard.press("Escape");await expect(dialog).not.toBeVisible();await expect(trigger).toBeFocused();
  const drawerTrigger=page.getByRole("button",{name:"Open drawer",exact:true});await drawerTrigger.click();const drawer=page.getByRole("dialog",{name:"Input details"});await expect(drawer).toBeVisible();await expect(page.getByLabel("Device name",{exact:true})).toBeFocused();
  await page.screenshot({path:"test-results/phase0-drawer.png"});
  await page.keyboard.press("Escape");await expect(drawer).not.toBeVisible();await expect(drawerTrigger).toBeFocused();
  await expect(page.getByRole("button",{name:"Disabled action"})).toBeDisabled();
});
test("an initially open modal survives React strict effect replay",async({page})=>{
  await page.goto("/#/overview");await page.evaluate(async()=>{const path="/tests/fixtures/primitives.tsx";const fixture=await import(path);fixture.mount(true);});
  await expect(page.getByRole("dialog",{name:"Capture details"})).toBeVisible();
  await page.keyboard.press("Escape");await expect(page.getByRole("dialog",{name:"Capture details"})).not.toBeVisible();
});
