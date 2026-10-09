import { expect, test, type Page } from "@playwright/test";
import { TEXT, type Key, type Lang } from "../src/text.ts";

const REPO = "https://github.com/ZacharyZhang-NY/ElyOffice";
const APPS = ["ElyDoc", "ElyTable", "ElyShow", "ElyPDF", "ElyTree", "ElyMark"];
const dmg = (app: string) => `${REPO}/releases/download/v0.1.0/${app}-0.1.0-arm64.dmg`;

test.use({ reducedMotion: "reduce" });

let errors: string[] = [];

test.beforeEach(async ({ page }) => {
  errors = [];
  page.on("pageerror", (e) => errors.push(String(e)));
});

test.afterEach(() => expect(errors).toEqual([]));

/** Each keyed element's key and text. */
const keyed = (page: Page) =>
  page.locator("[data-t]").evaluateAll((els) => els.map((e) => [(e as HTMLElement).dataset.t!, e.textContent!] as const));

/** Asserts every keyed element reads in `lang`. */
async function reads(page: Page, lang: Lang) {
  await expect(page.locator("html")).toHaveAttribute("lang", lang === "zh" ? "zh-CN" : "en");
  const found = await keyed(page);
  expect(found.length).toBeGreaterThan(0);
  for (const [key, text] of found) {
    expect(text, key).not.toBe("");
    expect(text, key).toBe(TEXT[lang][key as Key]);
  }
}

test.describe("Chinese locale", () => {
  test.use({ locale: "zh-CN" });

  test("opens in Chinese", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator("html")).toHaveAttribute("lang", "zh-CN");
    await expect(page.locator("[data-t=tagline]")).toHaveText(TEXT.zh.tagline);
    await reads(page, "zh");
  });
});

test.describe("English locale", () => {
  test.use({ locale: "en-US" });

  test("opens in English", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator("[data-t=tagline]")).toHaveText(TEXT.en.tagline);
    await reads(page, "en");
  });

  test("the language button hides until intent", async ({ page }) => {
    await page.goto("/");
    const lang = page.locator(".lang");
    await expect(lang).toHaveCSS("opacity", "0");
    await lang.hover();
    await expect(lang).toHaveCSS("opacity", "1");
    await page.evaluate(() => scrollTo(0, document.body.scrollHeight));
    await page.mouse.move(5, 500);
    await expect(lang).toHaveCSS("opacity", "0");
    await page.keyboard.press("Tab");
    await expect(lang).toBeFocused();
    await expect(lang).toHaveCSS("opacity", "1");
  });

  test("the language button switches every text and back", async ({ page }) => {
    await page.goto("/");
    const lang = page.locator(".lang");
    await expect(lang).toHaveText("中文");
    await lang.click();
    await reads(page, "zh");
    await expect(lang).toHaveText("English");
    await lang.click();
    await reads(page, "en");
    await expect(lang).toHaveText("中文");
  });

  test("links point at the release and the repository", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".region .get")).toHaveCount(APPS.length);
    for (const app of APPS) {
      await expect(page.locator(`#${app.toLowerCase()} .get`)).toHaveAttribute("href", dmg(app));
    }
    await expect(page.locator("#download ul a")).toHaveText([...APPS, TEXT.en.source]);
    for (const [i, app] of APPS.entries()) {
      await expect(page.locator("#download ul a").nth(i)).toHaveAttribute("href", dmg(app));
    }
    await expect(page.locator("[data-t=source]")).toHaveAttribute("href", REPO);
    await expect(page.locator(".hero > a")).toHaveText("GitHub");
    await expect(page.locator(".hero > a")).toHaveAttribute("href", REPO);
  });

  test("every external link answers @net", async ({ page, request }) => {
    await page.goto("/");
    const hrefs = await page.locator("a[href^='http']").evaluateAll((as) => as.map((a) => (a as HTMLAnchorElement).href));
    const unique = [...new Set(hrefs)];
    expect(unique.sort()).toEqual([REPO, ...APPS.map(dmg)].sort());
    for (const url of unique) {
      const res = await request.head(url);
      expect(res.status(), url).toBeLessThan(400);
    }
  });

  test("region downloads show on hover or focus", async ({ page }) => {
    await page.goto("/");
    for (const app of APPS) {
      const region = page.locator(`#${app.toLowerCase()}`);
      const get = region.locator(".get");
      await region.scrollIntoViewIfNeeded();
      await page.mouse.move(0, 0);
      await expect(get).toHaveCSS("opacity", "0");
      await region.locator("h2").hover();
      await expect(get).toHaveCSS("opacity", "1");
      await page.locator(".lang").hover();
      await expect(get).toHaveCSS("opacity", "0");
      await get.focus();
      await expect(get).toHaveCSS("opacity", "1");
      await get.blur();
    }
  });
});
