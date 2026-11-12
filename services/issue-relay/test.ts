// Tests without network access and without GitHub. Use: npm test (deploy runs them first).
import assert from "node:assert/strict";
import { generateKeyPairSync } from "node:crypto";
import { assetName, attachmentSection, checkAttachment } from "./src/attachment.ts";
import { pemToPkcs8 } from "./src/github.ts";
import { defuse, issueOf, parseReport, type Report } from "./src/report.ts";

const good = { kind: "bug", title: "Crash", text: "It closed", more: "", version: "0.3.0", program: "AnyVector", system: "" };

// 1) What is refused before GitHub is called
{
  assert.equal(typeof parseReport(good), "object");
  assert.match(parseReport({ ...good, kind: "spam" }) as string, /kind/);
  assert.match(parseReport({ ...good, title: "  " }) as string, /required/);
  assert.match(parseReport({ ...good, text: "x".repeat(8001) }) as string, /text/);
  assert.match(parseReport({ ...good, version: "latest" }) as string, /version/);
  assert.match(parseReport({ ...good, program: "Other" }) as string, /program/);
  assert.match(parseReport({ ...good, title: 5 }) as string, /title/);
  assert.match(parseReport({ ...good, system: "x".repeat(601) }) as string, /system/);
  assert.equal(typeof parseReport({ ...good, version: "0.3.0-beta.1" }), "object");
  assert.equal(typeof parseReport("text"), "string");
}

// 2) The issue: AnyVector's headings, empty optional fields left out, the label
{
  const system = "- AnyVector: 0.3.0 (release build)\n- Windows: Microsoft Windows [Version 10.0.26200]";
  const bug = issueOf(parseReport({ ...good, more: "1. Open", expected: "It stays", system }) as Report);
  assert.deepEqual(bug.labels, ["bug"]);
  assert.equal(bug.body, [
    "### What happened", "", "It closed", "",
    "### Steps to reproduce", "", "1. Open", "",
    "### Expected behavior", "", "It stays", "",
    "### AnyVector version", "", "0.3.0", "",
    "### System information", "", system, "",
    "_Sent from AnyVector (Help ▸ Report a Bug), without a GitHub account._", "",
  ].join("\n"));
  const idea = issueOf(parseReport({ ...good, kind: "idea", expected: "left out" }) as Report);
  assert.deepEqual(idea.labels, ["enhancement"]);
  assert.equal(idea.body, [
    "### Idea or feedback", "", "It closed", "",
    "### AnyVector version", "", "0.3.0", "",
    "_Sent from AnyVector (Help ▸ Submit Ideas and Feedback), without a GitHub account._", "",
  ].join("\n"));
}

// 3) Mentions and closing keywords lose their effect
{
  assert.equal(defuse("ask @toim22"), "ask @​toim22");
  assert.equal(defuse("this fixes #3"), "this fixes #​3");
  assert.equal(defuse("e-mail a@b.com"), "e-mail a@​b.com");
}

// 4) The private key GitHub gives (PKCS#1) signs in WebCrypto after the conversion, also on one line with "\n"
{
  const { privateKey, publicKey } = generateKeyPairSync("rsa", { modulusLength: 2048 });
  const pem = privateKey.export({ type: "pkcs1", format: "pem" }).toString();
  assert.match(pem, /BEGIN RSA PRIVATE KEY/);
  for (const text of [pem, pem.trim().replace(/\r?\n/g, "\\n")]) {
    const key = await crypto.subtle.importKey("pkcs8", pemToPkcs8(text), { name: "RSASSA-PKCS1-v1_5", hash: "SHA-256" }, false, ["sign"]);
    const data = new TextEncoder().encode("header.payload");
    const sig = await crypto.subtle.sign("RSASSA-PKCS1-v1_5", key, data);
    const pub = await crypto.subtle.importKey("spki", publicKey.export({ type: "spki", format: "der" }),
                                              { name: "RSASSA-PKCS1-v1_5", hash: "SHA-256" }, false, ["verify"]);
    assert.ok(await crypto.subtle.verify("RSASSA-PKCS1-v1_5", pub, sig, data));
  }
  const pkcs8 = privateKey.export({ type: "pkcs8", format: "pem" }).toString();
  await crypto.subtle.importKey("pkcs8", pemToPkcs8(pkcs8), { name: "RSASSA-PKCS1-v1_5", hash: "SHA-256" }, false, ["sign"]);
}

// 5) Attachments: GitHub's types and sizes, a safe unique asset name, the issue section
{
  const MB = 1024 * 1024;
  assert.equal(checkAttachment("shot.PNG", 10 * MB), null);
  assert.match(checkAttachment("shot.png", 10 * MB + 1)!, /10 MB/);
  assert.match(checkAttachment("clip.mp4", 11 * MB)!, /10 MB/);
  assert.equal(checkAttachment("drawing.zip", 25 * MB), null);
  assert.match(checkAttachment("drawing.zip", 25 * MB + 1)!, /25 MB/);
  assert.match(checkAttachment("drawing.cdr", 100)!, /\.cdr/);
  assert.match(checkAttachment("setup.exe", 100)!, /\.exe/);
  assert.match(checkAttachment("noext", 100)!, /type/);
  assert.match(checkAttachment("a.txt", 0)!, /empty/);
  assert.equal(assetName("My drawing (1).zip", new Date("2026-11-12T10:20:30Z"), "ab12cd"), "20261112-102030-ab12cd-My-drawing-1.zip");
  assert.equal(assetName("...zip", new Date("2026-11-12T10:20:30Z"), "x"), "20261112-102030-x-file.zip");
  const u = "https://github.com/toim22/AnyVector/releases/download/report-attachments/a.png";
  assert.equal(attachmentSection("a [b].png", 2048, u), `![a b.png](${u})\n\n[a b.png](${u}) (2 KB)`);
  assert.equal(attachmentSection("d.zip", 3 * MB, u), `[d.zip](${u}) (3.0 MB)`);
  const bug = issueOf(parseReport(good) as Report, "[d.zip](u) (1 KB)");
  assert.ok(bug.body.includes("### What happened\n\nIt closed\n\n### Attachment\n\n[d.zip](u) (1 KB)\n\n### AnyVector version"));
}

console.log("relay tests: ok");
