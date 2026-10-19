import { afterEach, test } from "node:test";
import assert from "node:assert/strict";
import worker from "../worker/index.ts";

const real = globalThis.fetch;
afterEach(() => {
  globalThis.fetch = real;
});

const github = {
  tag_name: "v0.1.9",
  assets: [
    { name: "ElyDoc-0.1.9-arm64.dmg", download_count: 3 },
    { name: "ely-doc.json", download_count: 50 },
  ],
};
const gitea = [
  { tag_name: "v0.1.8", assets: [{ name: "ElyDoc-0.1.8-arm64.dmg", download_count: 40 }, { name: "ely-doc.json", download_count: 900 }] },
  { tag_name: "v0.1.7", assets: [{ name: "ElyDoc-0.1.7-arm64.dmg", download_count: 7 }] },
];

/** Answers GitHub with `github` and Gitea with `old`. */
function hosts(old: () => Response) {
  globalThis.fetch = (async (input: RequestInfo | URL) => {
    const url = String(input);
    if (url.startsWith("https://git.zacharyzhang.com/")) return old();
    if (url.endsWith("/releases/latest")) return Response.json(github);
    return Response.json([github]);
  }) as typeof fetch;
}

async function answer() {
  const warn = console.warn;
  console.warn = () => {};
  try {
    const res = await worker.fetch(new Request("https://office.elygpui.com/release.json"), {});
    assert.equal(res.status, 200);
    return await res.json();
  } finally {
    console.warn = warn;
  }
}

test("downloads add Gitea's history to GitHub's, without manifests", async () => {
  hosts(() => Response.json(gitea));
  assert.deepEqual(await answer(), { version: "0.1.9", downloads: 3 + 40 + 7 });
});

test("an unreachable, failing or odd Gitea leaves GitHub's count", async () => {
  const cases: (() => Response)[] = [
    () => {
      throw new TypeError("fetch failed");
    },
    () => new Response("down", { status: 502 }),
    () => new Response("<html>", { status: 200 }),
    () => Response.json({ message: "not found" }),
    () => Response.json([{ tag_name: "v0.1.8", assets: [{ name: "ElyDoc.dmg" }] }]),
  ];
  for (const old of cases) {
    hosts(old);
    assert.deepEqual(await answer(), { version: "0.1.9", downloads: 3 });
  }
});
