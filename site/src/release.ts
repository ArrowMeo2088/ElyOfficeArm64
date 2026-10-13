// Gitea redirects this to the newest release.
export const RELEASES = "https://git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice/releases/latest";

/** The newest release's version, from the worker. */
export async function latest(): Promise<string> {
  const res = await fetch("/release.json");
  if (!res.ok) throw new Error(`release.json: ${res.status} ${await res.text()}`);
  const { version } = (await res.json()) as { version: string };
  return version;
}
