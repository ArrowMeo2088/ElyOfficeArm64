// Answers /release.json with the newest release.
const API = "https://git.zacharyzhang.com/api/v1/repos/ZacharyZhang-NY/ElyOffice/releases/latest";

type Cached = RequestInit & { cf: { cacheTtl: number; cacheEverything: boolean } };

export default {
  async fetch(): Promise<Response> {
    const init: Cached = {
      headers: { accept: "application/json", "user-agent": "ely-office-site" },
      cf: { cacheTtl: 300, cacheEverything: true },
    };
    const res = await fetch(API, init);
    if (!res.ok) return new Response(`Gitea answered ${res.status}`, { status: 502 });
    const { tag_name: tag } = (await res.json()) as { tag_name: unknown };
    if (typeof tag !== "string" || !/^v\d+\.\d+\.\d+$/.test(tag)) {
      return new Response(`Gitea's latest tag is ${JSON.stringify(tag)}`, { status: 502 });
    }
    return Response.json({ version: tag.slice(1) }, { headers: { "cache-control": "public, max-age=300" } });
  },
};
