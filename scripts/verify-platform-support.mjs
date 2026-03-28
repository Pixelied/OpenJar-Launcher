#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..");

const TARGET_TO_PLATFORM = new Map([
  ["aarch64-apple-darwin", "darwin-aarch64"],
  ["x86_64-apple-darwin", "darwin-x86_64"],
  ["aarch64-pc-windows-msvc", "windows-aarch64"],
  ["x86_64-pc-windows-msvc", "windows-x86_64"],
  ["x86_64-unknown-linux-gnu", "linux-x86_64"],
]);

function readRepoFile(relativePath) {
  // Normalize line endings so parsing is stable across Windows/macOS/Linux runners.
  return fs
    .readFileSync(path.join(repoRoot, relativePath), "utf8")
    .replace(/\r\n?/g, "\n");
}

function extractTargetsFromWorkflow(workflowBody) {
  const targets = new Set();
  const regex = /^\s*target:\s*([A-Za-z0-9._-]+)\s*$/gm;
  for (const match of workflowBody.matchAll(regex)) {
    targets.add(match[1]);
  }
  return targets;
}

function extractReadmeTargets(readmeBody) {
  const sectionMatch = readmeBody.match(
    /##\s+Platform Support(?:\s*&\s*Testing)?\s*\n([\s\S]*?)(?:\n##\s+|\s*$)/i,
  );
  if (!sectionMatch) {
    throw new Error("Could not parse README platform support section.");
  }

  const section = sectionMatch[1];
  const markerMatch = section.match(/Current(?:\s+build)?\s+targets:/i);
  if (!markerMatch || markerMatch.index == null) {
    throw new Error(
      "README platform section is missing 'Current targets:' or 'Current build targets:'.",
    );
  }

  const lines = section
    .slice(markerMatch.index + markerMatch[0].length)
    .split("\n");
  const targets = new Set();

  for (const line of lines) {
    const trimmed = line.trim();
    if (!trimmed) {
      if (targets.size > 0) break;
      continue;
    }
    if (!trimmed.startsWith("-")) {
      if (targets.size > 0) break;
      continue;
    }

    for (const match of trimmed.matchAll(/`([^`]+)`/g)) {
      targets.add(match[1]);
    }
  }

  if (targets.size === 0) {
    throw new Error("No Rust targets found in README 'Current build targets' list.");
  }

  return targets;
}

function extractRequiredPlatformSets(releaseWorkflowBody) {
  const sets = [];
  const regex = /--require-platforms\s+"([^"]+)"/g;
  for (const match of releaseWorkflowBody.matchAll(regex)) {
    const values = match[1]
      .split(",")
      .map((value) => value.trim())
      .filter(Boolean);
    sets.push(new Set(values));
  }
  return sets;
}

function toSortedArray(values) {
  return [...values].sort((a, b) => a.localeCompare(b));
}

function diffSets(expected, actual) {
  const missing = [...expected].filter((v) => !actual.has(v)).sort((a, b) =>
    a.localeCompare(b),
  );
  const extra = [...actual].filter((v) => !expected.has(v)).sort((a, b) =>
    a.localeCompare(b),
  );
  return { missing, extra };
}

function assertSetEquals(label, expected, actual) {
  const { missing, extra } = diffSets(expected, actual);
  if (missing.length === 0 && extra.length === 0) {
    return;
  }

  const lines = [`${label} does not match expected targets.`];
  if (missing.length > 0) {
    lines.push(`  Missing: ${missing.join(", ")}`);
  }
  if (extra.length > 0) {
    lines.push(`  Extra: ${extra.join(", ")}`);
  }
  throw new Error(lines.join("\n"));
}

function mapTargetsToPlatforms(targets) {
  const platforms = new Set();
  for (const target of targets) {
    const platform = TARGET_TO_PLATFORM.get(target);
    if (!platform) {
      throw new Error(`Unknown Rust target in platform matrix: ${target}`);
    }
    platforms.add(platform);
  }
  return platforms;
}

function main() {
  const readme = readRepoFile("README.md");
  const ciBuild = readRepoFile(".github/workflows/ci-build.yml");
  const release = readRepoFile(".github/workflows/release.yml");

  const readmeTargets = extractReadmeTargets(readme);
  const ciTargets = extractTargetsFromWorkflow(ciBuild);
  const releaseTargets = extractTargetsFromWorkflow(release);

  assertSetEquals("README targets", ciTargets, readmeTargets);
  assertSetEquals("release workflow targets", ciTargets, releaseTargets);

  const expectedPlatforms = mapTargetsToPlatforms(ciTargets);
  const requiredPlatformSets = extractRequiredPlatformSets(release);

  if (requiredPlatformSets.length === 0) {
    throw new Error("No --require-platforms entries found in release workflow.");
  }

  requiredPlatformSets.forEach((platformSet, index) => {
    assertSetEquals(
      `release --require-platforms set #${index + 1}`,
      expectedPlatforms,
      platformSet,
    );
  });

  console.log("Platform support verification passed.");
  console.log(`Targets: ${toSortedArray(ciTargets).join(", ")}`);
  console.log(`Updater platforms: ${toSortedArray(expectedPlatforms).join(", ")}`);
}

main();
