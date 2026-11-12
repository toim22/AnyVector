// The file a report may carry: the same types and sizes GitHub accepts when a file is dragged into an issue
// (docs.github.com › Attaching files). AnyVector checks the same rules before sending
// (crates/anyvector/src/bugreport.rs, ATTACH_*).

const MB = 1024 * 1024;

/** Images: shown in the issue. */
export const IMAGES = ["png", "gif", "jpg", "jpeg", "svg"];
/** Videos: 10 MB, as GitHub allows on a repository of a free account. */
export const VIDEOS = ["mp4", "mov", "webm"];
/** Every other type GitHub takes in an issue comment. */
export const OTHERS = [
  "pdf", "docx", "pptx", "xlsx", "xls", "xlsm", "odt", "fodt", "ods", "fods", "odp", "fodp", "odg", "fodg", "odf", "rtf", "doc",
  "txt", "md", "copilotmd", "csv", "tsv", "log", "json", "jsonc",
  "c", "cs", "cpp", "css", "drawio", "dmp", "html", "htm", "java", "js", "ipynb", "patch", "php", "cpuprofile", "pdb", "py", "sh",
  "sql", "ts", "tsx", "xml", "yaml", "yml",
  "zip", "gz", "tgz",
  "debug", "msg", "eml",
  "bmp", "tif", "tiff",
  "mp3", "wav",
];

export const MAX_IMAGE = 10 * MB;
export const MAX_VIDEO = 10 * MB;
export const MAX_OTHER = 25 * MB;

export function extensionOf(name: string): string {
  const dot = name.lastIndexOf(".");
  return dot < 0 ? "" : name.slice(dot + 1).toLowerCase();
}

export function isImage(name: string): boolean {
  return IMAGES.includes(extensionOf(name));
}

/** The largest size GitHub takes for this name, or 0 when it doesn't take the type. */
export function maxSize(name: string): number {
  const ext = extensionOf(name);
  if (IMAGES.includes(ext)) return MAX_IMAGE;
  if (VIDEOS.includes(ext)) return MAX_VIDEO;
  if (OTHERS.includes(ext)) return MAX_OTHER;
  return 0;
}

/** Why the file is refused, or null when GitHub would take it. */
export function checkAttachment(name: string, size: number): string | null {
  const max = maxSize(name);
  if (!max) return `file type .${extensionOf(name)} is not one GitHub accepts`;
  if (size <= 0) return "the file is empty";
  if (size > max) return `the file is larger than ${max / MB} MB`;
  return null;
}

/** A release asset name: letters, digits, dot, dash and underscore (GitHub changes the rest anyway), kept unique by
 *  a time stamp and a random part. */
export function assetName(name: string, now: Date, random: string): string {
  const ext = extensionOf(name);
  const stem = (ext ? name.slice(0, -(ext.length + 1)) : name).replace(/[^A-Za-z0-9._-]+/g, "-").replace(/^[-.]+|[-.]+$/g, "").slice(0, 60) || "file";
  const stamp = now.toISOString().replace(/[-:]/g, "").replace("T", "-").slice(0, 15);
  return `${stamp}-${random}-${stem}.${ext}`;
}

/** The "Attachment" section of the issue: the image itself, or a link to the file. */
export function attachmentSection(name: string, size: number, url: string): string {
  const shown = name.replace(/[[\]()<>`*_\\]/g, "");
  const kb = size < MB ? `${Math.max(1, Math.round(size / 1024))} KB` : `${(size / MB).toFixed(1)} MB`;
  return isImage(name) ? `![${shown}](${url})\n\n[${shown}](${url}) (${kb})` : `[${shown}](${url}) (${kb})`;
}
