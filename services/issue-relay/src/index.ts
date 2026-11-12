// anyvector-issues: lets AnyVector's Help › Report a Bug / Submit Ideas and Feedback file a GitHub issue
// without the user having a GitHub account. The programs POST the report here; the Worker checks it, limits how often
// one address may send, and creates the issue as the AnyVector Reports GitHub App (labels bug or enhancement).
//
//   POST /issues   {"kind":"bug"|"idea","title","text","more","expected","version","program","system"}
//                  or multipart/form-data with that JSON in the part "report" and one file in the part "file"
//                  (GitHub's attachment types and sizes, attachment.ts): the file is kept as an asset of the
//                  "report-attachments" release of ATTACHMENTS_REPO and the issue links to it (shows it, for an image)
//                  → 201 {"url":"https://github.com/toim22/AnyVector/issues/12"}
//                  → 400 {"error":"…"} (refused), 429 (too many), 502 (GitHub failed: the program opens the browser)
//   GET  /         → 200 "ok" (a check that it is up)

import { assetName, attachmentSection, checkAttachment, MAX_OTHER } from "./attachment.ts";
import { createIssue, deleteAttachment, uploadAttachment } from "./github.ts";
import { issueOf, parseReport } from "./report.ts";

interface RateLimiter {
  limit(o: { key: string }): Promise<{ success: boolean }>;
}

export interface Env {
  /** the GitHub App's ID (GitHub › Settings › Developer settings › GitHub Apps › the app › About) */
  GITHUB_APP_ID: string;
  /** the app's private key (.pem) */
  GITHUB_APP_PRIVATE_KEY: string;
  /** owner/name; set in cloudflare.config.ts */
  GITHUB_REPO: string;
  /** owner/name of the repository whose release keeps the attached files; GITHUB_REPO when empty */
  ATTACHMENTS_REPO?: string;
  /** 2 reports a minute per address */
  PER_MINUTE: RateLimiter;
}

const MAX_BODY = 32 * 1024;

function json(status: number, value: object): Response {
  return new Response(JSON.stringify(value), { status, headers: { "Content-Type": "application/json; charset=utf-8" } });
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    if (request.method === "GET" && url.pathname === "/") return new Response("ok");
    if (url.pathname !== "/issues") return json(404, { error: "not found" });
    if (request.method !== "POST") return json(405, { error: "POST only" });

    const ip = request.headers.get("CF-Connecting-IP") ?? "unknown";
    if (!(await env.PER_MINUTE.limit({ key: ip })).success) return json(429, { error: "too many reports, try again in a minute" });

    const length = Number(request.headers.get("Content-Length") ?? "0");
    const multipart = (request.headers.get("Content-Type") ?? "").toLowerCase().startsWith("multipart/form-data");
    if (length > (multipart ? MAX_BODY + MAX_OTHER + 64 * 1024 : MAX_BODY)) return json(413, { error: "too large" });
    let text: string;
    let file: File | null = null;
    if (multipart) {
      let form: FormData;
      try {
        form = await request.formData();
      } catch {
        return json(400, { error: "not multipart form data" });
      }
      const report = form.get("report");
      const part = form.get("file");
      if (typeof report !== "string") return json(400, { error: "the part \"report\" is missing" });
      text = report;
      if (part !== null) {
        if (typeof part === "string") return json(400, { error: "the part \"file\" is not a file" });
        const why = checkAttachment(part.name, part.size);
        if (why) return json(why.includes("larger") ? 413 : 400, { error: why });
        file = part;
      }
    } else {
      text = await request.text();
    }
    if (text.length > MAX_BODY) return json(413, { error: "too large" });
    let raw: unknown;
    try {
      raw = JSON.parse(text);
    } catch {
      return json(400, { error: "not JSON" });
    }
    const report = parseReport(raw);
    if (typeof report === "string") return json(400, { error: report });

    const app = { appId: env.GITHUB_APP_ID, privateKey: env.GITHUB_APP_PRIVATE_KEY, repo: env.GITHUB_REPO };
    let asset: { id: number; url: string } | null = null;
    try {
      let section = "";
      if (file) {
        asset = await uploadAttachment({ ...app, repo: env.ATTACHMENTS_REPO || env.GITHUB_REPO }, assetName(file.name, new Date(), crypto.randomUUID().slice(0, 6)), file.type, await file.arrayBuffer());
        section = attachmentSection(file.name, file.size, asset.url);
      }
      const issue = await createIssue(app, issueOf(report, section));
      console.log(`issue created: ${issue} (${report.kind}, ${report.program} ${report.version}${file ? `, file ${file.size} bytes` : ""})`);
      return json(201, { url: issue });
    } catch (e) {
      if (asset) await deleteAttachment({ ...app, repo: env.ATTACHMENTS_REPO || env.GITHUB_REPO }, asset.id).catch(() => {});
      console.error(`GitHub refused the report: ${e instanceof Error ? e.message : String(e)}`);
      return json(502, { error: "GitHub could not take the report now" });
    }
  },
};
