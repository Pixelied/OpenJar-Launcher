#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";

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

function requireArg(args, name) {
  const value = args[name];
  if (!value || typeof value !== "string") {
    throw new Error(`Missing required argument --${name}`);
  }
  return value;
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

async function sleep(ms) {
  await new Promise((resolve) => setTimeout(resolve, ms));
}

function validateBase64Text(value) {
  const compact = String(value || "").replace(/\s+/g, "");
  if (!compact) return false;
  if (!/^[A-Za-z0-9+/=]+$/.test(compact)) return false;
  return compact.length % 4 !== 1;
}

async function checkUrl(url, retries, retryDelayMs, authToken) {
  let lastErr = null;
  const baseHeaders = authToken ? { Authorization: `Bearer ${authToken}` } : {};
  for (let attempt = 1; attempt <= retries; attempt += 1) {
    try {
      const headResp = await fetch(url, {
        method: "HEAD",
        redirect: "follow",
        headers: baseHeaders,
      });
      if (headResp.ok) return;
      if (headResp.status !== 405 && headResp.status !== 403) {
        throw new Error(`HEAD ${headResp.status}`);
      }

      const getResp = await fetch(url, {
        method: "GET",
        redirect: "follow",
        headers: { ...baseHeaders, Range: "bytes=0-0" },
      });
      if (getResp.ok || getResp.status === 206) return;
      throw new Error(`GET ${getResp.status}`);
    } catch (err) {
      lastErr = err instanceof Error ? err : new Error(String(err));
      if (attempt < retries) {
        await sleep(retryDelayMs);
      }
    }
  }
  throw new Error(`URL check failed for ${url}: ${lastErr?.message || "unknown error"}`);
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const root = requireArg(args, "root");
  const fileName = typeof args.file === "string" ? args.file : "latest.json";
  const assetTag = requireArg(args, "asset-tag");
  const expectedVersion = typeof args["expected-version"] === "string" ? args["expected-version"] : "";
  const checkUrls = Boolean(args["check-urls"]);
  const authToken = typeof args["auth-token"] === "string" ? args["auth-token"] : "";
  const retries = Number.parseInt(String(args.retries || "3"), 10);
  const retryDelayMs = Number.parseInt(String(args["retry-delay-ms"] || "1500"), 10);
  const requiredPlatforms =
    typeof args["require-platforms"] === "string" && args["require-platforms"].trim()
      ? args["require-platforms"]
          .split(",")
          .map((s) => s.trim())
          .filter(Boolean)
      : [];

  const manifestPath = path.join(root, fileName);
  const raw = fs.readFileSync(manifestPath, "utf8");
  let manifest;
  try {
    manifest = JSON.parse(raw);
  } catch (err) {
    throw new Error(`Invalid JSON in ${manifestPath}: ${err instanceof Error ? err.message : String(err)}`);
  }

  if (!manifest || typeof manifest !== "object") {
    throw new Error(`Manifest is not an object: ${manifestPath}`);
  }
  if (typeof manifest.version !== "string" || !manifest.version.trim()) {
    throw new Error("Manifest missing non-empty string: version");
  }
  if (!manifest.platforms || typeof manifest.platforms !== "object") {
    throw new Error("Manifest missing object: platforms");
  }
  if (expectedVersion && String(manifest.version).trim() !== expectedVersion) {
    throw new Error(`Manifest version ${manifest.version} does not match expected ${expectedVersion}`);
  }

  const platformEntries = Object.entries(manifest.platforms);
  if (platformEntries.length === 0) {
    throw new Error("Manifest platforms is empty");
  }

  for (const required of requiredPlatforms) {
    if (!manifest.platforms[required]) {
      throw new Error(`Manifest missing required platform: ${required}`);
    }
  }

  const localFiles = walkFiles(root).map((full) => path.relative(root, full).replace(/\\/g, "/"));
  const localBasenames = new Set(localFiles.map((rel) => path.basename(rel)));

  for (const [platform, entry] of platformEntries) {
    if (!entry || typeof entry !== "object") {
      throw new Error(`Manifest platform entry is not an object: ${platform}`);
    }
    if (typeof entry.signature !== "string" || !entry.signature.trim()) {
      throw new Error(`Manifest platform entry missing signature: ${platform}`);
    }
    if (typeof entry.url !== "string" || !entry.url.startsWith("https://")) {
      throw new Error(`Manifest platform entry missing https url: ${platform}`);
    }

    if (!validateBase64Text(entry.signature)) {
      throw new Error(`Manifest signature is not base64 for platform: ${platform}`);
    }

    const url = new URL(entry.url);
    if (!url.pathname.includes(`/releases/download/${assetTag}/`)) {
      throw new Error(`Manifest URL does not target updater tag ${assetTag} for platform ${platform}`);
    }
    const assetName = decodeURIComponent(path.basename(url.pathname));
    if (!localBasenames.has(assetName)) {
      throw new Error(`Manifest URL points to asset not found in release-assets: ${assetName}`);
    }
  }

  if (checkUrls) {
    for (const [, entry] of platformEntries) {
      await checkUrl(entry.url, retries, retryDelayMs, authToken);
    }
  }

  console.log(
    `Validated ${manifestPath}: ${platformEntries.length} platforms${checkUrls ? ", URLs reachable" : ""}`
  );
}

main().catch((err) => {
  console.error(err instanceof Error ? err.message : String(err));
  process.exit(1);
});
