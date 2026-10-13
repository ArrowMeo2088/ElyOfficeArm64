import { DOWNLOADS, expect, test, VERSION, type Page } from "./fixtures.ts";
import { TEXT, word, type Key, type Lang } from "../src/text.ts";

const REPO = "https://git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice";
const APPS = ["ElyDoc", "ElyTable", "ElyShow", "ElyPDF", "ElyTree", "ElyMark"];
const RELEASES = `${REPO}/releases/latest`;

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
  await expect(page.locator("[data-t=count]")).toBeVisible();
  const found = await keyed(page);
  expect(found.length).toBeGreaterThan(0);
  for (const [key, text] of found) {
    expect(text, key).not.toBe("");
    expect(text, key).toBe(word(lang, key as Key, key === "count" ? DOWNLOADS : undefined));
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

  test("downloads go to the release page, never to a file", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".tour .get")).toHaveCount(APPS.length);
    for (const i of APPS.keys()) await expect(page.locator(".tour .get").nth(i)).toHaveAttribute("href", RELEASES);
    await expect(page.locator("#download a")).toHaveCount(1);
    await expect(page.locator("#download [data-t=get]")).toHaveAttribute("href", RELEASES);
    await expect(page.locator(".hero .links a").last()).toHaveText("Source");
    await expect(page.locator(".hero .links a").last()).toHaveAttribute("href", REPO);
    await expect(page.locator(".hero [data-t=get]")).toHaveAttribute("href", RELEASES);
    const files = await page.locator("a[href]").evaluateAll((as) =>
      as.map((a) => (a as HTMLAnchorElement).href).filter((h) => /\.(dmg|tar\.gz|zip|exe|AppImage)$/.test(h)),
    );
    expect(files).toEqual([]);
  });

  test("the download area names the newest version and all downloads", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator("#download h2")).toHaveText(`Ely Office ${VERSION}`);
    const count = page.locator("#download [data-t=count]");
    await expect(count).toHaveText("12,345 downloads so far.");
    await page.locator(".lang").click();
    await expect(count).toHaveText("已下载 12,345 次。");
  });

  test("a failed version read shows as an error", async ({ page }) => {
    await page.route("**/release.json", (r) => r.fulfill({ status: 502, body: "Gitea answered 500" }));
    await page.goto("/");
    await expect.poll(() => errors.length).toBe(1);
    expect(errors[0]).toContain("Gitea answered 500");
    await expect(page.locator("#download h2")).toHaveText("Ely Office");
    await expect(page.locator("#download [data-t=count]")).toBeHidden();
    errors = [];
  });

  test("every external link answers @net", async ({ page, request }) => {
    await page.goto("/");
    const hrefs = await page.locator("a[href^='http']").evaluateAll((as) => as.map((a) => (a as HTMLAnchorElement).href));
    const unique = [...new Set(hrefs)];
    expect(unique.sort()).toEqual([REPO, RELEASES].sort());
    for (const url of unique) {
      const res = await request.head(url);
      expect(res.status(), url).toBeLessThan(400);
    }
  });

  test("each app's copy arrives at its anchor; its download shows on intent", async ({ page }) => {
    for (const [i, app] of APPS.entries()) {
      await page.goto(`/#${app.toLowerCase()}`);
      const article = page.locator(".copy article").nth(i);
      await expect(article).toHaveClass(/current/);
      await expect(page.locator(".copy article.current")).toHaveCount(1);
      await expect(page.locator(".rail a").nth(i)).toHaveAttribute("aria-current", "step");
      const get = article.locator(".get");
      await page.mouse.move(1000, 5);
      await expect(get).toHaveCSS("opacity", "0");
      await article.locator("h2").hover();
      await expect(get).toHaveCSS("opacity", "1");
      await page.mouse.move(1000, 5);
      await expect(get).toHaveCSS("opacity", "0");
      await get.focus();
      await expect(get).toHaveCSS("opacity", "1");
      await get.blur();
    }
  });
});

test("the download area tells how to open an unnotarized app", async ({ browser }) => {
  for (const [locale, words] of [
    ["en-US", ["System Settings", "Privacy & Security", "Open Anyway"]],
    ["zh-CN", ["系统设置", "隐私与安全性", "仍要打开"]],
  ] as const) {
    const page = await browser.newPage({ locale, reducedMotion: "reduce" });
    await page.goto("/");
    const steps = page.locator("#download [data-t=unsigned]");
    await expect(steps).toBeVisible();
    for (const word of words) await expect(steps).toContainText(word);
    await page.locator(".lang").click();
    await expect(steps).toBeVisible();
    await expect(steps).not.toContainText(words[0]);
    await page.close();
  }
});
