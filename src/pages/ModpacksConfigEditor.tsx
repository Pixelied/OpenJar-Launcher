import { useEffect, useMemo, useState } from "react";
import type {
  Instance,
  InstanceConfigFileBackupEntry,
  InstanceConfigFileEntry,
  InstanceWorld,
  WorldConfigFileEntry,
} from "../types";
import {
  listInstanceConfigFileBackups,
  listInstanceConfigFiles,
  listInstanceWorlds,
  listWorldConfigFiles,
  readWorldConfigFile,
  readInstanceConfigFile,
  revealConfigEditorFile,
  restoreInstanceConfigFileBackup,
  writeInstanceConfigFile,
  writeWorldConfigFile,
} from "../tauri";
import AdvancedEditor from "../components/AdvancedEditor";
import ConfigEditorTopBar from "../components/ConfigEditorTopBar";
import ConfigFileList, { type ConfigFileListItem } from "../components/ConfigFileList";
import InspectorPanel from "../components/InspectorPanel";
import InstancePickerDropdown from "../components/InstancePickerDropdown";
import JsonSimpleEditor from "../components/JsonSimpleEditor";
import NewFileModal from "../components/NewFileModal";
import ServersDatSimpleEditor from "../components/ServersDatSimpleEditor";
import TextSimpleEditor from "../components/TextSimpleEditor";
import {
  asPrettyJson,
  describePath,
  fileGroupForPath,
  fileTypeForPath,
  getEffectiveFileContent,
  hasUnsavedDraft,
  isJsonFilePath,
  parseJsonWithError,
  SERVERS_DAT_PATH,
  type ConfigFileRecord,
  type JsonParseIssue,
  type JsonPath,
} from "./configEditorHelpers";
import {
  formatConfigContent,
  getFormatterSupport,
} from "../lib/configFormatting";
import {
  applySafeFixes,
  collectConfigIssues,
  getConfigDocForPath,
  groupIssuesByPath,
  type ConfigIssue,
} from "../lib/configIntelligence";

type EditorMode = "simple" | "advanced";
type EditorScope = "instance" | "world";
type NonJsonInspectorSelection = { path: string; value: string; type: string } | null;
type FileHistory = { entries: string[]; index: number };

function formatWorldGroup(path: string): string {
  const clean = String(path || "").replace(/\\/g, "/").trim();
  if (!clean.includes("/")) return "Root";
  const first = clean.split("/")[0] || "Root";
  return first
    .replace(/[-_]+/g, " ")
    .replace(/\s+/g, " ")
    .trim()
    .replace(/\b\w/g, (m) => m.toUpperCase());
}

function worldScopeId(instanceId: string, worldId: string) {
  return `${instanceId}::${worldId}`;
}

function worldRecordId(instanceId: string, worldId: string, filePath: string) {
  return `${instanceId}::${worldId}::${filePath}`;
}

function toWorldTypeLabel(item: WorldConfigFileEntry): string {
  const kind = String(item.kind || "").trim();
  if (!kind) return fileTypeForPath(item.path);
  return kind.toUpperCase();
}

function instanceRecordId(instanceId: string, filePath: string) {
  return `${instanceId}::${filePath}`;
}

function formatBackupTime(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "Unknown time";
  try {
    return new Date(value).toLocaleString();
  } catch {
    return "Unknown time";
  }
}

export default function ModpacksConfigEditor({
  instances,
  selectedInstanceId,
  onSelectInstance,
  onManageInstances,
  runningInstanceIds,
}: {
  instances: Instance[];
  selectedInstanceId: string | null;
  onSelectInstance: (instanceId: string) => void;
  onManageInstances: () => void;
  runningInstanceIds: string[];
}) {
  const [scope, setScope] = useState<EditorScope>("instance");
  const [instanceFilesByInstance, setInstanceFilesByInstance] = useState<
    Record<string, InstanceConfigFileEntry[]>
  >({});
  const [instanceDrafts, setInstanceDrafts] = useState<Record<string, ConfigFileRecord>>({});
  const [worldDrafts, setWorldDrafts] = useState<Record<string, ConfigFileRecord>>({});
  const [backupsByFile, setBackupsByFile] = useState<Record<string, InstanceConfigFileBackupEntry[]>>({});
  const [selectedBackupIdByFile, setSelectedBackupIdByFile] = useState<Record<string, string>>({});
  const [selectedPathByScope, setSelectedPathByScope] = useState<Record<string, string>>({});
  const [selectedWorldByInstance, setSelectedWorldByInstance] = useState<Record<string, string>>({});
  const [worldsByInstance, setWorldsByInstance] = useState<Record<string, InstanceWorld[]>>({});
  const [worldFilesByScope, setWorldFilesByScope] = useState<Record<string, WorldConfigFileEntry[]>>({});
  const [instanceFilesBusy, setInstanceFilesBusy] = useState(false);
  const [instanceFilesErr, setInstanceFilesErr] = useState<string | null>(null);
  const [instanceReadBusyPath, setInstanceReadBusyPath] = useState<string | null>(null);
  const [instanceReadErr, setInstanceReadErr] = useState<string | null>(null);
  const [backupsBusy, setBackupsBusy] = useState(false);
  const [backupsErr, setBackupsErr] = useState<string | null>(null);
  const [restoreBusy, setRestoreBusy] = useState(false);
  const [fileQuery, setFileQuery] = useState("");
  const [mode, setMode] = useState<EditorMode>("advanced");
  const [jsonFocusPath, setJsonFocusPath] = useState<JsonPath>([]);
  const [inspectorPath, setInspectorPath] = useState<JsonPath | null>(null);
  const [nonJsonInspectorSelection, setNonJsonInspectorSelection] =
    useState<NonJsonInspectorSelection>(null);
  const [showNewFileModal, setShowNewFileModal] = useState(false);
  const [worldBusy, setWorldBusy] = useState(false);
  const [worldErr, setWorldErr] = useState<string | null>(null);
  const [worldFilesBusy, setWorldFilesBusy] = useState(false);
  const [worldFilesErr, setWorldFilesErr] = useState<string | null>(null);
  const [worldReadBusyPath, setWorldReadBusyPath] = useState<string | null>(null);
  const [worldReadErr, setWorldReadErr] = useState<string | null>(null);
  const [saveBusy, setSaveBusy] = useState(false);
  const [saveErr, setSaveErr] = useState<string | null>(null);
  const [editorInfo, setEditorInfo] = useState<string | null>(null);
  const [showDiff, setShowDiff] = useState(false);
  const [historyByFile, setHistoryByFile] = useState<Record<string, FileHistory>>({});
  const [editorSearch, setEditorSearch] = useState("");

  const activeInstance = useMemo(() => {
    if (instances.length === 0) return null;
    const selected = instances.find((item) => item.id === selectedInstanceId);
    return selected ?? instances[0] ?? null;
  }, [instances, selectedInstanceId]);

  useEffect(() => {
    if (!activeInstance) return;
    if (!selectedInstanceId || selectedInstanceId !== activeInstance.id) {
      onSelectInstance(activeInstance.id);
    }
  }, [activeInstance, onSelectInstance, selectedInstanceId]);

  useEffect(() => {
    if (!activeInstance) return;
    let cancelled = false;
    setInstanceFilesBusy(true);
    setInstanceFilesErr(null);
    listInstanceConfigFiles({ instanceId: activeInstance.id })
      .then((files) => {
        if (cancelled) return;
        setInstanceFilesByInstance((prev) => ({
          ...prev,
          [activeInstance.id]: files,
        }));
      })
      .catch((err: any) => {
        if (cancelled) return;
        setInstanceFilesByInstance((prev) => ({
          ...prev,
          [activeInstance.id]: [],
        }));
        setInstanceFilesErr(err?.toString?.() ?? String(err));
      })
      .finally(() => {
        if (!cancelled) setInstanceFilesBusy(false);
      });
    return () => {
      cancelled = true;
    };
  }, [activeInstance?.id]);

  useEffect(() => {
    if (!activeInstance) return;
    let cancelled = false;
    setWorldBusy(true);
    setWorldErr(null);
    listInstanceWorlds({ instanceId: activeInstance.id })
      .then((worlds) => {
        if (cancelled) return;
        setWorldsByInstance((prev) => ({
          ...prev,
          [activeInstance.id]: worlds,
        }));
      })
      .catch((err: any) => {
        if (cancelled) return;
        setWorldsByInstance((prev) => ({
          ...prev,
          [activeInstance.id]: [],
        }));
        setWorldErr(err?.toString?.() ?? String(err));
      })
      .finally(() => {
        if (!cancelled) setWorldBusy(false);
      });
    return () => {
      cancelled = true;
    };
  }, [activeInstance?.id]);

  const activeWorlds = useMemo(
    () => (activeInstance ? worldsByInstance[activeInstance.id] ?? [] : []),
    [activeInstance, worldsByInstance]
  );

  const activeWorld = useMemo(() => {
    if (!activeInstance) return null;
    const selectedId = selectedWorldByInstance[activeInstance.id];
    if (selectedId) {
      const found = activeWorlds.find((world) => world.id === selectedId);
      if (found) return found;
    }
    return activeWorlds[0] ?? null;
  }, [activeInstance, activeWorlds, selectedWorldByInstance]);

  useEffect(() => {
    if (!activeInstance || !activeWorld) return;
    setSelectedWorldByInstance((prev) => {
      if (prev[activeInstance.id] === activeWorld.id) return prev;
      return {
        ...prev,
        [activeInstance.id]: activeWorld.id,
      };
    });
  }, [activeInstance, activeWorld]);

  const activeWorldScope = useMemo(() => {
    if (!activeInstance || !activeWorld) return null;
    return worldScopeId(activeInstance.id, activeWorld.id);
  }, [activeInstance, activeWorld]);

  useEffect(() => {
    if (scope !== "world" || !activeInstance || !activeWorldScope || !activeWorld) return;
    let cancelled = false;
    setWorldFilesBusy(true);
    setWorldFilesErr(null);
    listWorldConfigFiles({ instanceId: activeInstance.id, worldId: activeWorld.id })
      .then((files) => {
        if (cancelled) return;
        setWorldFilesByScope((prev) => ({
          ...prev,
          [activeWorldScope]: files,
        }));
      })
      .catch((err: any) => {
        if (cancelled) return;
        setWorldFilesByScope((prev) => ({
          ...prev,
          [activeWorldScope]: [],
        }));
        setWorldFilesErr(err?.toString?.() ?? String(err));
      })
      .finally(() => {
        if (!cancelled) setWorldFilesBusy(false);
      });
    return () => {
      cancelled = true;
    };
  }, [scope, activeInstance?.id, activeWorld?.id, activeWorldScope]);

  const instanceFiles = useMemo(
    () => (activeInstance ? instanceFilesByInstance[activeInstance.id] ?? [] : []),
    [activeInstance, instanceFilesByInstance]
  );

  const instanceList = useMemo<ConfigFileListItem[]>(() => {
    if (!activeInstance) return [];
    const items: ConfigFileListItem[] = instanceFiles
      .slice()
      .sort((a, b) => a.path.localeCompare(b.path))
      .map((file) => {
        const record = instanceDrafts[instanceRecordId(activeInstance.id, file.path)];
        return {
          path: file.path,
          group: fileGroupForPath(file.path),
          typeLabel: fileTypeForPath(file.path),
          editable: file.editable,
          readonlyReason: file.readonly_reason ?? null,
          unsaved: hasUnsavedDraft(record),
        };
      });
    return items.sort((a, b) => {
      const groupA = a.group === "Minecraft" ? 0 : a.group === "Loader" ? 1 : 2;
      const groupB = b.group === "Minecraft" ? 0 : b.group === "Loader" ? 1 : 2;
      if (groupA !== groupB) return groupA - groupB;
      if (a.path === "options.txt") return -1;
      if (b.path === "options.txt") return 1;
      if (a.path === "config/modpack.json") return -1;
      if (b.path === "config/modpack.json") return 1;
      return a.path.localeCompare(b.path);
    });
  }, [activeInstance, instanceDrafts, instanceFiles]);

  const worldList = useMemo<ConfigFileListItem[]>(() => {
    if (!activeInstance || !activeWorld || !activeWorldScope) return [];
    const source = worldFilesByScope[activeWorldScope] ?? [];
    return source.map((item) => ({
      path: item.path,
      group: formatWorldGroup(item.path),
      typeLabel: toWorldTypeLabel(item),
      editable: item.editable,
      readonlyReason: item.readonly_reason ?? null,
      unsaved: hasUnsavedDraft(worldDrafts[worldRecordId(activeInstance.id, activeWorld.id, item.path)]),
    }));
  }, [activeInstance, activeWorld, activeWorldScope, worldDrafts, worldFilesByScope]);

  const files = useMemo(() => (scope === "instance" ? instanceList : worldList), [instanceList, scope, worldList]);

  const activeScopeKey = useMemo(() => {
    if (!activeInstance) return null;
    if (scope === "instance") return `instance:${activeInstance.id}`;
    if (!activeWorld) return null;
    return `world:${activeInstance.id}:${activeWorld.id}`;
  }, [activeInstance, activeWorld, scope]);

  const activePath = useMemo(() => {
    if (!activeScopeKey) return null;
    const remembered = selectedPathByScope[activeScopeKey] ?? null;
    if (remembered && files.some((file) => file.path === remembered)) return remembered;
    return files[0]?.path ?? null;
  }, [activeScopeKey, files, selectedPathByScope]);

  useEffect(() => {
    if (!activeScopeKey || !activePath) return;
    setSelectedPathByScope((prev) => {
      if (prev[activeScopeKey] === activePath) return prev;
      return {
        ...prev,
        [activeScopeKey]: activePath,
      };
    });
  }, [activeScopeKey, activePath]);

  useEffect(() => {
    setInspectorPath(null);
    setJsonFocusPath([]);
    setNonJsonInspectorSelection(null);
    setSaveErr(null);
    setInstanceReadErr(null);
    setWorldReadErr(null);
    setBackupsErr(null);
    setEditorInfo(null);
    setShowDiff(false);
    setEditorSearch("");
  }, [scope, activeInstance?.id, activeWorld?.id, activePath]);

  const activeFile = useMemo(() => files.find((item) => item.path === activePath) ?? null, [files, activePath]);

  const activeRecord = useMemo(() => {
    if (!activePath || !activeInstance) return undefined;
    if (scope === "instance") {
      return instanceDrafts[instanceRecordId(activeInstance.id, activePath)];
    }
    if (!activeWorld) return undefined;
    return worldDrafts[worldRecordId(activeInstance.id, activeWorld.id, activePath)];
  }, [activePath, activeInstance, activeWorld, instanceDrafts, scope, worldDrafts]);

  const activeRecordKey = useMemo(() => {
    if (!activePath || !activeInstance) return null;
    if (scope === "instance") return `instance::${activeInstance.id}::${activePath}`;
    if (!activeWorld) return null;
    return `world::${activeInstance.id}::${activeWorld.id}::${activePath}`;
  }, [activePath, activeInstance, activeWorld, scope]);

  useEffect(() => {
    if (scope !== "instance" || !activeInstance || !activePath || activeRecord) return;
    let cancelled = false;
    const recordKey = instanceRecordId(activeInstance.id, activePath);
    setInstanceReadBusyPath(activePath);
    setInstanceReadErr(null);
    readInstanceConfigFile({ instanceId: activeInstance.id, path: activePath })
      .then((result) => {
        if (cancelled) return;
        setInstanceDrafts((prev) => {
          if (prev[recordKey]) return prev;
          return {
            ...prev,
            [recordKey]: {
              content: String(result.content ?? result.readonly_reason ?? ""),
              updatedAt: Number(result.modified_at ?? Date.now()),
            },
          };
        });
      })
      .catch((err: any) => {
        if (cancelled) return;
        setInstanceReadErr(err?.toString?.() ?? String(err));
      })
      .finally(() => {
        if (!cancelled) setInstanceReadBusyPath(null);
      });
    return () => {
      cancelled = true;
    };
  }, [scope, activeInstance, activePath, activeRecord]);

  useEffect(() => {
    if (scope !== "world" || !activeInstance || !activeWorld || !activePath || !activeFile) return;
    const recordKey = worldRecordId(activeInstance.id, activeWorld.id, activePath);
    if (worldDrafts[recordKey]) return;

    let cancelled = false;
    setWorldReadBusyPath(activePath);
    setWorldReadErr(null);
    readWorldConfigFile({ instanceId: activeInstance.id, worldId: activeWorld.id, path: activePath })
      .then((result) => {
        if (cancelled) return;
        setWorldDrafts((prev) => {
          if (prev[recordKey]) return prev;
          return {
            ...prev,
            [recordKey]: {
              content: String(result.content ?? result.readonly_reason ?? ""),
              updatedAt: Number(result.modified_at ?? Date.now()),
            },
          };
        });
      })
      .catch((err: any) => {
        if (cancelled) return;
        setWorldReadErr(err?.toString?.() ?? String(err));
      })
      .finally(() => {
        if (!cancelled) setWorldReadBusyPath(null);
      });

    return () => {
      cancelled = true;
    };
  }, [scope, activeInstance, activeWorld, activePath, activeFile, worldDrafts]);

  useEffect(() => {
    if (scope !== "instance" || !activeInstance || !activePath) return;
    const key = instanceRecordId(activeInstance.id, activePath);
    if (activeFile?.editable === false) {
      setBackupsBusy(false);
      setBackupsErr(null);
      setBackupsByFile((prev) => ({
        ...prev,
        [key]: [],
      }));
      setSelectedBackupIdByFile((prev) => {
        if (!(key in prev)) return prev;
        const next = { ...prev };
        delete next[key];
        return next;
      });
      return;
    }
    let cancelled = false;
    setBackupsBusy(true);
    setBackupsErr(null);
    listInstanceConfigFileBackups({ instanceId: activeInstance.id, path: activePath })
      .then((entries) => {
        if (cancelled) return;
        setBackupsByFile((prev) => ({
          ...prev,
          [key]: entries,
        }));
        setSelectedBackupIdByFile((prev) => {
          const existing = prev[key];
          if (existing && entries.some((item) => item.id === existing)) return prev;
          if (entries[0]?.id) {
            return { ...prev, [key]: entries[0].id };
          }
          if (!(key in prev)) return prev;
          const next = { ...prev };
          delete next[key];
          return next;
        });
      })
      .catch((err: any) => {
        if (cancelled) return;
        setBackupsByFile((prev) => ({
          ...prev,
          [key]: [],
        }));
        setBackupsErr(err?.toString?.() ?? String(err));
      })
      .finally(() => {
        if (!cancelled) setBackupsBusy(false);
      });
    return () => {
      cancelled = true;
    };
  }, [scope, activeInstance?.id, activePath, activeFile?.editable, activeRecord?.updatedAt]);

  const fileContent = getEffectiveFileContent(activeRecord);
  const savedContent = activeRecord?.content ?? "";
  const unsaved = hasUnsavedDraft(activeRecord);
  const isJson = Boolean(activePath && isJsonFilePath(activePath));

  useEffect(() => {
    if (!activeRecordKey || !activeRecord) return;
    const baseline = getEffectiveFileContent(activeRecord);
    setHistoryByFile((prev) => {
      const existing = prev[activeRecordKey];
      if (existing) return prev;
      return {
        ...prev,
        [activeRecordKey]: {
          entries: [baseline],
          index: 0,
        },
      };
    });
  }, [activeRecord, activeRecordKey]);

  const draftJson = useMemo(() => {
    if (!isJson) return null;
    return parseJsonWithError(fileContent);
  }, [fileContent, isJson]);

  const savedJson = useMemo(() => {
    if (!isJson) return null;
    return parseJsonWithError(savedContent);
  }, [savedContent, isJson]);

  const saveBlockedByJson = Boolean(isJson && draftJson && !draftJson.ok);
  let draftJsonIssue: JsonParseIssue | null = null;
  if (isJson && draftJson && draftJson.ok === false) {
    draftJsonIssue = draftJson.error;
  }

  const runningSet = useMemo(() => new Set(runningInstanceIds), [runningInstanceIds]);
  const instanceRunning = Boolean(activeInstance && runningSet.has(activeInstance.id));
  const worldReadOnly = scope === "world" && instanceRunning;
  const fileReadOnly = worldReadOnly || Boolean(activeFile?.editable === false);
  const formatterSupport = useMemo(
    () => (activePath ? getFormatterSupport(activePath, fileContent) : null),
    [activePath, fileContent]
  );
  const safeFixSupport = useMemo(
    () => (activePath ? applySafeFixes(activePath, fileContent) : null),
    [activePath, fileContent]
  );
  const configIssues = useMemo<ConfigIssue[]>(
    () => (activePath ? collectConfigIssues(activePath, fileContent) : []),
    [activePath, fileContent]
  );
  const issuesByPath = useMemo(() => groupIssuesByPath(configIssues), [configIssues]);
  const meaningfulIssueCount = useMemo(
    () => configIssues.filter((issue) => issue.severity !== "info").length,
    [configIssues]
  );
  const hasAutoFixCandidate = Boolean(
    safeFixSupport &&
      !safeFixSupport.blockingError &&
      (safeFixSupport.changed || meaningfulIssueCount > 0)
  );
  const canSave = Boolean(activeRecord && unsaved && !saveBlockedByJson && !fileReadOnly && !saveBusy);
  const canReset = Boolean(activeRecord && unsaved && !saveBusy);
  const canFormat = Boolean(formatterSupport?.supported && formatterSupport.canFormat && !fileReadOnly && !saveBusy);
  const canFixIssues = Boolean(activePath && activeRecord && !saveBusy && !fileReadOnly && hasAutoFixCandidate);
  const historyState = activeRecordKey ? historyByFile[activeRecordKey] : undefined;
  const canUndo = Boolean(historyState && historyState.index > 0);
  const canRedo = Boolean(historyState && historyState.index < historyState.entries.length - 1);
  const activeInstanceRecordKey =
    scope === "instance" && activeInstance && activePath
      ? instanceRecordId(activeInstance.id, activePath)
      : null;
  const activeBackups = activeInstanceRecordKey ? backupsByFile[activeInstanceRecordKey] ?? [] : [];
  const selectedBackupId = activeInstanceRecordKey
    ? selectedBackupIdByFile[activeInstanceRecordKey] ?? ""
    : "";
  const canRestoreBackup = Boolean(
    scope === "instance" &&
      activeInstance &&
      activePath &&
      selectedBackupId &&
      !restoreBusy &&
      !saveBusy &&
      activeFile?.editable !== false
  );

  function patchActiveRecord(mutator: (current: ConfigFileRecord) => ConfigFileRecord) {
    if (!activeInstance || !activePath || !activeRecord) return;
    if (scope === "instance") {
      const key = instanceRecordId(activeInstance.id, activePath);
      setInstanceDrafts((prev) => ({
        ...prev,
        [key]: mutator(prev[key]),
      }));
      return;
    }
    if (!activeWorld) return;
    const recordKey = worldRecordId(activeInstance.id, activeWorld.id, activePath);
    setWorldDrafts((prev) => ({
      ...prev,
      [recordKey]: mutator(prev[recordKey]),
    }));
  }

  function setDraft(next: string, opts?: { pushHistory?: boolean }) {
    if (!activeRecord) return;
    const pushHistory = opts?.pushHistory !== false;
    patchActiveRecord((current) => {
      if (next === current.content) {
        return {
          ...current,
          draft: undefined,
          draftUpdatedAt: undefined,
        };
      }
      return {
        ...current,
        draft: next,
        draftUpdatedAt: Date.now(),
      };
    });
    if (pushHistory && activeRecordKey) {
      setHistoryByFile((prev) => {
        const existing = prev[activeRecordKey] ?? { entries: [fileContent], index: 0 };
        const currentValue = existing.entries[existing.index] ?? "";
        if (currentValue === next) return prev;
        const nextEntries = [...existing.entries.slice(0, existing.index + 1), next].slice(-120);
        return {
          ...prev,
          [activeRecordKey]: {
            entries: nextEntries,
            index: nextEntries.length - 1,
          },
        };
      });
    }
  }

  async function onSave() {
    if (!canSave || !activeRecord || !activePath || !activeInstance) return;
    setSaveErr(null);
    if (scope === "instance") {
      setSaveBusy(true);
      const nextContent = getEffectiveFileContent(activeRecord);
      try {
        const out = await writeInstanceConfigFile({
          instanceId: activeInstance.id,
          path: activePath,
          content: nextContent,
          expectedModifiedAt: activeRecord.updatedAt,
        });
        const recordKey = instanceRecordId(activeInstance.id, activePath);
        setInstanceDrafts((prev) => ({
          ...prev,
          [recordKey]: {
            content: nextContent,
            updatedAt: Number(out.modified_at ?? Date.now()),
            draft: undefined,
            draftUpdatedAt: undefined,
          },
        }));
        setInstanceFilesByInstance((prev) => {
          const files = prev[activeInstance.id] ?? [];
          return {
            ...prev,
            [activeInstance.id]: files.map((item) =>
              item.path === activePath
                ? {
                    ...item,
                    modified_at: Number(out.modified_at ?? item.modified_at),
                    size_bytes: Number(out.size_bytes ?? item.size_bytes),
                  }
                : item
            ),
          };
        });
        setEditorInfo(out.message || "Instance file saved.");
      } catch (err: any) {
        setSaveErr(err?.toString?.() ?? String(err));
      } finally {
        setSaveBusy(false);
      }
      return;
    }
    if (!activeWorld || activeFile?.editable === false || worldReadOnly) return;

    setSaveBusy(true);
    setSaveErr(null);
    const nextContent = getEffectiveFileContent(activeRecord);
    try {
      const out = await writeWorldConfigFile({
        instanceId: activeInstance.id,
        worldId: activeWorld.id,
        path: activePath,
        content: nextContent,
        expectedModifiedAt: activeRecord.updatedAt,
      });
      const recordKey = worldRecordId(activeInstance.id, activeWorld.id, activePath);
      setWorldDrafts((prev) => ({
        ...prev,
        [recordKey]: {
          content: nextContent,
          updatedAt: Number(out.modified_at ?? Date.now()),
          draft: undefined,
          draftUpdatedAt: undefined,
        },
      }));
      const worldKey = worldScopeId(activeInstance.id, activeWorld.id);
      setWorldFilesByScope((prev) => {
        const filesForWorld = prev[worldKey] ?? [];
        return {
          ...prev,
          [worldKey]: filesForWorld.map((item) =>
            item.path === activePath
              ? {
                  ...item,
                  modified_at: Number(out.modified_at ?? item.modified_at),
                  size_bytes: Number(out.size_bytes ?? item.size_bytes),
                }
              : item
          ),
        };
      });
      setEditorInfo("World file saved.");
    } catch (err: any) {
      setSaveErr(err?.toString?.() ?? String(err));
    } finally {
      setSaveBusy(false);
    }
  }

  function onReset() {
    if (!canReset) return;
    setDraft(savedContent);
  }

  function onUndo() {
    if (!activeRecordKey) return;
    const history = historyByFile[activeRecordKey];
    if (!history || history.index <= 0) return;
    const nextIndex = history.index - 1;
    const nextValue = history.entries[nextIndex] ?? "";
    setDraft(nextValue, { pushHistory: false });
    setHistoryByFile((prev) => ({
      ...prev,
      [activeRecordKey]: {
        ...history,
        index: nextIndex,
      },
    }));
  }

  function onRedo() {
    if (!activeRecordKey) return;
    const history = historyByFile[activeRecordKey];
    if (!history || history.index >= history.entries.length - 1) return;
    const nextIndex = history.index + 1;
    const nextValue = history.entries[nextIndex] ?? "";
    setDraft(nextValue, { pushHistory: false });
    setHistoryByFile((prev) => ({
      ...prev,
      [activeRecordKey]: {
        ...history,
        index: nextIndex,
      },
    }));
  }

  function onFormat() {
    if (!activePath || !canFormat || fileReadOnly) return;
    const result = formatConfigContent(activePath, fileContent);
    if (result.blockingError) {
      setSaveErr(result.blockingError);
      return;
    }
    if (result.changed) {
      setDraft(result.output);
    }
    const notes = result.diagnostics.map((diag) => diag.message);
    if (notes.length > 0) {
      setEditorInfo(notes.join(" "));
    } else if (!result.changed) {
      setEditorInfo("Already formatted.");
    }
    setSaveErr(null);
  }

  function onFixIssues() {
    if (!activePath || !activeRecord) return;
    if (fileReadOnly) {
      setEditorInfo(readOnlyMessage ?? "This file is read-only, so no fixes can be applied.");
      return;
    }
    if (!safeFixSupport || safeFixSupport.blockingError) {
      setEditorInfo(safeFixSupport?.blockingError ?? "No safe automatic fixes are available for this file.");
      return;
    }
    if (meaningfulIssueCount === 0 && !safeFixSupport.changed) {
      setEditorInfo("No fixable issues detected in this file.");
      return;
    }
    const result = applySafeFixes(activePath, fileContent);
    if (result.blockingError) {
      setEditorInfo(result.blockingError);
      return;
    }
    if (result.changed) {
      setDraft(result.output);
    }
    const unresolved = result.issues.length;
    if (result.notes.length > 0) {
      setEditorInfo(`${result.notes.join(" ")} ${unresolved > 0 ? `${unresolved} issue(s) still need manual edits.` : "No remaining issues."}`);
    } else if (result.changed) {
      setEditorInfo(unresolved > 0 ? `Applied safe fixes. ${unresolved} issue(s) still need manual edits.` : "Applied safe fixes.");
    } else {
      if (unresolved > 0) {
        const examples = result.issues
          .slice(0, 2)
          .map((issue) => issue.message)
          .join(" ");
        setEditorInfo(
          examples
            ? `No safe automatic fix available for ${unresolved} issue(s). ${examples}`
            : `No safe automatic fix available for ${unresolved} issue(s).`
        );
      } else {
        setEditorInfo("No safe fixes needed.");
      }
    }
    setSaveErr(null);
  }

  function onSimpleChange(nextValue: unknown) {
    setDraft(asPrettyJson(nextValue));
  }

  async function onOpenInFinder() {
    if (!activeInstance || !activePath) return;
    setSaveErr(null);
    try {
      const out =
        scope === "world" && activeWorld
          ? await revealConfigEditorFile({
              instanceId: activeInstance.id,
              scope: "world",
              worldId: activeWorld.id,
              path: activePath,
            })
          : await revealConfigEditorFile({
              instanceId: activeInstance.id,
              scope: "instance",
              path: activePath,
            });
      setEditorInfo(out.message);
    } catch (err: any) {
      setSaveErr(err?.toString?.() ?? String(err));
    }
  }

  async function onCreateFile(path: string) {
    if (!activeInstance || scope !== "instance") return;
    setSaveErr(null);
    setSaveBusy(true);
    const content = asPrettyJson({});
    try {
      const out = await writeInstanceConfigFile({
        instanceId: activeInstance.id,
        path,
        content,
      });
      const recordKey = instanceRecordId(activeInstance.id, path);
      setInstanceDrafts((prev) => ({
        ...prev,
        [recordKey]: {
          content,
          updatedAt: Number(out.modified_at ?? Date.now()),
        },
      }));
      const refreshed = await listInstanceConfigFiles({ instanceId: activeInstance.id });
      setInstanceFilesByInstance((prev) => ({
        ...prev,
        [activeInstance.id]: refreshed,
      }));
      setSelectedPathByScope((prev) => ({
        ...prev,
        [`instance:${activeInstance.id}`]: path,
      }));
      setMode("simple");
      setShowNewFileModal(false);
      setEditorInfo("Created instance config file.");
    } catch (err: any) {
      setSaveErr(err?.toString?.() ?? String(err));
    } finally {
      setSaveBusy(false);
    }
  }

  async function onRestoreBackup() {
    if (!canRestoreBackup || !activeInstance || !activePath || !selectedBackupId) return;
    setSaveErr(null);
    setRestoreBusy(true);
    try {
      const out = await restoreInstanceConfigFileBackup({
        instanceId: activeInstance.id,
        path: activePath,
        backupId: selectedBackupId,
      });
      const latest = await readInstanceConfigFile({
        instanceId: activeInstance.id,
        path: activePath,
      });
      const recordKey = instanceRecordId(activeInstance.id, activePath);
      setInstanceDrafts((prev) => ({
        ...prev,
        [recordKey]: {
          content: String(latest.content ?? ""),
          updatedAt: Number(out.modified_at ?? Date.now()),
          draft: undefined,
          draftUpdatedAt: undefined,
        },
      }));
      const refreshedFiles = await listInstanceConfigFiles({ instanceId: activeInstance.id });
      const refreshedBackups = await listInstanceConfigFileBackups({
        instanceId: activeInstance.id,
        path: activePath,
      });
      setInstanceFilesByInstance((prev) => ({
        ...prev,
        [activeInstance.id]: refreshedFiles,
      }));
      setBackupsByFile((prev) => ({
        ...prev,
        [recordKey]: refreshedBackups,
      }));
      setSelectedBackupIdByFile((prev) => ({
        ...prev,
        [recordKey]: refreshedBackups[0]?.id ?? "",
      }));
      setEditorInfo(out.message || "Backup restored.");
    } catch (err: any) {
      setSaveErr(err?.toString?.() ?? String(err));
    } finally {
      setRestoreBusy(false);
    }
  }

  const readOnlyMessage =
    scope === "world" && instanceRunning
      ? "World file editing is read-only while this instance is running."
      : activeFile?.editable === false
        ? activeFile.readonlyReason ?? "This file type cannot be edited here."
        : null;
  const virtualFile = false;
  const formatTitle = fileReadOnly
    ? readOnlyMessage ?? "This file is read-only."
    : formatterSupport?.reason || "Apply safe formatting for this file type.";
  const openInFinderTitle =
    scope === "instance"
      ? "Reveal this file in your file manager."
      : "Reveal this file in your file manager.";
  const siblingContents = useMemo(() => {
    if (!activeInstance || !activePath) return [];
    if (scope === "instance") {
      return instanceFiles
        .filter((item) => item.path !== activePath)
        .slice(0, 20)
        .map((item) => {
          const key = instanceRecordId(activeInstance.id, item.path);
          return {
            path: item.path,
            content: getEffectiveFileContent(instanceDrafts[key]),
          };
        });
    }
    if (!activeWorld) return [];
    const worldScope = worldScopeId(activeInstance.id, activeWorld.id);
    const worldFiles = worldFilesByScope[worldScope] ?? [];
    const out: Array<{ path: string; content: string }> = [];
    for (const file of worldFiles) {
      if (file.path === activePath) continue;
      const key = worldRecordId(activeInstance.id, activeWorld.id, file.path);
      const record = worldDrafts[key];
      if (!record) continue;
      out.push({
        path: file.path,
        content: getEffectiveFileContent(record),
      });
      if (out.length >= 20) break;
    }
    return out;
  }, [
    activeInstance,
    activePath,
    activeWorld,
    instanceDrafts,
    instanceFiles,
    scope,
    worldDrafts,
    worldFilesByScope,
  ]);

  const searchMatches = useMemo(() => {
    const query = editorSearch.trim().toLowerCase();
    if (!query) return [] as Array<{ start: number; end: number }>;
    const haystack = fileContent.toLowerCase();
    const matches: Array<{ start: number; end: number }> = [];
    let cursor = 0;
    while (cursor < haystack.length) {
      const idx = haystack.indexOf(query, cursor);
      if (idx < 0) break;
      matches.push({ start: idx, end: idx + query.length });
      cursor = idx + Math.max(1, query.length);
      if (matches.length >= 500) break;
    }
    return matches;
  }, [editorSearch, fileContent]);

  const inspectorPathText = useMemo(() => {
    if (isJson) return describePath(inspectorPath ?? []);
    return String(nonJsonInspectorSelection?.path ?? "line");
  }, [inspectorPath, isJson, nonJsonInspectorSelection?.path]);

  const inspectorDoc = useMemo(
    () => (activePath ? getConfigDocForPath(activePath, inspectorPathText) : null),
    [activePath, inspectorPathText]
  );

  const inspectorIssues = useMemo(() => {
    const candidates = new Set<string>();
    const normalized = inspectorPathText.replace(/^root\.?/, "") || "root";
    candidates.add(normalized);
    candidates.add(inspectorPathText);
    if (normalized.includes("[")) {
      candidates.add(normalized.replace(/\[[0-9]+\]/g, ""));
    }
    const out: ConfigIssue[] = [];
    for (const key of candidates) {
      for (const issue of issuesByPath[key] ?? []) {
        if (!out.includes(issue)) out.push(issue);
      }
    }
    return out;
  }, [inspectorPathText, issuesByPath]);

  if (!activeInstance) {
    return (
      <div className="card" style={{ padding: 18, marginTop: 14 }}>
        <div className="settingTitle">Config Editor</div>
        <div className="settingSub">Create an instance first to edit config files.</div>
      </div>
    );
  }

  return (
    <div className="configWorkspace">
      <div className="card configWorkspaceHeader">
        <div className="configHeaderLeft">
          <div className="configHeaderEyebrow">Live config workspace</div>
          <div className="settingTitle">Config Editor</div>
          <div className="settingSub">
            Choose target, edit, and save. Instance files auto-back up before every write.
          </div>
        </div>
        <div className="configHeaderControls">
          <div className="configHeaderControlGrid">
            <div className="configHeaderPicker configHeaderPickerInstance">
              <span className="configHeaderLabel">Instance</span>
              <InstancePickerDropdown
                instances={instances.map((item) => ({ id: item.id, name: item.name }))}
                value={activeInstance.id}
                onChange={onSelectInstance}
              />
            </div>

            <div className="configHeaderPicker configHeaderPickerTarget">
              <span className="configHeaderLabel">Target</span>
              <div className="segmented configScopeToggle">
                <button
                  type="button"
                  className={`segBtn ${scope === "instance" ? "active" : ""}`}
                  onClick={() => setScope("instance")}
                >
                  Instance
                </button>
                <button
                  type="button"
                  className={`segBtn ${scope === "world" ? "active" : ""}`}
                  onClick={() => setScope("world")}
                >
                  World
                </button>
              </div>
            </div>

            {scope === "world" ? (
              <div className="configHeaderPicker configHeaderPickerWorld">
                <span className="configHeaderLabel">World</span>
                <InstancePickerDropdown
                  instances={activeWorlds.map((world) => ({ id: world.id, name: world.name }))}
                  value={activeWorld?.id ?? null}
                  onChange={(worldId) => {
                    setSelectedWorldByInstance((prev) => ({
                      ...prev,
                      [activeInstance.id]: worldId,
                    }));
                  }}
                  placeholder={worldBusy ? "Loading worlds..." : "No worlds found"}
                />
              </div>
            ) : null}

            <div className={`configHeaderMetaRow ${scope === "world" ? "withWorld" : "noWorld"}`}>
              <button className="btn" type="button" onClick={onManageInstances}>
                Manage instances
              </button>
              <span className="chip subtle">
                {scope === "instance" ? "Editing instance" : "Editing world"} • {activeInstance.name}
                {scope === "world" && activeWorld ? ` • ${activeWorld.name}` : ""}
              </span>
            </div>
          </div>
          <div className="configHeaderStatusStrip">
            <span className="chip subtle">{files.length} files</span>
            <span className="chip subtle">{scope === "instance" ? "Instance scope" : "World scope"}</span>
            <span className={`chip subtle ${runningInstanceIds.includes(activeInstance.id) ? "" : "configHeaderStatusMuted"}`}>
              {runningInstanceIds.includes(activeInstance.id) ? "Instance running" : "Instance stopped"}
            </span>
          </div>
        </div>
      </div>

      {scope === "instance" && instanceFilesErr ? <div className="errorBox">{instanceFilesErr}</div> : null}
      {scope === "world" && worldErr ? <div className="errorBox">{worldErr}</div> : null}
      {scope === "world" && worldFilesErr ? <div className="errorBox">{worldFilesErr}</div> : null}
      {saveErr ? <div className="errorBox">{saveErr}</div> : null}
      {scope === "instance" && instanceReadErr ? <div className="errorBox">{instanceReadErr}</div> : null}
      {scope === "world" && worldReadErr ? <div className="errorBox">{worldReadErr}</div> : null}
      {scope === "instance" && backupsErr ? <div className="errorBox">{backupsErr}</div> : null}
      {editorInfo ? <div className="noticeBox">{editorInfo}</div> : null}

      <div className="configWorkspaceGrid">
        <ConfigFileList
          files={files}
          query={fileQuery}
          onQueryChange={setFileQuery}
          selectedPath={activePath}
          onSelect={(path) => {
            if (!activeScopeKey) return;
            setSelectedPathByScope((prev) => ({
              ...prev,
              [activeScopeKey]: path,
            }));
          }}
          onNewFile={() => setShowNewFileModal(true)}
          allowNewFile={scope === "instance"}
        />

        <div className="configWorkspacePanel configEditorPanel">
          {scope === "instance" && instanceFilesBusy && files.length === 0 ? (
            <div className="configSimpleEmpty">
              <div className="settingTitle">Loading instance files…</div>
            </div>
          ) : scope === "world" && worldFilesBusy && files.length === 0 ? (
            <div className="configSimpleEmpty">
              <div className="settingTitle">Loading world files…</div>
            </div>
          ) : activePath ? (
            <>
              <ConfigEditorTopBar
                filePath={activePath}
                unsaved={unsaved}
                mode={mode}
                onModeChange={setMode}
                onSave={() => {
                  void onSave();
                }}
                onReset={onReset}
                onUndo={onUndo}
                onRedo={onRedo}
                onFormat={onFormat}
                canSave={canSave}
                canReset={canReset}
                canUndo={canUndo}
                canRedo={canRedo}
                canFormat={canFormat}
                formatTitle={formatTitle}
                readOnly={fileReadOnly}
                virtualFile={virtualFile}
                readOnlyMessage={readOnlyMessage}
              />

              {draftJsonIssue ? (
                <div className="errorBox configJsonError">
                  {draftJsonIssue.message}
                  {draftJsonIssue.line && draftJsonIssue.column
                    ? ` (line ${draftJsonIssue.line}, column ${draftJsonIssue.column})`
                    : ""}
                </div>
              ) : null}
              {!draftJsonIssue && (formatterSupport?.diagnostics.length ?? 0) > 0 ? (
                <div className="noticeBox configFormatNotice">
                  {formatterSupport?.diagnostics.map((diag) => diag.message).join(" ")}
                </div>
              ) : null}
              <div className="configEditorToolsRow">
                <div className="configEditorToolsMain">
                  <button
                    className="btn"
                    type="button"
                    onClick={() => {
                      void onOpenInFinder();
                    }}
                    disabled={!activePath}
                    title={openInFinderTitle}
                  >
                    Open location
                  </button>
                  {unsaved || showDiff ? (
                    <button
                      className="btn"
                      type="button"
                      onClick={() => setShowDiff((prev) => !prev)}
                      disabled={!activePath || !activeRecord}
                      aria-pressed={showDiff}
                    >
                      {showDiff ? "Hide diff" : "Preview diff"}
                    </button>
                  ) : null}
                </div>
                <div className="configEditorToolsDivider" aria-hidden="true" />
                <div className="configEditorIssuesSlot">
                  {meaningfulIssueCount > 0 ? (
                    <button
                      className="btn warning"
                      type="button"
                      onClick={onFixIssues}
                      disabled={!canFixIssues}
                      title={
                        fileReadOnly
                          ? readOnlyMessage ?? "This file is read-only."
                          : safeFixSupport?.blockingError
                            ? safeFixSupport.blockingError
                            : "Applies conservative automatic fixes to known issues."
                      }
                    >
                      <span className="configIssueWarningIcon" aria-hidden="true">!</span>
                      Fix issues ({meaningfulIssueCount})
                    </button>
                  ) : (
                    <span className="configIssuesOk">
                      <span className="configIssuesOkDot" aria-hidden="true" />
                      No issues
                    </span>
                  )}
                </div>
              </div>
              <div className="configEditorUtilityRow">
                <div className="configEditorSearchWrap">
                  <input
                    className="input"
                    value={editorSearch}
                    onChange={(event) => setEditorSearch(event.target.value)}
                    placeholder="Search in file..."
                  />
                  <span className="chip subtle">
                    {searchMatches.length} matches
                  </span>
                </div>
              </div>
              {scope === "instance" ? (
                <div className="configBackupRow">
                  <span className="chip subtle">
                    {backupsBusy ? "Loading backups…" : `${activeBackups.length} backups`}
                  </span>
                  <select
                    className="input configBackupSelect"
                    value={selectedBackupId}
                    onChange={(event) => {
                      if (!activeInstanceRecordKey) return;
                      const backupId = event.target.value;
                      setSelectedBackupIdByFile((prev) => ({
                        ...prev,
                        [activeInstanceRecordKey]: backupId,
                      }));
                    }}
                    disabled={activeBackups.length === 0 || backupsBusy || restoreBusy}
                  >
                    {activeBackups.length === 0 ? (
                      <option value="">No backups yet</option>
                    ) : (
                      activeBackups.map((item) => (
                        <option key={item.id} value={item.id}>
                          {formatBackupTime(item.created_at)} ({item.size_bytes} bytes)
                        </option>
                      ))
                    )}
                  </select>
                  <button
                    className="btn"
                    type="button"
                    onClick={() => {
                      void onRestoreBackup();
                    }}
                    disabled={!canRestoreBackup}
                  >
                    {restoreBusy ? "Restoring…" : "Restore backup"}
                  </button>
                </div>
              ) : null}
              {showDiff ? (
                <div className="configDiffPanel">
                  <div className="configDiffHead">
                    <span className="chip subtle">Saved</span>
                    <span className="chip subtle">Draft</span>
                  </div>
                  <div className="configDiffGrid">
                    <pre>{savedContent || "(empty)"}</pre>
                    <pre>{fileContent || "(empty)"}</pre>
                  </div>
                </div>
              ) : null}

              {scope === "instance" && instanceReadBusyPath === activePath && !activeRecord ? (
                <div className="configSimpleEmpty">
                  <div className="settingTitle">Loading file…</div>
                </div>
              ) : scope === "world" && worldReadBusyPath === activePath && !activeRecord ? (
                <div className="configSimpleEmpty">
                  <div className="settingTitle">Loading file…</div>
                </div>
              ) : !activeRecord && activeFile?.editable === false ? (
                <div className="configSimpleEmpty">
                  <div className="settingTitle">Read-only file</div>
                  <div className="muted">Loading preview…</div>
                </div>
              ) : activeFile?.editable === false ? (
                <div className="configReadonlyPreview">
                  <div className="configReadonlyPreviewLabel">
                    {activeFile.readonlyReason ?? "Read-only file"} Preview
                  </div>
                  <AdvancedEditor
                    value={fileContent}
                    filePath={activePath}
                    siblingContents={siblingContents}
                    onChange={setDraft}
                    readOnly
                  />
                </div>
              ) : mode === "advanced" ? (
                <AdvancedEditor
                  value={fileContent}
                  filePath={activePath}
                  siblingContents={siblingContents}
                  onChange={setDraft}
                  readOnly={fileReadOnly}
                />
              ) : mode === "simple" ? (
                isJson ? (
                  draftJson?.ok ? (
                    activePath === SERVERS_DAT_PATH ? (
                      <ServersDatSimpleEditor
                        value={fileContent}
                        onChange={setDraft}
                        onSelectField={(selection) => {
                          setNonJsonInspectorSelection(selection);
                          setInspectorPath(null);
                        }}
                        readOnly={fileReadOnly}
                      />
                    ) : (
                      <JsonSimpleEditor
                        filePath={activePath}
                        value={draftJson.value}
                        savedValue={savedJson?.ok ? savedJson.value : {}}
                        focusPath={jsonFocusPath}
                        onFocusPathChange={setJsonFocusPath}
                        onValueChange={onSimpleChange}
                        onSelectPath={setInspectorPath}
                        issuesByPath={issuesByPath}
                        readOnly={fileReadOnly}
                      />
                    )
                  ) : (
                    <div className="configSimpleEmpty">
                      <div className="settingTitle">JSON draft is invalid</div>
                      <div className="muted">Fix JSON in Advanced mode to continue using Simple mode.</div>
                    </div>
                  )
                ) : (
                  <TextSimpleEditor
                    filePath={activePath}
                    value={fileContent}
                    onChange={setDraft}
                    issuesByPath={issuesByPath}
                    onSelectField={(selection) => {
                      setNonJsonInspectorSelection(selection);
                      setInspectorPath(null);
                    }}
                    readOnly={fileReadOnly}
                  />
                )
              ) : null}
            </>
          ) : (
            <div className="configSimpleEmpty">
              <div className="settingTitle">
                {scope === "world" && !activeWorld
                  ? "No worlds found"
                  : "No file selected"}
              </div>
              {scope === "world" && !activeWorld ? (
                <div className="muted">Create a world in Minecraft first, then return here.</div>
              ) : null}
            </div>
          )}
        </div>

        <InspectorPanel
          filePath={activePath ?? "No file"}
          isJson={Boolean(isJson && draftJson?.ok)}
          rootValue={draftJson && draftJson.ok ? draftJson.value : {}}
          selectedPath={inspectorPath}
          nonJsonPath={nonJsonInspectorSelection?.path ?? null}
          nonJsonValue={nonJsonInspectorSelection?.value ?? null}
          nonJsonType={nonJsonInspectorSelection?.type ?? null}
          doc={inspectorDoc}
          pathIssues={inspectorIssues}
        />
      </div>

      <NewFileModal
        open={showNewFileModal && scope === "instance"}
        onClose={() => setShowNewFileModal(false)}
        onCreate={(path) => {
          void onCreateFile(path);
        }}
        existingPaths={instanceFiles.map((item) => item.path)}
      />
    </div>
  );
}
