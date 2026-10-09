import { expect, test, type Page } from "@playwright/test";

const CELL = 28;
const APPS = ["ElyDoc", "ElyTable", "ElyShow", "ElyPDF", "ElyTree", "ElyMark"];

test.use({ reducedMotion: "reduce" });

/** Screen top of the building's first block, in SVG pixels. */
const top = (page: Page, name: string) =>
  page.evaluate((n) => {
    const svg = document.querySelector<SVGSVGElement>(".hero .stage")!;
    const scale = svg.getBoundingClientRect().width / svg.viewBox.baseVal.width;
    return (document.querySelector(`.block[data-part="${n}"]`)!.getBoundingClientRect().top + scrollY) / scale;
  }, name);

/** A point where the building is hit; `low` takes the lowest. */
const spot = (page: Page, name: string, low = false) =>
  page.evaluate(
    ([n, low]) => {
      const faces = [...document.querySelectorAll(`.hit-face[data-part="${n}"]`)];
      const r = faces.map((f) => f.getBoundingClientRect());
      const box = {
        left: Math.min(...r.map((b) => b.left)),
        right: Math.max(...r.map((b) => b.right)),
        top: Math.min(...r.map((b) => b.top)),
        bottom: Math.max(...r.map((b) => b.bottom)),
      };
      const ys = [];
      for (let y = Math.floor(box.top); y < box.bottom; y += 2) ys.push(y);
      if (low) ys.reverse();
      for (const y of ys)
        for (let x = Math.floor(box.left); x < box.right; x += 2) {
          const hit = document.elementFromPoint(x, y);
          if (hit instanceof SVGElement && hit.dataset.part === n) return { x, y };
        }
      throw new Error(`${n} has no hit point`);
    },
    [name, low] as const,
  );

const caption = (page: Page, name: string) => page.locator(".caption", { hasText: name });

let errors: string[] = [];

test.beforeEach(async ({ page }) => {
  errors = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  await page.goto("/");
  await expect(page.locator(".hero .hit-face:not(.ground)").first()).toBeAttached();
});

test.afterEach(() => expect(errors).toEqual([]));

test("six links in reading order; reduced motion skips growth", async ({ page }) => {
  await expect(page.locator(".doors a")).toHaveText(APPS);
  const anim = await page.locator(".block").first().evaluate((b) => getComputedStyle(b).animationName);
  expect(anim).toBe("none");
});

test("focus lifts one cell, shows the name, survives the mouse", async ({ page }) => {
  const rest = await top(page, "ElyDoc");
  for (let i = 0; i < 3; i++) await page.keyboard.press("Tab");
  await expect(page.locator(".doors a").first()).toBeFocused();
  expect(rest - (await top(page, "ElyDoc"))).toBeCloseTo(CELL, 0);
  await expect(caption(page, "ElyDoc")).toHaveCSS("opacity", "1");
  const at = await spot(page, "ElyDoc");
  await page.mouse.move(at.x, at.y);
  await page.mouse.move(5, 5);
  expect(rest - (await top(page, "ElyDoc"))).toBeCloseTo(CELL, 0);
  await expect(caption(page, "ElyDoc")).toHaveCSS("opacity", "1");
});

test("hover at the bottom edge holds still", async ({ page }) => {
  const at = await spot(page, "ElyTable", true);
  await page.evaluate(() => {
    (window as unknown as { flips: number }).flips = 0;
    for (const face of document.querySelectorAll('.hit-face[data-part="ElyTable"]'))
      for (const kind of ["pointerenter", "pointerleave"])
        face.addEventListener(kind, (e) => {
          const to = (e as PointerEvent).relatedTarget;
          if (!(to instanceof SVGElement && to.dataset.part === "ElyTable")) (window as unknown as { flips: number }).flips++;
        });
  });
  await page.mouse.move(at.x, at.y);
  await page.waitForTimeout(600);
  expect(await page.evaluate(() => (window as unknown as { flips: number }).flips)).toBe(1);
  await expect(caption(page, "ElyTable")).toHaveCSS("opacity", "1");
});

test("the hit owner is the block you see", async ({ page }) => {
  const wrong = await page.evaluate(() => {
    const r = document.querySelector(".hero .stage")!.getBoundingClientRect();
    const out: string[] = [];
    const owner = (x: number, y: number) => {
      const face = document.elementsFromPoint(x, y).find((e) => e.classList.contains("face"));
      return face?.closest<SVGElement>(".block")!.dataset.part;
    };
    for (let y = r.top; y < r.bottom; y += 5)
      for (let x = r.left; x < r.right; x += 5) {
        const seen = owner(x, y);
        if (seen === undefined) continue;
        const edge = [[-1, 0], [1, 0], [0, -1], [0, 1]].some(([dx, dy]) => owner(x + dx, y + dy) !== seen);
        if (edge) continue;
        const hit = document.elementsFromPoint(x, y).find((e) => e.classList.contains("hit-face")) as SVGElement | undefined;
        if (hit?.dataset.part !== seen) out.push(`${x},${y}: ${hit?.dataset.part} over ${seen}`);
      }
    return out;
  });
  expect(wrong).toEqual([]);
});

test("a primary press sinks one cell; only a click starting on it opens", async ({ page }) => {
  const rest = await top(page, "ElyShow");
  const at = await spot(page, "ElyShow");
  await page.mouse.move(at.x, at.y);
  await page.mouse.down();
  expect(await top(page, "ElyShow")).toBeCloseTo(rest, 0);
  await page.mouse.up();
  await expect(page).toHaveURL(/#elyshow$/);

  await page.evaluate(() => history.replaceState(null, "", "/"));
  await page.evaluate(() => scrollTo(0, 0));
  const again = await spot(page, "ElyShow");
  await page.mouse.click(again.x, again.y, { button: "right" });
  await page.mouse.move(5, 5);
  await page.mouse.down();
  await page.mouse.move(again.x, again.y);
  await page.mouse.up();
  expect(await page.evaluate(() => location.hash)).toBe("");
});

test("holding Enter sinks the focused building until release", async ({ page }) => {
  const rest = await top(page, "ElyDoc");
  for (let i = 0; i < 3; i++) await page.keyboard.press("Tab");
  await page.keyboard.down("Enter");
  expect(await top(page, "ElyDoc")).toBeCloseTo(rest, 0);
  await expect(page).toHaveURL(/#elydoc$/);
  await page.keyboard.up("Enter");
  await expect(page.locator(".block.pressed")).toHaveCount(0);
});
