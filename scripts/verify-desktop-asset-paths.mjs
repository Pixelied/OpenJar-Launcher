#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..");
const indexPath = path.join(repoRoot, "dist", "index.html");

function fail(message) {
  console.error(message);
  process.exit(1);
}

if (!fs.existsSync(indexPath)) {
  fail(
    "dist/index.html not found. Run a production build first (for example: npm run build).",
  );
}

const html = fs.readFileSync(indexPath, "utf8");
const issues = [];

const patterns = [
  { regex: /<script[^>]+src="([^"]+)"/g, label: "script src" },
  { regex: /<link[^>]+href="([^"]+)"/g, label: "link href" },
];

for (const { regex, label } of patterns) {
  for (const match of html.matchAll(regex)) {
    const value = match[1];
    const isAbsoluteWeb = /^https?:\/\//i.test(value);
    const isRootAbsolute = value.startsWith("/");
    const isAllowedRelative =
      value.startsWith("./") ||
      value.startsWith("assets/") ||
      value.startsWith("../");

    if (isAbsoluteWeb || isRootAbsolute || !isAllowedRelative) {
      issues.push(`${label}="${value}"`);
    }
  }
}

if (issues.length > 0) {
  fail(
    [
      "Desktop bundle asset paths must be relative for Tauri packaged apps.",
      ...issues.map((entry) => ` - ${entry}`),
      "Expected values like ./assets/... (not /OpenJar-Launcher/... or https://...).",
    ].join("\n"),
  );
}

console.log("Desktop asset path verification passed.");
