#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";

const PLATFORM_MAP = {
  "macos-apple-silicon": "darwin-aarch64",
  "macos-intel": "darwin-x86_64",
  "windows-arm64": "windows-aarch64",
  "windows-x64": "windows-x86_64",
  "linux-x64": "linux-x86_64",
};

const ARCHIVE_RE =
  /^openjar-launcher_v[^_]+_(macos-apple-silicon|macos-intel|windows-arm64|windows-x64|linux-x64)_auto-update-only_updater\.(tar\.gz|zip)$/;

function parseArgs(argv) {
  const args = {};
  for (let i = 0; i < argv.length; i += 1) {
    const token = argv[i];
    if (!token.startsWith("--")) continue;
    const key = token.slice(2);
    const next = argv[i + 1];
    if (!next || next.startsWith("--")) {
      args[key] = true;
    } else {
      args[key] = next;
      i += 1;
    }
  }
  return args;
}

function walkFiles(dir) {
  const out = [];
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      out.push(...walkFiles(full));
      continue;
    }
    if (entry.isFile()) out.push(full);
  }
  return out;
}

function requireArg(args, name) {
  const value = args[name];
  if (!value || typeof value !== "string") {
    throw new Error(`Missing required argument --${name}`);
  }
  return value;
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const root = requireArg(args, "root");
  const repo = requireArg(args, "repo");
  const tag = requireArg(args, "tag");
  const outName = typeof args.out === "string" ? args.out : "latest.json";

  if (!fs.existsSync(root) || !fs.statSync(root).isDirectory()) {
    throw new Error(`Release assets root does not exist or is not a directory: ${root}`);
  }

  const files = walkFiles(root).map((full) => path.relative(root, full).replace(/\\/g, "/"));
  const updaterArchives = files.filter((rel) => ARCHIVE_RE.test(path.basename(rel)));
  const seen = new Set();
  const platforms = {};

  for (const relArchive of updaterArchives) {
    const fileName = path.basename(relArchive);
    const match = fileName.match(ARCHIVE_RE);
    if (!match) continue;

    const label = match[1];
    const platform = PLATFORM_MAP[label];
    if (!platform) continue;

    if (seen.has(platform)) {
      throw new Error(`Multiple updater archives found for platform ${platform}`);
    }

    const relSig = `${relArchive}.sig`;
    const fullSig = path.join(root, relSig);
    if (!fs.existsSync(fullSig)) {
      throw new Error(`Missing updater signature for ${relArchive}`);
    }

    const signature = fs.readFileSync(fullSig, "utf8").trim();
    if (!signature) {
      throw new Error(`Updater signature file is empty: ${relSig}`);
    }

    const encodedName = encodeURIComponent(fileName);
    platforms[platform] = {
      signature,
      url: `https://github.com/${repo}/releases/download/${tag}/${encodedName}`,
    };
    seen.add(platform);
  }

  if (Object.keys(platforms).length === 0) {
    throw new Error("No signed updater archives matched expected naming pattern.");
  }

  const version = tag.startsWith("v") ? tag.slice(1) : tag;
  const manifest = {
    version,
    notes: `OpenJar Launcher ${tag}`,
    pub_date: new Date().toISOString(),
    platforms,
  };

  const outPath = path.join(root, outName);
  fs.writeFileSync(outPath, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
  console.log(`Wrote ${outPath}`);
  console.log(JSON.stringify(manifest, null, 2));
}

try {
  main();
} catch (err) {
  console.error(err instanceof Error ? err.message : String(err));
  process.exit(1);
}
