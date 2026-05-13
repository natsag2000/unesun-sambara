#!/usr/bin/env node
/**
 * Decompress `01_kv.tsv.xz` (repo root) into `dist/dictionary/01_kv.tsv`.
 *
 * Uses `xz-decompress` (pure JS) so this runs without native builds on any
 * platform that has Node 18+.
 *
 * Skips work when the output is newer than the input unless `--force` is
 * passed.
 */

const fs = require("fs");
const path = require("path");
const { Readable } = require("stream");

const root = path.resolve(__dirname, "..");
const inputPath = path.join(root, "01_kv.tsv.xz");
const outputDir = path.join(root, "dist", "dictionary");
const outputPath = path.join(outputDir, "01_kv.tsv");

const force = process.argv.includes("--force");

function mtime(p) {
  try {
    return fs.statSync(p).mtimeMs;
  } catch {
    return 0;
  }
}

async function main() {
  if (!fs.existsSync(inputPath)) {
    console.error(`[prepare-dictionary] Missing input: ${inputPath}`);
    process.exit(1);
  }

  if (!force && mtime(outputPath) >= mtime(inputPath) && mtime(outputPath) > 0) {
    console.log(
      `[prepare-dictionary] Up-to-date, skipping. (Use --force to rebuild.)`,
    );
    return;
  }

  fs.mkdirSync(outputDir, { recursive: true });

  const xzMod = require("xz-decompress");
  const XzReadableStream = xzMod.XzReadableStream || xzMod.default?.XzReadableStream;
  if (typeof XzReadableStream !== "function") {
    throw new Error(
      "xz-decompress did not export XzReadableStream (got " +
        Object.keys(xzMod).join(", ") +
        ")",
    );
  }

  const nodeStream = fs.createReadStream(inputPath);
  const webStream = Readable.toWeb(nodeStream);
  const decompressed = new XzReadableStream(webStream);

  const chunks = [];
  const reader = decompressed.getReader();
  while (true) {
    const { value, done } = await reader.read();
    if (done) break;
    chunks.push(value);
  }
  const totalLen = chunks.reduce((a, b) => a + b.length, 0);
  const buf = Buffer.alloc(totalLen);
  let off = 0;
  for (const c of chunks) {
    buf.set(c, off);
    off += c.length;
  }

  // Basic sanity check + summary.
  const text = buf.toString("utf8");
  const lines = text.split(/\r?\n/);
  let entries = 0;
  let skipped = 0;
  for (const line of lines) {
    if (!line || line.startsWith("#")) continue;
    const tabs = (line.match(/\t/g) || []).length;
    if (tabs >= 1) entries += 1;
    else skipped += 1;
  }

  fs.writeFileSync(outputPath, buf);
  console.log(
    `[prepare-dictionary] Wrote ${outputPath} (${(buf.length / 1024).toFixed(
      1,
    )} KB, ${entries} entries, ${skipped} skipped lines).`,
  );
}

main().catch((err) => {
  console.error("[prepare-dictionary] Failed:", err);
  process.exit(1);
});
