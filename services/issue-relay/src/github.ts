// GitHub as the AnyVector Reports app (a GitHub App with Issues: read and write on toim22/AnyVector only): a JWT
// signed with the app's private key gets a one-hour installation token, which creates the issue. The token is kept
// for the next reports while it is valid.

const API = "https://api.github.com";
const HEADERS = {
  Accept: "application/vnd.github+json",
  "X-GitHub-Api-Version": "2022-11-28",
  "User-Agent": "anyvector-issue-relay",
};

export interface AppCredentials {
  appId: string;
  /** the .pem GitHub gives (PKCS#1, "BEGIN RSA PRIVATE KEY") or PKCS#8; line breaks may be "\n" or spaces */
  privateKey: string;
  repo: string;
}

function base64url(data: ArrayBuffer | Uint8Array | string): string {
  const bytes = typeof data === "string" ? new TextEncoder().encode(data) : new Uint8Array(data);
  let s = "";
  for (const b of bytes) s += String.fromCharCode(b);
  return btoa(s).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

/** The DER length bytes of an ASN.1 value. */
function derLength(n: number): number[] {
  if (n < 0x80) return [n];
  const out: number[] = [];
  while (n > 0) {
    out.unshift(n & 0xff);
    n >>= 8;
  }
  return [0x80 | out.length, ...out];
}

/** PKCS#1 RSAPrivateKey → PKCS#8 PrivateKeyInfo (WebCrypto reads only PKCS#8). */
export function pkcs1ToPkcs8(pkcs1: Uint8Array): Uint8Array {
  // version 0, AlgorithmIdentifier { rsaEncryption, NULL }
  const head = [0x02, 0x01, 0x00, 0x30, 0x0d, 0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01, 0x05, 0x00];
  const octet = [0x04, ...derLength(pkcs1.length)];
  const inner = head.length + octet.length + pkcs1.length;
  const out = new Uint8Array([0x30, ...derLength(inner), ...head, ...octet, ...pkcs1]);
  return out;
}

export function pemToPkcs8(pem: string): Uint8Array {
  const text = pem.replace(/\\n/g, "\n");
  const pkcs1 = /BEGIN RSA PRIVATE KEY/.test(text);
  const b64 = text.replace(/-----(BEGIN|END)[^-]+-----/g, "").replace(/\s+/g, "");
  const der = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
  return pkcs1 ? pkcs1ToPkcs8(der) : der;
}

async function appJwt(c: AppCredentials): Promise<string> {
  const key = await crypto.subtle.importKey("pkcs8", pemToPkcs8(c.privateKey), { name: "RSASSA-PKCS1-v1_5", hash: "SHA-256" },
                                            false, ["sign"]);
  const now = Math.floor(Date.now() / 1000);
  // a minute in the past for clock drift; GitHub accepts at most ten minutes
  const unsigned = `${base64url(JSON.stringify({ alg: "RS256", typ: "JWT" }))}.${base64url(JSON.stringify({ iat: now - 60, exp: now + 540, iss: c.appId }))}`;
  const sig = await crypto.subtle.sign("RSASSA-PKCS1-v1_5", key, new TextEncoder().encode(unsigned));
  return `${unsigned}.${base64url(sig)}`;
}

let cached: { token: string; expires: number } | null = null;

async function installationToken(c: AppCredentials): Promise<string> {
  if (cached && cached.expires - Date.now() > 5 * 60 * 1000) return cached.token;
  const jwt = await appJwt(c);
  const auth = { ...HEADERS, Authorization: `Bearer ${jwt}` };
  const inst = await fetch(`${API}/repos/${c.repo}/installation`, { headers: auth });
  if (!inst.ok) throw new Error(`installation lookup: ${inst.status} ${await inst.text()}`);
  const { id } = (await inst.json()) as { id: number };
  const tok = await fetch(`${API}/app/installations/${id}/access_tokens`, { method: "POST", headers: auth });
  if (!tok.ok) throw new Error(`installation token: ${tok.status} ${await tok.text()}`);
  const t = (await tok.json()) as { token: string; expires_at: string };
  cached = { token: t.token, expires: Date.parse(t.expires_at) };
  return t.token;
}

/** Creates the issue; its address. A label the repository lacks is dropped rather than failing the report. */
export async function createIssue(c: AppCredentials, issue: { title: string; body: string; labels: string[] }): Promise<string> {
  const token = await installationToken(c);
  const post = (payload: object) =>
    fetch(`${API}/repos/${c.repo}/issues`, {
      method: "POST",
      headers: { ...HEADERS, Authorization: `Bearer ${token}`, "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    });
  let r = await post(issue);
  if (r.status === 422) r = await post({ title: issue.title, body: issue.body });
  if (r.status === 401) cached = null; // revoked or expired early: a new token next time
  if (!r.ok) throw new Error(`create issue: ${r.status} ${await r.text()}`);
  const { html_url } = (await r.json()) as { html_url: string };
  return html_url;
}

/** The pre-release that holds the files sent with reports. GitHub has no API for the files people drag into an
 *  issue, so the relay keeps them as assets of this release; `/releases/latest` (AnyVector's update check) skips it. */
export const ATTACHMENTS_TAG = "report-attachments";

async function attachmentsRelease(c: AppCredentials, token: string): Promise<number> {
  const auth = { ...HEADERS, Authorization: `Bearer ${token}` };
  const got = await fetch(`${API}/repos/${c.repo}/releases/tags/${ATTACHMENTS_TAG}`, { headers: auth });
  if (got.ok) return ((await got.json()) as { id: number }).id;
  if (got.status !== 404) throw new Error(`attachments release: ${got.status} ${await got.text()}`);
  const made = await fetch(`${API}/repos/${c.repo}/releases`, {
    method: "POST",
    headers: { ...auth, "Content-Type": "application/json" },
    body: JSON.stringify({
      tag_name: ATTACHMENTS_TAG,
      name: "Report attachments (not a release)",
      body: "Files sent with bug reports and ideas from AnyVector (Help ▸ Report a Bug). Each issue links to its file. This is not a version of AnyVector.",
      prerelease: true,
      make_latest: "false",
    }),
  });
  if (!made.ok) throw new Error(`create attachments release: ${made.status} ${await made.text()}`);
  return ((await made.json()) as { id: number }).id;
}

/** Uploads the file as an asset of the attachments release; its id and download address. */
export async function uploadAttachment(c: AppCredentials, name: string, type: string, data: ArrayBuffer): Promise<{ id: number; url: string }> {
  const token = await installationToken(c);
  const release = await attachmentsRelease(c, token);
  const r = await fetch(`https://uploads.github.com/repos/${c.repo}/releases/${release}/assets?name=${encodeURIComponent(name)}`, {
    method: "POST",
    headers: { ...HEADERS, Authorization: `Bearer ${token}`, "Content-Type": type || "application/octet-stream" },
    body: data,
  });
  if (r.status === 401) cached = null;
  if (!r.ok) throw new Error(`upload attachment: ${r.status} ${await r.text()}`);
  const a = (await r.json()) as { id: number; browser_download_url: string };
  return { id: a.id, url: a.browser_download_url };
}

/** Removes an uploaded file whose issue could not be created. */
export async function deleteAttachment(c: AppCredentials, id: number): Promise<void> {
  const token = await installationToken(c);
  await fetch(`${API}/repos/${c.repo}/releases/assets/${id}`, { method: "DELETE", headers: { ...HEADERS, Authorization: `Bearer ${token}` } });
}
