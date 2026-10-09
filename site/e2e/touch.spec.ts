import { expect, test, type Page } from "@playwright/test";

const CELL = 28;
const APPS = ["ElyDoc", "ElyTable", "ElyShow", "ElyPDF", "ElyTree", "ElyMark"];

test.use({ reducedMotion: "reduce" });

/** Screen top of the building's first block, in SVG pixels. */
const top = (page: Page, name: string) =>
  page.evaluate((n) => {
    const svg = document.querySelector<SVGSVGElement>(".stage")!;
    const scale = svg.getBoundingClientRect().width / svg.viewBox.baseVal.width;
    return document.querySelector(`.block[data-part="${n}"]`)!.getBoundingClientRect().top / scale;
  }, name);

const centre = async (page: Page, name: string) => {
  const box = await page.locator(`.hit[data-part="${name}"]`).boundingBox();
  if (!box) throw new Error(`${name} has no hit area`);
  return { x: box.x + box.width / 2, y: box.y + box.height * 0.75 };
};

let errors: string[] = [];

test.beforeEach(async ({ page }) => {
  errors = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  await page.goto("/");
  await expect(page.locator(".hit")).toHaveCount(6);
});

test.afterEach(() => expect(errors).toEqual([]));

test("six links in reading order; reduced motion skips growth", async ({ page }) => {
  await expect(page.locator(".doors a")).toHaveText(APPS);
  const anim = await page.locator(".block").first().evaluate((b) => getComputedStyle(b).animationName);
  expect(anim).toBe("none");
});

test("focus lifts one cell and survives the mouse passing", async ({ page }) => {
  const rest = (await top(page, "ElyDoc"));
  await page.keyboard.press("Tab");
  await expect(page.locator(".doors a").first()).toBeFocused();
  expect(rest - (await top(page, "ElyDoc"))).toBeCloseTo(CELL, 0);
  await expect(page.locator(".caption.shown")).toHaveText("ElyDoc");
  const at = await centre(page, "ElyDoc");
  await page.mouse.move(at.x, at.y);
  await page.mouse.move(5, 5);
  expect(rest - (await top(page, "ElyDoc"))).toBeCloseTo(CELL, 0);
  await expect(page.locator(".caption.shown")).toHaveText("ElyDoc");
});

test("hover at the bottom edge holds still", async ({ page }) => {
  const box = (await page.locator('.hit[data-part="ElyTable"]').boundingBox())!;
  await page.evaluate(() => {
    (window as unknown as { flips: number }).flips = 0;
    const hit = document.querySelector('.hit[data-part="ElyTable"]')!;
    for (const kind of ["pointerenter", "pointerleave"])
      hit.addEventListener(kind, () => (window as unknown as { flips: number }).flips++);
  });
  await page.mouse.move(box.x + box.width / 2, box.y + box.height - 2);
  await page.waitForTimeout(600);
  expect(await page.evaluate(() => (window as unknown as { flips: number }).flips)).toBe(1);
  await expect(page.locator(".caption.shown")).toHaveText("ElyTable");
});

test("a primary press sinks one cell; only a click starting on it opens", async ({ page }) => {
  const rest = (await top(page, "ElyShow"));
  const at = await centre(page, "ElyShow");
  await page.mouse.move(at.x, at.y);
  await page.mouse.down();
  expect((await top(page, "ElyShow"))).toBeCloseTo(rest, 0);
  await page.mouse.up();
  expect(rest - (await top(page, "ElyShow"))).toBeCloseTo(CELL, 0);
  await expect(page).toHaveURL(/#elyshow$/);

  await page.evaluate(() => (location.hash = ""));
  await page.mouse.click(at.x, at.y, { button: "right" });
  await page.mouse.move(5, 5);
  await page.mouse.down();
  await page.mouse.move(at.x, at.y);
  await page.mouse.up();
  expect(await page.evaluate(() => location.hash)).toBe("");
});

test("holding Enter sinks the focused building until release", async ({ page }) => {
  const rest = (await top(page, "ElyDoc"));
  await page.keyboard.press("Tab");
  await page.keyboard.down("Enter");
  expect((await top(page, "ElyDoc"))).toBeCloseTo(rest, 0);
  await expect(page).toHaveURL(/#elydoc$/);
  await page.keyboard.up("Enter");
  expect(rest - (await top(page, "ElyDoc"))).toBeCloseTo(CELL, 0);
});
