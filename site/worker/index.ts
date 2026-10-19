// Answers /release.json with the newest release and downloads.
const REPO = "https://api.github.com/repos/ELY-Open-Source/ElyOffice";
const PAGE = 100;

type Cached = RequestInit & { cf: { cacheTtl: number; cacheEverything: boolean } };
type Asset = { name: string; download_count: number };
type Release = { tag_name: string; assets: Asset[] };
// A token lifts GitHub's limit for the shared egress addresses: `wrangler secret put GITHUB_TOKEN`.
type Env = { GITHUB_TOKEN?: string };

async function read<T>(env: Env, path: string): Promise<T> {
  const headers: Record<string, string> = {
    accept: "application/vnd.github+json",
    "user-agent": "ely-office-site",
    "x-github-api-version": "2022-11-28",
  };
  if (env.GITHUB_TOKEN) headers.authorization = `Bearer ${env.GITHUB_TOKEN}`;
  const init: Cached = { headers, cf: { cacheTtl: 300, cacheEverything: true } };
  const res = await fetch(`${REPO}${path}`, init);
  if (!res.ok) throw new Error(`GitHub answered ${res.status} for ${path}`);
  return (await res.json()) as T;
}

/** The newest release's version, without the v. */
async function version(env: Env): Promise<string> {
  const { tag_name: tag } = await read<Release>(env, "/releases/latest");
  if (!/^v\d+\.\d+\.\d+$/.test(tag)) throw new Error(`GitHub's latest tag is ${JSON.stringify(tag)}`);
  return tag.slice(1);
}

/** Downloads of every release; update manifests are checks, not downloads. */
async function downloads(env: Env): Promise<number> {
  let total = 0;
  for (let page = 1; ; page++) {
    const releases = await read<Release[]>(env, `/releases?per_page=${PAGE}&page=${page}`);
    for (const { assets } of releases) {
      for (const a of assets) {
        if (!Number.isInteger(a.download_count)) throw new Error(`${a.name} has no download count`);
        if (!a.name.endsWith(".json")) total += a.download_count;
      }
    }
    if (releases.length < PAGE) return total;
  }
}

export default {
  async fetch(_request: Request, env: Env): Promise<Response> {
    try {
      const [v, n] = await Promise.all([version(env), downloads(env)]);
      return Response.json({ version: v, downloads: n }, { headers: { "cache-control": "public, max-age=300" } });
    } catch (e) {
      return new Response(String(e), { status: 502 });
    }
  },
};
