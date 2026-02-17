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

function normalizeTagVersion(tag) {
  const trimmed = String(tag || "").trim();
  const version = trimmed.startsWith("v") ? trimmed.slice(1) : trimmed;
  if (!/^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(version)) {
    throw new Error(`Tag "${tag}" is not a supported semantic version tag`);
  }
  return version;
}

function writeIfChanged(filePath, next) {
  const prev = fs.readFileSync(filePath, "utf8");
  if (prev === next) return false;
  fs.writeFileSync(filePath, next, "utf8");
  return true;
}

function updatePackageJson(version) {
  const filePath = "package.json";
  const json = JSON.parse(fs.readFileSync(filePath, "utf8"));
  json.version = version;
  return writeIfChanged(filePath, `${JSON.stringify(json, null, 2)}\n`);
}

function updateTauriConf(version) {
  const filePath = "src-tauri/tauri.conf.json";
  const json = JSON.parse(fs.readFileSync(filePath, "utf8"));
  json.package = json.package || {};
  json.package.version = version;
  return writeIfChanged(filePath, `${JSON.stringify(json, null, 2)}\n`);
}

function updateCargoToml(version) {
  const filePath = "src-tauri/Cargo.toml";
  const raw = fs.readFileSync(filePath, "utf8");
  const next = raw.replace(
    /(\[package\][\s\S]*?\nversion\s*=\s*")[^"]+(")/,
    `$1${version}$2`
  );
  if (next === raw) {
    throw new Error(`Could not locate [package] version in ${filePath}`);
  }
  return writeIfChanged(filePath, next);
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const tag = requireArg(args, "tag");
  const version = normalizeTagVersion(tag);

  if (!fs.existsSync(path.resolve("src-tauri", "tauri.conf.json"))) {
    throw new Error("Expected src-tauri/tauri.conf.json to exist");
  }

  const touched = [];
  if (updatePackageJson(version)) touched.push("package.json");
  if (updateTauriConf(version)) touched.push("src-tauri/tauri.conf.json");
  if (updateCargoToml(version)) touched.push("src-tauri/Cargo.toml");

  if (touched.length === 0) {
    console.log(`Versions already synced to ${version}`);
    return;
  }

  console.log(`Synced version to ${version} in:`);
  for (const file of touched) {
    console.log(`- ${file}`);
  }
}

try {
  main();
} catch (err) {
  console.error(err instanceof Error ? err.message : String(err));
  process.exit(1);
}
