#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..");

function readRepoFile(relativePath) {
  return fs.readFileSync(path.join(repoRoot, relativePath), "utf8");
}

function stripComments(input) {
  return input
    .replace(/\/\*[\s\S]*?\*\//g, "")
    .replace(/^\s*\/\/.*$/gm, "");
}

function extractGenerateHandlerBody(rustSource) {
  const marker = "tauri::generate_handler![";
  const markerIndex = rustSource.indexOf(marker);
  if (markerIndex < 0) {
    throw new Error("Could not find tauri::generate_handler![...] in src-tauri/src/main.rs");
  }

  const listStart = markerIndex + marker.length - 1; // points at '['
  let depth = 0;
  let endIndex = -1;
  for (let i = listStart; i < rustSource.length; i += 1) {
    const char = rustSource[i];
    if (char === "[") depth += 1;
    if (char === "]") {
      depth -= 1;
      if (depth === 0) {
        endIndex = i;
        break;
      }
    }
  }

  if (endIndex < 0) {
    throw new Error("Failed to parse tauri::generate_handler! list boundaries");
  }

  return rustSource.slice(listStart + 1, endIndex);
}

function extractRustCommandNames(rustSource) {
  const body = stripComments(extractGenerateHandlerBody(rustSource));
  const commands = new Set();

  for (const token of body.split(",")) {
    const trimmed = token.trim();
    if (!trimmed) continue;
    if (!/^[A-Za-z_][A-Za-z0-9_:]*$/.test(trimmed)) {
      throw new Error(`Unexpected token in generate_handler list: "${trimmed}"`);
    }
    const localName = trimmed.split("::").pop();
    commands.add(localName);
  }

  return commands;
}

function extractTsInvokeNames(tsSource) {
  const commands = new Set();
  const invokePattern = /invoke\(\s*"([A-Za-z_][A-Za-z0-9_]*)"/g;
  for (const match of tsSource.matchAll(invokePattern)) {
    commands.add(match[1]);
  }
  return commands;
}

function toSortedArray(set) {
  return [...set].sort((a, b) => a.localeCompare(b));
}

function setDifference(left, right) {
  return [...left].filter((value) => !right.has(value)).sort((a, b) =>
    a.localeCompare(b)
  );
}

function main() {
  const rustSource = readRepoFile("src-tauri/src/main.rs");
  const tsSource = readRepoFile("src/tauri.ts");

  const rustCommands = extractRustCommandNames(rustSource);
  const tsCommands = extractTsInvokeNames(tsSource);

  const missingInTs = setDifference(rustCommands, tsCommands);
  const missingInRust = setDifference(tsCommands, rustCommands);

  if (missingInTs.length > 0 || missingInRust.length > 0) {
    const lines = [
      "Tauri command contract mismatch detected.",
      `Backend commands (${rustCommands.size}): ${toSortedArray(rustCommands).join(", ")}`,
      `Frontend invoke wrappers (${tsCommands.size}): ${toSortedArray(tsCommands).join(", ")}`,
    ];
    if (missingInTs.length > 0) {
      lines.push(`Missing in src/tauri.ts: ${missingInTs.join(", ")}`);
    }
    if (missingInRust.length > 0) {
      lines.push(`Missing in src-tauri/src/main.rs generate_handler!: ${missingInRust.join(", ")}`);
    }
    throw new Error(lines.join("\n"));
  }

  console.log(
    `Tauri command contract verification passed (${rustCommands.size} commands).`
  );
}

main();
