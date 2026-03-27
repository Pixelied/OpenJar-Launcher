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

function extractBlock(source, startIndex, openChar, closeChar) {
  let depth = 0;
  let endIndex = -1;
  for (let i = startIndex; i < source.length; i += 1) {
    const char = source[i];
    if (char === openChar) depth += 1;
    if (char === closeChar) {
      depth -= 1;
      if (depth === 0) {
        endIndex = i;
        break;
      }
    }
  }
  if (endIndex < 0) {
    throw new Error(`Failed to parse block starting at index ${startIndex}`);
  }
  return source.slice(startIndex + 1, endIndex);
}

function extractRustStructFields(rustSource, structName) {
  const cleaned = stripComments(rustSource);
  const marker = `struct ${structName}`;
  const markerIndex = cleaned.indexOf(marker);
  if (markerIndex < 0) {
    throw new Error(`Could not find Rust struct ${structName}`);
  }
  const braceIndex = cleaned.indexOf("{", markerIndex);
  if (braceIndex < 0) {
    throw new Error(`Could not find opening brace for Rust struct ${structName}`);
  }
  const body = extractBlock(cleaned, braceIndex, "{", "}");
  const fields = new Set();
  for (const line of body.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#[")) continue;
    const match = trimmed.match(/^([A-Za-z_][A-Za-z0-9_]*)\s*:/);
    if (match) fields.add(match[1]);
  }
  return fields;
}

function extractTsTypeFields(tsSource, typeName) {
  const cleaned = stripComments(tsSource);
  const pattern = new RegExp(`export\\s+type\\s+${typeName}\\s*=\\s*\\{`);
  const match = pattern.exec(cleaned);
  if (!match) {
    throw new Error(`Could not find TypeScript type ${typeName}`);
  }
  const braceIndex = cleaned.indexOf("{", match.index);
  const body = extractBlock(cleaned, braceIndex, "{", "}");
  const fields = new Set();
  for (const line of body.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    const fieldMatch = trimmed.match(/^([A-Za-z_][A-Za-z0-9_]*)\??\s*:/);
    if (fieldMatch) fields.add(fieldMatch[1]);
  }
  return fields;
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
  const typeSource = readRepoFile("src/types.ts");

  const rustCommands = extractRustCommandNames(rustSource);
  const tsCommands = extractTsInvokeNames(tsSource);

  const missingInTs = setDifference(rustCommands, tsCommands);
  const missingInRust = setDifference(tsCommands, rustCommands);
  const sharedTypeNames = [
    "ProviderCandidate",
    "InstallProgressEvent",
    "DiscoverSearchHit",
    "DiscoverSearchResult",
    "GithubProjectDetail",
  ];
  const typeMismatches = [];

  for (const typeName of sharedTypeNames) {
    const rustFields = extractRustStructFields(rustSource, typeName);
    const tsFields = extractTsTypeFields(typeSource, typeName);
    const missingTypeFieldsInTs = setDifference(rustFields, tsFields);
    const missingTypeFieldsInRust = setDifference(tsFields, rustFields);
    if (missingTypeFieldsInTs.length > 0 || missingTypeFieldsInRust.length > 0) {
      typeMismatches.push({
        typeName,
        missingTypeFieldsInTs,
        missingTypeFieldsInRust,
      });
    }
  }

  if (missingInTs.length > 0 || missingInRust.length > 0 || typeMismatches.length > 0) {
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
    for (const mismatch of typeMismatches) {
      if (mismatch.missingTypeFieldsInTs.length > 0) {
        lines.push(
          `Missing ${mismatch.typeName} fields in src/types.ts: ${mismatch.missingTypeFieldsInTs.join(", ")}`
        );
      }
      if (mismatch.missingTypeFieldsInRust.length > 0) {
        lines.push(
          `Missing ${mismatch.typeName} fields in src-tauri/src/main.rs: ${mismatch.missingTypeFieldsInRust.join(", ")}`
        );
      }
    }
    throw new Error(lines.join("\n"));
  }

  console.log(
    `Tauri command contract verification passed (${rustCommands.size} commands, ${sharedTypeNames.length} shared payload types).`
  );
}

main();
