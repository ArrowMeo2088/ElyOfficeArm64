// Every word on the page, in both languages.
export type Lang = "en" | "zh";

const en = {
  tagline: "Six desktop apps. They work offline.",
  ElyDoc: "Letters, reports, books. Opens and saves Word files.",
  ElyTable: "Formulas, filters, charts. Opens and saves Excel files.",
  ElyShow: "Slides, animation, a presenter view. Opens PowerPoint files.",
  ElyPDF: "Read, mark up, sign, fill forms. Finds text in scans.",
  ElyTree: "Stage lines, commit, read the graph. Git without a terminal.",
  ElyMark: "Markdown as you read it. Math and diagrams in place.",
  offline: "Works with the network off.",
  open: "Open source, under the AGPL.",
  local: "Your files stay in your folders.",
  download: "Download",
  platforms: "Out now for macOS. Windows and Linux are coming soon.",
  needs: "macOS 14 or later, Apple silicon.",
  unsigned: "Not notarized. The first time, Control-click the app and choose Open.",
  source: "Source code",
  shot: "screenshot",
  other: "中文",
};

const zh: typeof en = {
  tagline: "六个桌面应用，断网也能用。",
  ElyDoc: "信件、报告、书稿。打开并保存 Word 文件。",
  ElyTable: "公式、筛选、图表。打开并保存 Excel 文件。",
  ElyShow: "幻灯片、动画、演讲者视图。打开 PowerPoint 文件。",
  ElyPDF: "阅读、批注、签名、填表。扫描件里也能查字。",
  ElyTree: "按行暂存、提交、看提交图。不用终端的 Git。",
  ElyMark: "写的就是看到的 Markdown。公式与图表就地显示。",
  offline: "断网照常工作。",
  open: "开源，AGPL 许可。",
  local: "文件留在你自己的文件夹里。",
  download: "下载",
  platforms: "macOS 版已发布。Windows 与 Linux 版即将推出。",
  needs: "macOS 14 或更新，Apple 芯片。",
  unsigned: "未经公证。第一次打开时，按住 Control 点按应用，选“打开”。",
  source: "源代码",
  shot: "截图",
  other: "English",
};

export type Key = keyof typeof en;
export const TEXT: Record<Lang, typeof en> = { en, zh };

/** An element whose text follows the language. */
export function said<K extends keyof HTMLElementTagNameMap>(tag: K, key: Key): HTMLElementTagNameMap[K] {
  const el = document.createElement(tag);
  el.dataset.t = key;
  return el;
}

/** Writes every keyed text in `lang`. */
export function speak(lang: Lang): void {
  document.documentElement.lang = lang === "zh" ? "zh-CN" : "en";
  for (const el of document.querySelectorAll<HTMLElement>("[data-t]")) {
    el.textContent = TEXT[lang][el.dataset.t as Key];
  }
  for (const el of document.querySelectorAll<SVGElement>("[data-shot]")) {
    el.setAttribute("aria-label", `${el.dataset.shot} ${TEXT[lang].shot}`);
  }
}
