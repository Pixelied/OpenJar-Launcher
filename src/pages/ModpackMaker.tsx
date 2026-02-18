import { useEffect, useMemo, useRef, useState } from "react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/api/dialog";
import type {
  CreatorConflictSuggestion,
  Instance,
  Layer,
  LayerDiffResult,
  ModEntry,
  ModpackSpec,
  ResolutionPlan,
  ResolutionSettings,
} from "../types";
import {
  applyModpackPlan,
  applyTemplateLayerUpdate,
  deleteModpackSpec,
  duplicateModpackSpec,
  exportModpackSpecJson,
  getModpackSpec,
  importLocalJarsToModpackLayer,
  importModpackLayerFromProvider,
  importModpackLayerFromSpec,
  importModpackSpecJson,
  listModpackSpecs,
  migrateLegacyCreatorPresets,
  previewTemplateLayerUpdate,
  resolveLocalModpackEntries,
  resolveModpackForInstance,
  seedDevModpackData,
  upsertModpackSpec,
} from "../tauri";

type Props = {
  instances: Instance[];
  selectedInstanceId: string | null;
  autoIdentifyLocalJarsEnabled: boolean;
  onSelectInstance: (id: string) => void;
  onOpenDiscover: (context?: {
    modpackId: string;
    modpackName: string;
    layerId?: string | null;
    layerName?: string | null;
  }) => void;
  isDevMode: boolean;
  onNotice: (message: string) => void;
  onError: (message: string) => void;
};

type MakerView = "home" | "editor";
type EntryFilter = "all" | "mods" | "optional" | "pinned" | "failing" | "by_layer";

type EntryRow = {
  key: string;
  layerId: string;
  layerName: string;
  layerIndex: number;
  entryIndex: number;
  entry: ModEntry;
  identity: string;
  duplicateCount: number;
};

const MIGRATION_MARKER_KEY = "mpm.modpackMaker.migrated.v1";

const LAYER_HELP = {
  template:
    "Template layer: imported starter content. Use this for upstream template sync and base packs.",
  user:
    "User Additions layer: your normal add/remove list for this modpack.",
  overrides:
    "Overrides layer: explicit wins for conflicts, pinning, or forcing behavior over other layers.",
};

function layerHelpText(layer: Layer): string {
  const id = layer.id.toLowerCase();
  const name = layer.name.toLowerCase();
  if (id.includes("template") || name.includes("template")) return LAYER_HELP.template;
  if (id.includes("override") || name.includes("override")) return LAYER_HELP.overrides;
  if (id.includes("user") || name.includes("user")) return LAYER_HELP.user;
  return "Layer content merges top-to-bottom. Later layers can override earlier ones.";
}

function defaultSettings(): ResolutionSettings {
  return {
    global_fallback_mode: "smart",
    channel_allowance: "stable",
    allow_cross_minor: true,
    allow_cross_major: false,
    prefer_stable: true,
    max_fallback_distance: 3,
    dependency_mode: "detect_only",
    partial_apply_unsafe: false,
  };
}

function baseSpec(name: string): ModpackSpec {
  const now = new Date().toISOString();
  return {
    id: `modpack_${Date.now()}`,
    name,
    description: "",
    tags: [],
    created_at: now,
    updated_at: now,
    layers: [
      {
        id: "layer_template",
        name: "Template",
        is_frozen: false,
        entries_delta: { add: [], remove: [], override: [] },
      },
      {
        id: "layer_user",
        name: "User Additions",
        is_frozen: false,
        entries_delta: { add: [], remove: [], override: [] },
      },
      {
        id: "layer_overrides",
        name: "Overrides",
        is_frozen: false,
        entries_delta: { add: [], remove: [], override: [] },
      },
    ],
    profiles: [
      { id: "lite", name: "Lite", optional_entry_states: {} },
      { id: "recommended", name: "Recommended", optional_entry_states: {} },
      { id: "full", name: "Full", optional_entry_states: {} },
    ],
    settings: defaultSettings(),
  };
}

function emptyEntry(): ModEntry {
  return {
    provider: "modrinth",
    project_id: "",
    slug: "",
    content_type: "mods",
    required: true,
    pin: null,
    channel_policy: "stable",
    fallback_policy: "inherit",
    replacement_group: "",
    notes: "",
    disabled_by_default: false,
    optional: false,
    target_scope: "instance",
    target_worlds: [],
  };
}

function cloneSpec(spec: ModpackSpec): ModpackSpec {
  return JSON.parse(JSON.stringify(spec));
}

function entryIdentity(entry: ModEntry): string {
  return `${entry.provider}:${entry.content_type}:${String(entry.project_id ?? "").trim().toLowerCase()}`;
}

function entryDisplayName(entry: ModEntry): string {
  const notes = String(entry.notes ?? "").trim();
  const project = String(entry.project_id ?? "").trim();
  const slug = String(entry.slug ?? "").trim();
  if (notes && notes.toLowerCase() !== project.toLowerCase()) return notes;
  if (slug) return slug;
  return project || "Unnamed entry";
}

function firstUserLayerId(spec: ModpackSpec): string | null {
  const byId = spec.layers.find((layer) => layer.id === "layer_user");
  if (byId) return byId.id;
  const byName = spec.layers.find((layer) => layer.name.trim().toLowerCase().includes("user"));
  if (byName) return byName.id;
  return spec.layers[0]?.id ?? null;
}

function fmtDate(value?: string | null): string {
  if (!value) return "n/a";
  const d = new Date(value);
  if (Number.isNaN(d.getTime())) return value;
  return d.toLocaleString();
}

export default function ModpackMaker({
  instances,
  selectedInstanceId,
  autoIdentifyLocalJarsEnabled,
  onSelectInstance,
  onOpenDiscover,
  isDevMode,
  onNotice,
  onError,
}: Props) {
  const [view, setView] = useState<MakerView>("home");
  const [busy, setBusy] = useState(false);
  const [migrationPending, setMigrationPending] = useState(false);

  const [specs, setSpecs] = useState<ModpackSpec[]>([]);
  const [selectedSpecId, setSelectedSpecId] = useState<string | null>(null);
  const [homeSearch, setHomeSearch] = useState("");
  const [homeActionsOpen, setHomeActionsOpen] = useState(false);
  const homeActionsRef = useRef<HTMLDivElement | null>(null);

  const [editorSpec, setEditorSpec] = useState<ModpackSpec | null>(null);
  const [selectedLayerId, setSelectedLayerId] = useState<string | null>(null);
  const [selectedEntryKey, setSelectedEntryKey] = useState<string | null>(null);
  const [openLayerMenuId, setOpenLayerMenuId] = useState<string | null>(null);
  const [entrySearch, setEntrySearch] = useState("");
  const [entryFilter, setEntryFilter] = useState<EntryFilter>("all");
  const [filterLayerId, setFilterLayerId] = useState<string>("");

  const [addModalOpen, setAddModalOpen] = useState(false);
  const [entryDraft, setEntryDraft] = useState<ModEntry>(emptyEntry());
  const [importLocalJarsBusy, setImportLocalJarsBusy] = useState(false);
  const [identifyLocalJarsBusy, setIdentifyLocalJarsBusy] = useState(false);

  const [providerImport, setProviderImport] = useState({
    source: "modrinth",
    projectId: "",
    layerName: "Imported Template",
  });
  const [specImport, setSpecImport] = useState({
    sourceModpackId: "",
    layerName: "Imported Modpack Layer",
  });
  const [templateDiff, setTemplateDiff] = useState<LayerDiffResult | null>(null);

  const [plan, setPlan] = useState<ResolutionPlan | null>(null);
  const [applyWizardOpen, setApplyWizardOpen] = useState(false);
  const [applySpecId, setApplySpecId] = useState("");
  const [applyProfileId, setApplyProfileId] = useState("recommended");
  const [applyLinkMode, setApplyLinkMode] = useState<"linked" | "unlinked">("linked");
  const [applySettings, setApplySettings] = useState<ResolutionSettings>(defaultSettings());
  const [applySettingsOpen, setApplySettingsOpen] = useState(false);
  const [applyInstanceId, setApplyInstanceId] = useState<string>(selectedInstanceId ?? "");
  const [conflictWizardOpen, setConflictWizardOpen] = useState(false);
  const [selectedConflictSuggestionIds, setSelectedConflictSuggestionIds] = useState<string[]>([]);

  const selectedSpec = useMemo(
    () => specs.find((spec) => spec.id === selectedSpecId) ?? null,
    [specs, selectedSpecId]
  );

  const selectedLayer = useMemo(() => {
    if (!editorSpec || !selectedLayerId) return null;
    return editorSpec.layers.find((layer) => layer.id === selectedLayerId) ?? null;
  }, [editorSpec, selectedLayerId]);

  const allEntries = useMemo<EntryRow[]>(() => {
    if (!editorSpec) return [];
    const baseRows = editorSpec.layers.flatMap((layer, layerIndex) =>
      layer.entries_delta.add.map((entry, entryIndex) => {
        const identity = entryIdentity(entry);
        return {
          key: `${layer.id}:${entryIndex}`,
          layerId: layer.id,
          layerName: layer.name,
          layerIndex,
          entryIndex,
          entry,
          identity,
          duplicateCount: 1,
        } as EntryRow;
      })
    );
    const counts = new Map<string, number>();
    for (const row of baseRows) {
      counts.set(row.identity, (counts.get(row.identity) ?? 0) + 1);
    }
    return baseRows.map((row) => ({ ...row, duplicateCount: counts.get(row.identity) ?? 1 }));
  }, [editorSpec]);

  const entryCountsByType = useMemo(() => {
    const counts = { mods: 0, resourcepacks: 0, shaderpacks: 0, datapacks: 0 };
    for (const row of allEntries) {
      const type = row.entry.content_type as keyof typeof counts;
      if (counts[type] != null) counts[type] += 1;
    }
    return counts;
  }, [allEntries]);

  const packConflictCount = useMemo(() => {
    return allEntries.filter((row) => row.duplicateCount > 1).length;
  }, [allEntries]);

  const conflictSuggestions = useMemo<CreatorConflictSuggestion[]>(() => {
    if (!editorSpec) return [];
    const suggestions: CreatorConflictSuggestion[] = [];
    const seen = new Set<string>();
    const addSuggestion = (item: CreatorConflictSuggestion) => {
      if (seen.has(item.id)) return;
      seen.add(item.id);
      suggestions.push(item);
    };

    const duplicateGroups = new Map<string, EntryRow[]>();
    for (const row of allEntries) {
      if (row.duplicateCount <= 1) continue;
      const list = duplicateGroups.get(row.identity);
      if (list) list.push(row);
      else duplicateGroups.set(row.identity, [row]);
    }
    for (const [identity, rows] of duplicateGroups.entries()) {
      const ordered = [...rows].sort((a, b) => a.layerIndex - b.layerIndex);
      const keep = ordered[ordered.length - 1];
      const remove = ordered.filter((row) => row.key !== keep.key);
      addSuggestion({
        id: `dup:${identity}:${keep.layerId}`,
        conflict_code: "LAYER_DUPLICATE",
        title: `Keep ${keep.layerName} for ${entryDisplayName(keep.entry)}`,
        detail: `Remove ${remove.length} duplicate entr${remove.length === 1 ? "y" : "ies"} from earlier layers.`,
        patch_preview: [
          `KEEP ${keep.layerName}: ${entryDisplayName(keep.entry)}`,
          ...remove.map((row) => `REMOVE ${row.layerName}: ${entryDisplayName(row.entry)}`),
        ].join("\n"),
        risk: "low",
      });
    }

    for (const row of allEntries) {
      const layer = editorSpec.layers.find((item) => item.id === row.layerId);
      if (!layer) continue;
      const isOverrideLayer =
        layer.id.toLowerCase().includes("override") || layer.name.toLowerCase().includes("override");
      if (!isOverrideLayer) continue;
      const hasBase = allEntries.some(
        (candidate) => candidate.identity === row.identity && candidate.layerIndex < row.layerIndex
      );
      if (hasBase) continue;
      addSuggestion({
        id: `override:${row.key}`,
        conflict_code: "OVERRIDE_WITHOUT_BASE",
        title: `Move ${entryDisplayName(row.entry)} to User Additions`,
        detail: "Override entry has no base entry. Convert it to a normal add entry.",
        patch_preview: [
          `ADD User Additions: ${entryDisplayName(row.entry)}`,
          `REMOVE ${layer.name}: ${entryDisplayName(row.entry)}`,
        ].join("\n"),
        risk: "low",
      });
    }

    for (const conflict of plan?.conflicts ?? []) {
      if (!String(conflict.code).toUpperCase().includes("FILE_COLLISION")) continue;
      const key = conflict.keys?.[0] ?? conflict.message;
      addSuggestion({
        id: `file:${key}`,
        conflict_code: "FILE_COLLISION",
        title: "Resolve file collision by single winner",
        detail: "Choose one entry as winner and remove alternate colliding entries.",
        patch_preview: `Resolve ${key}\nApply a single source/layer winner for this file key.`,
        risk: "medium",
      });
    }

    return suggestions.slice(0, 24);
  }, [editorSpec, allEntries, plan]);

  const failureKeySet = useMemo(() => {
    if (!plan || !editorSpec || plan.modpack_id !== editorSpec.id) return new Set<string>();
    return new Set(plan.failed_mods.map((item) => `${item.source}:${item.content_type}:${item.project_id}`.toLowerCase()));
  }, [plan, editorSpec]);

  const filteredEntries = useMemo(() => {
    let list = allEntries;
    if (entryFilter === "mods") {
      list = list.filter((row) => row.entry.content_type === "mods");
    } else if (entryFilter === "optional") {
      list = list.filter((row) => !row.entry.required || row.entry.optional);
    } else if (entryFilter === "pinned") {
      list = list.filter((row) => Boolean(row.entry.pin));
    } else if (entryFilter === "failing") {
      list = list.filter((row) => failureKeySet.has(entryIdentity(row.entry)));
    } else if (entryFilter === "by_layer") {
      list = list.filter((row) => row.layerId === (filterLayerId || selectedLayerId));
    }

    const q = entrySearch.trim().toLowerCase();
    if (q) {
      list = list.filter((row) => {
        const haystack = [
          entryDisplayName(row.entry),
          row.entry.project_id,
          row.entry.slug,
          row.layerName,
          row.entry.provider,
          row.entry.content_type,
        ]
          .filter(Boolean)
          .join(" ")
          .toLowerCase();
        return haystack.includes(q);
      });
    }
    return list;
  }, [allEntries, entryFilter, filterLayerId, selectedLayerId, entrySearch, failureKeySet]);

  const selectedEntryRow = useMemo(
    () => allEntries.find((row) => row.key === selectedEntryKey) ?? null,
    [allEntries, selectedEntryKey]
  );

  const selectedApplySpec = useMemo(
    () => specs.find((spec) => spec.id === applySpecId) ?? null,
    [specs, applySpecId]
  );

  const filteredHomeSpecs = useMemo(() => {
    const q = homeSearch.trim().toLowerCase();
    if (!q) return specs;
    return specs.filter((spec) => {
      const haystack = [spec.name, spec.description ?? "", ...(spec.tags ?? [])].join(" ").toLowerCase();
      return haystack.includes(q);
    });
  }, [specs, homeSearch]);

  useEffect(() => {
    setApplyInstanceId(selectedInstanceId ?? "");
  }, [selectedInstanceId]);

  useEffect(() => {
    const marker = localStorage.getItem(MIGRATION_MARKER_KEY);
    const raw = localStorage.getItem("mpm.presets.v2") ?? localStorage.getItem("mpm.presets.v1");
    setMigrationPending(!marker && Boolean(raw));
  }, []);

  useEffect(() => {
    if (!editorSpec) return;
    if (!selectedLayerId || !editorSpec.layers.some((layer) => layer.id === selectedLayerId)) {
      setSelectedLayerId(firstUserLayerId(editorSpec));
    }
    if (entryFilter === "by_layer" && filterLayerId && !editorSpec.layers.some((layer) => layer.id === filterLayerId)) {
      setFilterLayerId("");
    }
  }, [editorSpec, selectedLayerId, entryFilter, filterLayerId]);

  useEffect(() => {
    if (selectedEntryKey && !allEntries.some((row) => row.key === selectedEntryKey)) {
      setSelectedEntryKey(null);
    }
  }, [selectedEntryKey, allEntries]);

  useEffect(() => {
    if (!conflictWizardOpen) return;
    setSelectedConflictSuggestionIds(conflictSuggestions.map((item) => item.id));
  }, [conflictWizardOpen, conflictSuggestions]);

  useEffect(() => {
    if (!homeActionsOpen) return;
    const onDocMouseDown = (event: MouseEvent) => {
      const target = event.target as Node;
      if (homeActionsRef.current && !homeActionsRef.current.contains(target)) {
        setHomeActionsOpen(false);
      }
    };
    document.addEventListener("mousedown", onDocMouseDown);
    return () => document.removeEventListener("mousedown", onDocMouseDown);
  }, [homeActionsOpen]);

  useEffect(() => {
    if (selectedEntryKey) return;
    if (filteredEntries.length > 0) {
      setSelectedEntryKey(filteredEntries[0].key);
    }
  }, [selectedEntryKey, filteredEntries]);

  async function refreshSpecs() {
    const list = await listModpackSpecs();
    setSpecs(list);
    setSelectedSpecId((prev) => {
      if (prev && list.some((spec) => spec.id === prev)) return prev;
      return list[0]?.id ?? null;
    });
  }

  useEffect(() => {
    refreshSpecs().catch((err: any) => onError(err?.toString?.() ?? String(err)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function openEditor(specId: string) {
    setBusy(true);
    try {
      const spec = await getModpackSpec({ modpackId: specId });
      const copy = cloneSpec(spec);
      setEditorSpec(copy);
      setSelectedSpecId(copy.id);
      const defaultLayerId = firstUserLayerId(copy);
      setSelectedLayerId(defaultLayerId);
      setFilterLayerId(defaultLayerId ?? "");
      setSelectedEntryKey(null);
      setEntrySearch("");
      setEntryFilter("all");
      setTemplateDiff(null);
      setView("editor");
    } catch (err: any) {
      onError(err?.toString?.() ?? String(err));
    } finally {
      setBusy(false);
    }
  }

  async function createSpec() {
    setBusy(true);
    try {
      const created = await upsertModpackSpec({ spec: baseSpec("New Modpack") });
      await refreshSpecs();
      await openEditor(created.id);
      onNotice("Created new modpack spec.");
    } catch (err: any) {
      onError(err?.toString?.() ?? String(err));
    } finally {
      setBusy(false);
    }
  }

  async function saveEditorSpec() {
    if (!editorSpec) return;
    setBusy(true);
    try {
      const saved = await upsertModpackSpec({ spec: editorSpec });
      setEditorSpec(cloneSpec(saved));
      await refreshSpecs();
      onNotice("Modpack spec saved.");
    } catch (err: any) {
      onError(err?.toString?.() ?? String(err));
    } finally {
      setBusy(false);
    }
  }

  function updateEditorSpec(mutator: (copy: ModpackSpec) => void) {
    if (!editorSpec) return;
    const copy = cloneSpec(editorSpec);
    mutator(copy);
    setEditorSpec(copy);
  }

  function updateEntry(layerId: string, entryIndex: number, updater: (entry: ModEntry) => ModEntry) {
    updateEditorSpec((copy) => {
      const layerIdx = copy.layers.findIndex((layer) => layer.id === layerId);
      if (layerIdx < 0) return;
      const current = copy.layers[layerIdx].entries_delta.add[entryIndex];
      if (!current) return;
      copy.layers[layerIdx].entries_delta.add[entryIndex] = updater(current);
    });
  }

  function removeEntry(layerId: string, entryIndex: number) {
    updateEditorSpec((copy) => {
      const layerIdx = copy.layers.findIndex((layer) => layer.id === layerId);
      if (layerIdx < 0) return;
      copy.layers[layerIdx].entries_delta.add = copy.layers[layerIdx].entries_delta.add.filter((_, idx) => idx !== entryIndex);
    });
  }

  function openDiscoverForSelectedLayer() {
    if (!editorSpec) {
      onOpenDiscover();
      return;
    }
    onOpenDiscover({
      modpackId: editorSpec.id,
      modpackName: editorSpec.name,
      layerId: selectedLayer?.id ?? null,
      layerName: selectedLayer?.name ?? null,
    });
  }

  async function addLocalJarsFromComputer() {
    if (!editorSpec || !selectedLayer) {
      onError("Select a layer first.");
      return;
    }
    if (selectedLayer.is_frozen) {
      onError(`Layer "${selectedLayer.name}" is frozen. Unfreeze before adding content.`);
      return;
    }
    const picked = await openDialog({
      multiple: true,
      filters: [{ name: "Java archives", extensions: ["jar"] }],
    });
    if (!picked) return;
    const filePaths = Array.isArray(picked) ? picked : [picked];
    if (filePaths.length === 0) return;

    setImportLocalJarsBusy(true);
    try {
      const out = await importLocalJarsToModpackLayer({
        modpackId: editorSpec.id,
        layerId: selectedLayer.id,
        filePaths,
        autoIdentify: autoIdentifyLocalJarsEnabled,
      });
      setEditorSpec(cloneSpec(out.spec));
      const warningText =
        out.warnings.length > 0 ? ` ${out.warnings.slice(0, 2).join(" | ")}` : "";
      const resolvedText =
        out.resolved_entries > 0
          ? ` Identified ${out.resolved_entries} entr${out.resolved_entries === 1 ? "y" : "ies"}.`
          : "";
      onNotice(
        `Added ${out.added_entries} entr${out.added_entries === 1 ? "y" : "ies"} and updated ${out.updated_entries}.${resolvedText}${warningText}`
      );
    } catch (err: any) {
      onError(err?.toString?.() ?? String(err));
    } finally {
      setImportLocalJarsBusy(false);
    }
  }

  async function identifyLocalJarsInCreator(mode: "missing_only" | "all" = "all") {
    if (!editorSpec) {
      onError("Open a modpack first.");
      return;
    }
    setIdentifyLocalJarsBusy(true);
    try {
      const out = await resolveLocalModpackEntries({
        modpackId: editorSpec.id,
        mode,
      });
      setEditorSpec(cloneSpec(out.spec));
      if (out.resolved_entries > 0) {
        onNotice(
          `Identified ${out.resolved_entries} local entr${out.resolved_entries === 1 ? "y" : "ies"} (${out.remaining_local_entries} remaining local).`
        );
      } else if (out.warnings.length > 0) {
        onError(out.warnings[0] ?? "No local entries were identified.");
      } else {
        onNotice("No additional local JAR entries were identified.");
      }
    } catch (err: any) {
      onError(err?.toString?.() ?? String(err));
    } finally {
      setIdentifyLocalJarsBusy(false);
    }
  }

  function openAddModal() {
    if (!selectedLayer) {
      onError("Select a layer first.");
      return;
    }
    if (selectedLayer.is_frozen) {
      onError(`Layer "${selectedLayer.name}" is frozen. Unfreeze before adding content.`);
      return;
    }
    setEntryDraft(emptyEntry());
    setAddModalOpen(true);
  }

  function addDraftToSelectedLayer() {
    if (!editorSpec || !selectedLayer) return;
    if (!entryDraft.project_id.trim()) {
      onError("Project id/slug is required.");
      return;
    }
    if (selectedLayer.is_frozen) {
      onError(`Layer "${selectedLayer.name}" is frozen. Unfreeze before adding content.`);
      return;
    }
    const nextEntry = {
      ...entryDraft,
      project_id: entryDraft.project_id.trim(),
      slug: entryDraft.slug?.trim() || null,
      pin: entryDraft.pin?.trim() || null,
      notes: entryDraft.notes?.trim() || null,
      optional: !entryDraft.required,
      disabled_by_default: Boolean(entryDraft.disabled_by_default),
    };
    updateEditorSpec((copy) => {
      const layerIdx = copy.layers.findIndex((layer) => layer.id === selectedLayer.id);
      if (layerIdx < 0) return;
      copy.layers[layerIdx].entries_delta.add.push(nextEntry);
      const addedIdx = copy.layers[layerIdx].entries_delta.add.length - 1;
      setSelectedEntryKey(`${selectedLayer.id}:${addedIdx}`);
    });
    setAddModalOpen(false);
    onNotice(`Added ${nextEntry.project_id} to ${selectedLayer.name}.`);
  }

  function openApplyWizard(initialModpackId?: string) {
    const modpackId = initialModpackId ?? editorSpec?.id ?? selectedSpec?.id ?? specs[0]?.id ?? "";
    if (!modpackId) {
      onError("Create or select a modpack first.");
      return;
    }
    const spec = specs.find((item) => item.id === modpackId) ?? null;
    setApplySpecId(modpackId);
    setApplyProfileId(spec?.profiles[0]?.id ?? "recommended");
    setApplySettings(spec?.settings ?? defaultSettings());
    setApplySettingsOpen(false);
    setApplyLinkMode("linked");
    setApplyInstanceId(selectedInstanceId ?? instances[0]?.id ?? "");
    setPlan(null);
    setApplyWizardOpen(true);
  }

  function applySelectedConflictSuggestions() {
    if (!editorSpec) return;
    const selectedSet = new Set(selectedConflictSuggestionIds);
    if (selectedSet.size === 0) {
      onNotice("Select at least one suggestion.");
      return;
    }
    const copy = cloneSpec(editorSpec);
    let applied = 0;

    for (const suggestion of conflictSuggestions) {
      if (!selectedSet.has(suggestion.id)) continue;
      if (suggestion.id.startsWith("dup:")) {
        const payload = suggestion.id.slice(4);
        const lastColon = payload.lastIndexOf(":");
        if (lastColon <= 0) continue;
        const identity = payload.slice(0, lastColon);
        const keepLayerId = payload.slice(lastColon + 1);
        let changed = false;
        for (const layer of copy.layers) {
          if (layer.id === keepLayerId) continue;
          const before = layer.entries_delta.add.length;
          layer.entries_delta.add = layer.entries_delta.add.filter(
            (entry) => entryIdentity(entry) !== identity
          );
          if (layer.entries_delta.add.length !== before) changed = true;
        }
        if (changed) applied += 1;
      } else if (suggestion.id.startsWith("override:")) {
        const rowKey = suggestion.id.slice("override:".length);
        const row = allEntries.find((item) => item.key === rowKey);
        if (!row) continue;
        const sourceLayer = copy.layers.find((layer) => layer.id === row.layerId);
        if (!sourceLayer) continue;
        const targetLayerId = firstUserLayerId(copy);
        const targetLayer = targetLayerId
          ? copy.layers.find((layer) => layer.id === targetLayerId)
          : null;
        if (!targetLayer || targetLayer.id === sourceLayer.id) continue;
        const entry = sourceLayer.entries_delta.add[row.entryIndex];
        if (!entry) continue;
        sourceLayer.entries_delta.add = sourceLayer.entries_delta.add.filter(
          (_, idx) => idx !== row.entryIndex
        );
        targetLayer.entries_delta.add.push({ ...entry });
        applied += 1;
      } else if (suggestion.id.startsWith("file:")) {
        const duplicates = new Map<string, EntryRow[]>();
        for (const row of allEntries) {
          if (row.duplicateCount <= 1) continue;
          const rows = duplicates.get(row.identity);
          if (rows) rows.push(row);
          else duplicates.set(row.identity, [row]);
        }
        let changedAny = false;
        for (const [identity, rows] of duplicates.entries()) {
          const keep = [...rows].sort((a, b) => a.layerIndex - b.layerIndex).pop();
          if (!keep) continue;
          for (const layer of copy.layers) {
            if (layer.id === keep.layerId) continue;
            const before = layer.entries_delta.add.length;
            layer.entries_delta.add = layer.entries_delta.add.filter(
              (entry) => entryIdentity(entry) !== identity
            );
            if (layer.entries_delta.add.length !== before) changedAny = true;
          }
        }
        if (changedAny) applied += 1;
      }
    }

    setEditorSpec(copy);
    setConflictWizardOpen(false);
    if (applied > 0) {
      onNotice(`Applied ${applied} conflict suggestion patch${applied === 1 ? "" : "es"}.`);
    } else {
      onNotice("No matching changes were applied from the selected suggestions.");
    }
  }

  const templateLayerSelected = Boolean(
    selectedLayer?.source && selectedLayer.source.kind === "provider_template"
  );

  return (
    <div className="mpmPage" style={{ maxWidth: 1440, margin: "0 auto" }}>
      {migrationPending ? (
        <div className="card" style={{ marginTop: 12, padding: 14, borderRadius: 16 }}>
          <div style={{ fontWeight: 900 }}>Legacy Creator migration available</div>
          <div className="muted" style={{ marginTop: 4 }}>
            Found old presets in local storage. Migration creates new ModpackSpec entries and reports skipped items.
          </div>
          <div className="row" style={{ marginTop: 8, gap: 8 }}>
            <button
              className="btn primary"
              disabled={busy}
              onClick={async () => {
                const raw = localStorage.getItem("mpm.presets.v2") ?? localStorage.getItem("mpm.presets.v1") ?? "[]";
                setBusy(true);
                try {
                  const payload = JSON.parse(raw);
                  const report = await migrateLegacyCreatorPresets({ payload });
                  localStorage.setItem(MIGRATION_MARKER_KEY, new Date().toISOString());
                  setMigrationPending(false);
                  await refreshSpecs();
                  onNotice(`Migrated ${report.migrated_count} preset(s), skipped ${report.skipped_count}.`);
                } catch (err: any) {
                  onError(err?.toString?.() ?? String(err));
                } finally {
                  setBusy(false);
                }
              }}
            >
              Run migration
            </button>
          </div>
        </div>
      ) : null}

      {view === "home" ? (
        <>
          <div className="card mpmShellCard mpmHomeCard" style={{ position: "relative", zIndex: 4 }}>
            <div className="mpmHomeHeader">
              <div className="mpmHeaderBlock">
                <div className="h2">Modpacks</div>
                <div className="muted" style={{ marginTop: 4 }}>
                  Build modpack specs, then preview and apply with clear results.
                </div>
              </div>
              <div className="mpmHomeActions">
                <button className="btn primary" disabled={busy} onClick={createSpec} title="Create a new modpack spec.">
                  Create modpack
                </button>
                <button
                  className="btn"
                  disabled={busy || !selectedSpec}
                  onClick={() => selectedSpec && openEditor(selectedSpec.id)}
                  title="Open selected modpack in the 3-panel editor."
                >
                  Open editor
                </button>
                <button
                  className="btn"
                  disabled={busy || !selectedSpec}
                  onClick={() => selectedSpec && openApplyWizard(selectedSpec.id)}
                  title="Open apply wizard for selected modpack."
                >
                  Preview + apply
                </button>
                <div ref={homeActionsRef} className="mpmHomeMoreActions" style={{ position: "relative" }}>
                  <button className="btn" onClick={() => setHomeActionsOpen((prev) => !prev)}>
                    More actions
                  </button>
                  {homeActionsOpen ? (
                  <div
                    className="card"
                    style={{
                      position: "absolute",
                      right: 0,
                      top: "calc(100% + 8px)",
                      zIndex: 40,
                      padding: 10,
                      borderRadius: 12,
                      minWidth: 240,
                      boxShadow: "var(--shadowSoft)",
                    }}
                  >
                    <div style={{ display: "grid", gap: 8 }}>
                      <button
                        className="btn"
                        disabled={busy || !selectedSpec}
                        onClick={async () => {
                          setHomeActionsOpen(false);
                          if (!selectedSpec) return;
                          setBusy(true);
                          try {
                            const copy = await duplicateModpackSpec({
                              modpackId: selectedSpec.id,
                              newName: `${selectedSpec.name} copy`,
                            });
                            await refreshSpecs();
                            setSelectedSpecId(copy.id);
                            onNotice("Duplicated modpack spec.");
                          } catch (err: any) {
                            onError(err?.toString?.() ?? String(err));
                          } finally {
                            setBusy(false);
                          }
                        }}
                        title="Duplicate selected modpack."
                      >
                        Duplicate selected
                      </button>
                      <button
                        className="btn"
                        disabled={busy}
                        onClick={async () => {
                          setHomeActionsOpen(false);
                          const picked = await openDialog({
                            multiple: false,
                            filters: [{ name: "JSON", extensions: ["json"] }],
                          });
                          if (!picked || Array.isArray(picked)) return;
                          setBusy(true);
                          try {
                            const out = await importModpackSpecJson({ inputPath: picked });
                            await refreshSpecs();
                            onNotice(`Imported modpack spec data from ${out.path}.`);
                          } catch (err: any) {
                            onError(err?.toString?.() ?? String(err));
                          } finally {
                            setBusy(false);
                          }
                        }}
                        title="Import modpack spec JSON."
                      >
                        Import JSON
                      </button>
                      <button
                        className="btn"
                        disabled={busy || !selectedSpec}
                        onClick={async () => {
                          setHomeActionsOpen(false);
                          if (!selectedSpec) return;
                          const target = await saveDialog({
                            defaultPath: `${selectedSpec.name.replace(/\s+/g, "-").toLowerCase()}.modpack-spec.json`,
                            filters: [{ name: "JSON", extensions: ["json"] }],
                          });
                          if (!target || Array.isArray(target)) return;
                          setBusy(true);
                          try {
                            const out = await exportModpackSpecJson({
                              modpackId: selectedSpec.id,
                              outputPath: target,
                            });
                            onNotice(`Exported spec to ${out.path}.`);
                          } catch (err: any) {
                            onError(err?.toString?.() ?? String(err));
                          } finally {
                            setBusy(false);
                          }
                        }}
                        title="Export selected modpack spec JSON."
                      >
                        Export selected JSON
                      </button>
                      <button
                        className="btn danger"
                        disabled={busy || !selectedSpec}
                        onClick={async () => {
                          setHomeActionsOpen(false);
                          if (!selectedSpec) return;
                          setBusy(true);
                          try {
                            await deleteModpackSpec({ modpackId: selectedSpec.id });
                            setSelectedSpecId(null);
                            await refreshSpecs();
                            onNotice("Deleted modpack spec.");
                          } catch (err: any) {
                            onError(err?.toString?.() ?? String(err));
                          } finally {
                            setBusy(false);
                          }
                        }}
                        title="Delete selected modpack."
                      >
                        Delete selected
                      </button>
                      {isDevMode ? (
                        <button
                          className="btn"
                          disabled={busy}
                          onClick={async () => {
                            setHomeActionsOpen(false);
                            setBusy(true);
                            try {
                              const out = await seedDevModpackData();
                              await refreshSpecs();
                              onNotice(`${out.message} Spec ${out.created_spec_id}, instance ${out.created_instance_id}.`);
                            } catch (err: any) {
                              onError(err?.toString?.() ?? String(err));
                            } finally {
                              setBusy(false);
                            }
                          }}
                          title="Developer only: seed local sample data."
                        >
                          Load dev seed data
                        </button>
                      ) : null}
                    </div>
                  </div>
                  ) : null}
                </div>
              </div>
            </div>

            <div style={{ marginTop: 10 }}>
              <input
                className="input"
                value={homeSearch}
                onChange={(e) => setHomeSearch(e.target.value)}
                placeholder="Search modpacks..."
                title="Search by name, description, or tags."
              />
            </div>
          </div>

          <div className="card mpmShellCard">
            {filteredHomeSpecs.length === 0 ? (
              <div className="muted">
                {specs.length === 0 ? "No modpacks yet. Create one to get started." : "No modpacks match your search."}
              </div>
            ) : (
              <div style={{ display: "grid", gap: 8 }}>
                {filteredHomeSpecs.map((spec) => {
                  const totalEntries = spec.layers.reduce((sum, layer) => sum + layer.entries_delta.add.length, 0);
                  const isSelected = selectedSpecId === spec.id;
                  return (
                    <div
                      key={spec.id}
                      className="card"
                      style={{
                        padding: 12,
                        borderRadius: 12,
                        textAlign: "left",
                        borderColor: isSelected ? "var(--accent-ring)" : undefined,
                        boxShadow: isSelected ? "0 0 0 1px color-mix(in srgb, var(--accent-ring) 55%, transparent)" : undefined,
                        cursor: "pointer",
                      }}
                      onClick={() => setSelectedSpecId(spec.id)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter" || e.key === " ") {
                          e.preventDefault();
                          setSelectedSpecId(spec.id);
                        }
                      }}
                      tabIndex={0}
                      role="button"
                      title="Select this modpack."
                    >
                      <div className="rowBetween">
                        <div style={{ minWidth: 0 }}>
                          <div style={{ fontWeight: 900, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{spec.name}</div>
                          <div className="muted" style={{ marginTop: 2 }}>
                            {spec.layers.length} layers · {totalEntries} entries · {spec.profiles.length} profiles
                          </div>
                          {spec.description ? (
                            <div className="muted" style={{ marginTop: 4, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                              {spec.description}
                            </div>
                          ) : null}
                        </div>
                        <div className="mpmInlineActions">
                          <button
                            className="btn"
                            onClick={(e) => {
                              e.stopPropagation();
                              openEditor(spec.id);
                            }}
                          >
                            Edit
                          </button>
                          <button
                            className="btn"
                            onClick={(e) => {
                              e.stopPropagation();
                              openApplyWizard(spec.id);
                            }}
                          >
                            Apply
                          </button>
                        </div>
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        </>
      ) : null}

      {view === "editor" && editorSpec ? (
        <>
          <div className="card mpmShellCard">
            <div className="mpmEditorTopBar">
              <button className="btn subtle" onClick={() => setView("home")} title="Back to modpack home.">
                Home
              </button>
              <div className="mpmEditorHeaderActions">
                <button className="btn primary" onClick={() => openApplyWizard(editorSpec.id)} title="Preview/resolve/apply in wizard modal.">
                  Preview + apply
                </button>
                <button className="btn subtle" disabled={busy} onClick={saveEditorSpec} title="Save this modpack spec.">
                  Save
                </button>
              </div>
            </div>
            <div className="mpmEditorHeader">
              <div style={{ minWidth: 0 }}>
                <div className="h2">{editorSpec.name || "Untitled modpack"}</div>
                <div className="muted" style={{ marginTop: 4 }}>
                  3-panel editor: layers, unified entries list, and entry inspector.
                </div>
              </div>
            </div>
            <div className="mpmEditorMetaGrid">
              <input
                className="input"
                value={editorSpec.name}
                onChange={(e) => setEditorSpec({ ...editorSpec, name: e.target.value })}
                placeholder="Modpack name"
                title="Name shown in modpack home and apply wizard."
              />
              <input
                className="input"
                value={editorSpec.tags?.join(", ") ?? ""}
                onChange={(e) =>
                  setEditorSpec({
                    ...editorSpec,
                    tags: e.target.value
                      .split(",")
                      .map((tag) => tag.trim())
                      .filter(Boolean),
                  })
                }
                placeholder="Tags (comma separated)"
                title="Optional tags for quick search/grouping."
              />
              <textarea
                className="textarea"
                style={{ gridColumn: "1 / -1" }}
                value={editorSpec.description ?? ""}
                onChange={(e) => setEditorSpec({ ...editorSpec, description: e.target.value })}
                placeholder="Description"
                title="Short summary of this modpack."
              />
            </div>
            <div className="mpmPackSummaryRow">
              <span className="muted">
                {allEntries.length} entries · {entryCountsByType.mods} mods · {entryCountsByType.resourcepacks} resourcepacks · {entryCountsByType.shaderpacks} shaderpacks · {entryCountsByType.datapacks} datapacks
              </span>
              <span className={`chip ${packConflictCount > 0 ? "danger" : "subtle"}`} title="Duplicate entries across layers.">
                Potential conflicts: {packConflictCount}
              </span>
              {conflictSuggestions.length > 0 ? (
                <button className="btn" onClick={() => setConflictWizardOpen(true)}>
                  Conflict wizard
                </button>
              ) : null}
              {plan && plan.modpack_id === editorSpec.id ? (
                <span className={`chip ${plan.failed_mods.length > 0 ? "danger" : "subtle"}`} title="From latest preview resolve.">
                  Last preview failures: {plan.failed_mods.length}
                </span>
              ) : null}
            </div>
          </div>

          <div
            className="mpmEditorGrid"
            style={{
              marginTop: 12,
              display: "grid",
              gap: 12,
              alignItems: "start",
            }}
          >
            <div className="card mpmLayersPanel" style={{ padding: 12, borderRadius: 16 }}>
              <div className="rowBetween mpmLayersHeader">
                <div className="mpmPanelTitle">Layers</div>
                <button
                  className="btn mpmLayersAddBtn"
                  title="Add another layer. Later layers can override earlier ones."
                  onClick={() => {
                    const nextLayer: Layer = {
                      id: `layer_${Date.now()}`,
                      name: `Layer ${editorSpec.layers.length + 1}`,
                      is_frozen: false,
                      entries_delta: { add: [], remove: [], override: [] },
                    };
                    updateEditorSpec((copy) => {
                      copy.layers.push(nextLayer);
                    });
                    setSelectedLayerId(nextLayer.id);
                    setFilterLayerId(nextLayer.id);
                    setTemplateDiff(null);
                  }}
                >
                  Add
                </button>
              </div>
              <div style={{ display: "grid", gap: 8, marginTop: 10 }}>
                {editorSpec.layers.map((layer) => {
                  const active = selectedLayerId === layer.id;
                  const layerMenuOpen = openLayerMenuId === layer.id;
                  const showLayerMeta = Boolean(layer.is_frozen || layer.source);
                  return (
                    <div
                      key={layer.id}
                      className="card"
                      style={{
                        padding: 8,
                        borderRadius: 12,
                        borderColor: active ? "var(--accent-ring)" : undefined,
                      }}
                    >
                      <div className="rowBetween mpmLayerRow">
                        <button
                          className="btn mpmLayerSelectBtn"
                          style={{ flex: 1, justifyContent: "space-between" }}
                          onClick={() => {
                            setSelectedLayerId(layer.id);
                            setFilterLayerId(layer.id);
                            setEntryFilter("by_layer");
                            setOpenLayerMenuId(null);
                            setTemplateDiff(null);
                          }}
                          title={layerHelpText(layer)}
                        >
                          <span>{layer.name}</span>
                          <span className="chip subtle">{layer.entries_delta.add.length}</span>
                        </button>
                        <button
                          className="btn mpmLayerMoreBtn"
                          onClick={() => setOpenLayerMenuId((prev) => (prev === layer.id ? null : layer.id))}
                          title="Layer actions"
                        >
                          ⋯
                        </button>
                      </div>
                      {layerMenuOpen ? (
                        <div className="card" style={{ marginTop: 6, padding: 8, borderRadius: 10 }}>
                          <div style={{ display: "grid", gap: 6 }}>
                            <button
                              className="btn"
                              onClick={() => {
                                updateEditorSpec((copy) => {
                                  const idx = copy.layers.findIndex((item) => item.id === layer.id);
                                  if (idx < 0) return;
                                  copy.layers[idx].is_frozen = !copy.layers[idx].is_frozen;
                                });
                                setOpenLayerMenuId(null);
                              }}
                            >
                              {layer.is_frozen ? "Unfreeze layer" : "Freeze layer"}
                            </button>
                            <button
                              className="btn"
                              onClick={() => {
                                setEntryFilter("by_layer");
                                setFilterLayerId(layer.id);
                                setOpenLayerMenuId(null);
                              }}
                            >
                              Filter list to layer
                            </button>
                          </div>
                        </div>
                      ) : null}
                      {showLayerMeta ? (
                        <div className="row mpmLayerMetaRow" style={{ marginTop: 6, gap: 6, flexWrap: "wrap" }}>
                          {layer.is_frozen ? <span className="chip subtle">Frozen</span> : null}
                          {layer.source ? <span className="chip subtle">Source: {layer.source.kind}</span> : null}
                        </div>
                      ) : null}
                    </div>
                  );
                })}
              </div>

              <details className="card" style={{ marginTop: 10, padding: 10, borderRadius: 12 }}>
                <summary className="mpmLayerToolsSummary" style={{ cursor: "pointer", fontWeight: 800 }}>Layer tools, templates, and diffs</summary>
                <div style={{ marginTop: 10, display: "grid", gap: 8 }}>
                  <div className="muted" title="Import a provider modpack as a new stable layer snapshot.">
                    Import provider template
                  </div>
                  <input
                    className="input"
                    value={providerImport.layerName}
                    onChange={(e) => setProviderImport((prev) => ({ ...prev, layerName: e.target.value }))}
                    placeholder="Imported layer name"
                  />
                  <div className="row" style={{ gap: 8 }}>
                    <select
                      className="input"
                      value={providerImport.source}
                      onChange={(e) => setProviderImport((prev) => ({ ...prev, source: e.target.value }))}
                    >
                      <option value="modrinth">Modrinth</option>
                      <option value="curseforge">CurseForge</option>
                    </select>
                    <input
                      className="input"
                      value={providerImport.projectId}
                      onChange={(e) => setProviderImport((prev) => ({ ...prev, projectId: e.target.value }))}
                      placeholder="Template project id"
                    />
                  </div>
                  <button
                    className="btn"
                    disabled={busy || !providerImport.projectId.trim() || !editorSpec}
                    onClick={async () => {
                      if (!editorSpec) return;
                      setBusy(true);
                      try {
                        const next = await importModpackLayerFromProvider({
                          modpackId: editorSpec.id,
                          layerName: providerImport.layerName.trim() || "Imported Template",
                          source: providerImport.source,
                          projectId: providerImport.projectId.trim(),
                        });
                        setEditorSpec(cloneSpec(next));
                        onNotice("Imported provider template layer.");
                      } catch (err: any) {
                        onError(err?.toString?.() ?? String(err));
                      } finally {
                        setBusy(false);
                      }
                    }}
                  >
                    Import provider layer
                  </button>

                  <div className="muted" style={{ marginTop: 4 }}>Import from existing modpack</div>
                  <select
                    className="input"
                    value={specImport.sourceModpackId}
                    onChange={(e) => setSpecImport((prev) => ({ ...prev, sourceModpackId: e.target.value }))}
                  >
                    <option value="">Select source modpack...</option>
                    {specs.filter((spec) => spec.id !== editorSpec.id).map((spec) => (
                      <option key={spec.id} value={spec.id}>{spec.name}</option>
                    ))}
                  </select>
                  <input
                    className="input"
                    value={specImport.layerName}
                    onChange={(e) => setSpecImport((prev) => ({ ...prev, layerName: e.target.value }))}
                    placeholder="Imported layer name"
                  />
                  <button
                    className="btn"
                    disabled={busy || !specImport.sourceModpackId || !editorSpec}
                    onClick={async () => {
                      if (!editorSpec) return;
                      setBusy(true);
                      try {
                        const next = await importModpackLayerFromSpec({
                          targetModpackId: editorSpec.id,
                          sourceModpackId: specImport.sourceModpackId,
                          layerName: specImport.layerName.trim() || "Imported Modpack Layer",
                        });
                        setEditorSpec(cloneSpec(next));
                        onNotice("Imported layer from modpack.");
                      } catch (err: any) {
                        onError(err?.toString?.() ?? String(err));
                      } finally {
                        setBusy(false);
                      }
                    }}
                  >
                    Import layer from modpack
                  </button>

                  <div className="muted" style={{ marginTop: 4 }}>
                    Diff/conflicts entry point
                  </div>
                  <div className="row" style={{ gap: 8, flexWrap: "wrap" }}>
                    <button
                      className="btn"
                      disabled={busy || !selectedLayer || !templateLayerSelected || !editorSpec}
                      onClick={async () => {
                        if (!editorSpec || !selectedLayer) return;
                        setBusy(true);
                        try {
                          const diff = await previewTemplateLayerUpdate({
                            modpackId: editorSpec.id,
                            layerId: selectedLayer.id,
                          });
                          setTemplateDiff(diff);
                        } catch (err: any) {
                          onError(err?.toString?.() ?? String(err));
                        } finally {
                          setBusy(false);
                        }
                      }}
                    >
                      Preview template update
                    </button>
                    <button
                      className="btn"
                      disabled={busy || !selectedLayer || !templateLayerSelected || !templateDiff || !editorSpec}
                      onClick={async () => {
                        if (!editorSpec || !selectedLayer) return;
                        setBusy(true);
                        try {
                          const next = await applyTemplateLayerUpdate({
                            modpackId: editorSpec.id,
                            layerId: selectedLayer.id,
                          });
                          setEditorSpec(cloneSpec(next));
                          setTemplateDiff(null);
                          onNotice("Applied template update.");
                        } catch (err: any) {
                          onError(err?.toString?.() ?? String(err));
                        } finally {
                          setBusy(false);
                        }
                      }}
                    >
                      Apply template update
                    </button>
                  </div>

                  {templateDiff ? (
                    <div className="card" style={{ padding: 8, borderRadius: 10 }}>
                      <div className="muted">
                        Diff: {templateDiff.added.length} added · {templateDiff.removed.length} removed · {templateDiff.overridden.length} overridden
                      </div>
                    </div>
                  ) : null}
                  <div className="muted">
                    Current conflicts: {packConflictCount}
                    {plan ? ` · Preview conflicts: ${plan.conflicts.length}` : ""}
                  </div>
                </div>
              </details>
            </div>

            <div className="card mpmEntriesPanel" style={{ padding: 12, borderRadius: 16 }}>
              <div className="mpmEntriesTop">
                <div className="rowBetween mpmEntriesHead">
                  <div className="h3">Entries</div>
                  <div className="mpmInlineActions mpmEntriesPrimaryActions">
                    <button className="btn" onClick={openAddModal} title="Add content directly by id/slug.">
                      Add in-place
                    </button>
                    <button className="btn" onClick={openDiscoverForSelectedLayer} title="Search in Discover and add directly to this modpack/layer.">
                      Open in Discover
                    </button>
                  </div>
                </div>
                <details className="mpmEntryToolsFold">
                  <summary>Local JAR tools</summary>
                  <div className="mpmEntryToolsRow">
                    <button
                      className="btn"
                      onClick={() => void addLocalJarsFromComputer()}
                      disabled={importLocalJarsBusy}
                      title="Add one or more local .jar files into the selected layer."
                    >
                      {importLocalJarsBusy ? "Adding..." : "Add mod(s) from computer"}
                    </button>
                    <button
                      className="btn"
                      onClick={() => void identifyLocalJarsInCreator("all")}
                      disabled={identifyLocalJarsBusy}
                      title="Try to match local entries to Modrinth/CurseForge metadata."
                    >
                      {identifyLocalJarsBusy ? "Identifying..." : "Identify local JARs"}
                    </button>
                  </div>
                </details>
              </div>
              <div style={{ marginTop: 10, display: "grid", gridTemplateColumns: "1fr auto", gap: 8 }}>
                <input
                  className="input"
                  value={entrySearch}
                  onChange={(e) => setEntrySearch(e.target.value)}
                  placeholder="Search entries by name, id, source, or layer..."
                />
                <select
                  className="input"
                  value={entryFilter}
                  onChange={(e) => setEntryFilter((e.target.value as EntryFilter) ?? "all")}
                  title="Quick filters for entry list."
                >
                  <option value="all">All</option>
                  <option value="mods">Mods</option>
                  <option value="optional">Optional</option>
                  <option value="pinned">Pinned</option>
                  <option value="failing">Failing (from preview)</option>
                  <option value="by_layer">By layer</option>
                </select>
              </div>

              {entryFilter === "by_layer" ? (
                <div style={{ marginTop: 8 }}>
                  <select
                    className="input"
                    value={filterLayerId || selectedLayerId || ""}
                    onChange={(e) => setFilterLayerId(e.target.value)}
                    title="Layer scope when using By layer filter."
                  >
                    {editorSpec.layers.map((layer) => (
                      <option key={layer.id} value={layer.id}>
                        {layer.name}
                      </option>
                    ))}
                  </select>
                </div>
              ) : null}

              <div className="mpmEntryStatsRow">
                <span className="muted">
                  Showing {filteredEntries.length} of {allEntries.length}
                </span>
                {entryFilter === "failing" && !plan ? (
                  <span className="muted" title="Run preview resolve in apply wizard to populate this filter.">
                    No preview data yet
                  </span>
                ) : null}
              </div>
              <div className="muted" style={{ marginTop: 6 }}>
                Click an entry to edit it in Inspector.
              </div>

              <div style={{ marginTop: 10, display: "grid", gap: 8 }}>
                {filteredEntries.length === 0 ? (
                  <div className="card" style={{ padding: 14, borderRadius: 12 }}>
                    <div style={{ fontWeight: 900 }}>No entries in this view.</div>
                    <div className="muted" style={{ marginTop: 4 }}>
                      Add content in-place or open Discover to search with full filters and add directly to this modpack.
                    </div>
                    <div className="row" style={{ marginTop: 8, gap: 8 }}>
                      <button className="btn" onClick={openAddModal}>Add in-place</button>
                      <button className="btn" onClick={openDiscoverForSelectedLayer}>Open in Discover</button>
                    </div>
                  </div>
                ) : (
                  filteredEntries.map((row) => {
                    const active = selectedEntryKey === row.key;
                    const isFailing = failureKeySet.has(entryIdentity(row.entry));
                    const layerFrozen = editorSpec.layers.find((layer) => layer.id === row.layerId)?.is_frozen;
                    const metaBits = [
                      row.entry.provider,
                      row.entry.content_type,
                      row.layerName,
                      !row.entry.required ? "Optional" : null,
                      row.entry.pin ? "Pinned" : null,
                    ]
                      .filter(Boolean)
                      .join(" · ");
                    return (
                      <div
                        key={row.key}
                        className="card mpmEntryCard"
                        style={{
                          padding: 10,
                          borderRadius: 12,
                          textAlign: "left",
                          borderColor: active ? "var(--accent-ring)" : undefined,
                          cursor: "pointer",
                        }}
                        onClick={() => setSelectedEntryKey(row.key)}
                        onKeyDown={(e) => {
                          if (e.key === "Enter" || e.key === " ") {
                            e.preventDefault();
                            setSelectedEntryKey(row.key);
                          }
                        }}
                        tabIndex={0}
                        role="button"
                        title="Select entry to edit in inspector."
                      >
                        <div className="rowBetween mpmEntryRow">
                          <div className="mpmEntryMain">
                            <div className="mpmEntryTitle">
                              {entryDisplayName(row.entry)}
                            </div>
                            <div className="muted mpmEntryProjectId">
                              {row.entry.project_id}
                            </div>
                            <div className="mpmEntryMetaText">{metaBits}</div>
                          </div>
                          <div className="mpmEntryActions">
                            {row.duplicateCount > 1 ? <span className="chip danger">Conflict</span> : null}
                            {isFailing ? <span className="chip danger">Failing</span> : null}
                            <button
                              className="btn subtle mpmEntryRemoveBtn"
                              disabled={Boolean(layerFrozen)}
                              onClick={(e) => {
                                e.stopPropagation();
                                removeEntry(row.layerId, row.entryIndex);
                                onNotice(`Removed ${entryDisplayName(row.entry)} from ${row.layerName}.`);
                              }}
                              title={layerFrozen ? "Layer is frozen. Unfreeze to remove." : "Remove this entry from the modpack."}
                            >
                              Remove
                            </button>
                          </div>
                        </div>
                      </div>
                    );
                  })
                )}
              </div>
            </div>

            <div className="card mpmInspectorPanel" style={{ padding: 12, borderRadius: 16 }}>
              <div className="rowBetween">
                <div style={{ fontWeight: 900 }}>Inspector</div>
                {selectedEntryRow ? (
                  <button
                    className="btn subtle"
                    disabled={editorSpec.layers.find((layer) => layer.id === selectedEntryRow.layerId)?.is_frozen}
                    onClick={() => removeEntry(selectedEntryRow.layerId, selectedEntryRow.entryIndex)}
                    title="Remove this entry from its layer."
                  >
                    Remove
                  </button>
                ) : null}
              </div>

              {!selectedEntryRow ? (
                <div className="muted" style={{ marginTop: 8 }}>
                  Select an entry from the center list to edit requirement, defaults, and advanced policies.
                </div>
              ) : (
                <div style={{ marginTop: 8, display: "grid", gap: 8 }}>
                  <div className="mpmInspectorMeta">
                    {selectedEntryRow.layerName} · {selectedEntryRow.entry.provider} · {selectedEntryRow.entry.content_type}
                    {selectedEntryRow.duplicateCount > 1 ? <span className="chip danger">Conflict candidate</span> : null}
                  </div>

                  <label style={{ display: "grid", gap: 6 }}>
                    <span className="muted">Display name / notes</span>
                    <input
                      className="input"
                      value={selectedEntryRow.entry.notes ?? ""}
                      onChange={(e) =>
                        updateEntry(selectedEntryRow.layerId, selectedEntryRow.entryIndex, (prev) => ({
                          ...prev,
                          notes: e.target.value,
                        }))
                      }
                      placeholder="Friendly name or notes"
                      title="Name shown in entry list. Keep this human-friendly."
                    />
                  </label>

                  <label style={{ display: "grid", gap: 6 }}>
                    <span className="muted">Project id / slug</span>
                    <input
                      className="input"
                      value={selectedEntryRow.entry.project_id}
                      onChange={(e) =>
                        updateEntry(selectedEntryRow.layerId, selectedEntryRow.entryIndex, (prev) => ({
                          ...prev,
                          project_id: e.target.value,
                        }))
                      }
                    />
                  </label>

                  <div style={{ display: "grid", gap: 8, gridTemplateColumns: "1fr 1fr" }}>
                    <label style={{ display: "grid", gap: 6 }}>
                      <span className="muted">Requirement</span>
                      <select
                        className="input"
                        value={selectedEntryRow.entry.required ? "required" : "optional"}
                        onChange={(e) =>
                          updateEntry(selectedEntryRow.layerId, selectedEntryRow.entryIndex, (prev) => ({
                            ...prev,
                            required: e.target.value === "required",
                            optional: e.target.value !== "required",
                          }))
                        }
                        title="Required failures block apply by default."
                      >
                        <option value="required">Required</option>
                        <option value="optional">Optional</option>
                      </select>
                    </label>

                    <label style={{ display: "grid", gap: 6 }}>
                      <span className="muted">Default state</span>
                      <select
                        className="input"
                        value={selectedEntryRow.entry.disabled_by_default ? "disabled" : "enabled"}
                        onChange={(e) =>
                          updateEntry(selectedEntryRow.layerId, selectedEntryRow.entryIndex, (prev) => ({
                            ...prev,
                            disabled_by_default: e.target.value === "disabled",
                          }))
                        }
                      >
                        <option value="enabled">Enabled</option>
                        <option value="disabled">Disabled</option>
                      </select>
                    </label>
                  </div>

                  <details>
                    <summary style={{ cursor: "pointer", fontWeight: 800 }}>Advanced options</summary>
                    <div style={{ marginTop: 8, display: "grid", gap: 8 }}>
                      <div style={{ display: "grid", gap: 8, gridTemplateColumns: "1fr 1fr" }}>
                        <label style={{ display: "grid", gap: 6 }}>
                          <span className="muted">Provider</span>
                          <select
                            className="input"
                            value={selectedEntryRow.entry.provider}
                            onChange={(e) =>
                              updateEntry(selectedEntryRow.layerId, selectedEntryRow.entryIndex, (prev) => ({
                                ...prev,
                                provider: e.target.value,
                              }))
                            }
                          >
                            <option value="modrinth">Modrinth</option>
                            <option value="curseforge">CurseForge</option>
                          </select>
                        </label>

                        <label style={{ display: "grid", gap: 6 }}>
                          <span className="muted">Type</span>
                          <select
                            className="input"
                            value={selectedEntryRow.entry.content_type}
                            onChange={(e) =>
                              updateEntry(selectedEntryRow.layerId, selectedEntryRow.entryIndex, (prev) => ({
                                ...prev,
                                content_type: e.target.value,
                              }))
                            }
                          >
                            <option value="mods">Mods</option>
                            <option value="resourcepacks">Resourcepacks</option>
                            <option value="shaderpacks">Shaderpacks</option>
                            <option value="datapacks">Datapacks</option>
                          </select>
                        </label>
                      </div>

                      <div style={{ display: "grid", gap: 8, gridTemplateColumns: "1fr 1fr" }}>
                        <label style={{ display: "grid", gap: 6 }}>
                          <span className="muted">Channel policy</span>
                          <select
                            className="input"
                            value={selectedEntryRow.entry.channel_policy ?? "stable"}
                            onChange={(e) =>
                              updateEntry(selectedEntryRow.layerId, selectedEntryRow.entryIndex, (prev) => ({
                                ...prev,
                                channel_policy: e.target.value,
                              }))
                            }
                          >
                            <option value="stable">Stable only</option>
                            <option value="beta">Allow beta</option>
                            <option value="alpha">Allow alpha</option>
                            <option value="inherit">Inherit</option>
                          </select>
                        </label>

                        <label style={{ display: "grid", gap: 6 }}>
                          <span className="muted">Fallback policy</span>
                          <select
                            className="input"
                            value={selectedEntryRow.entry.fallback_policy ?? "inherit"}
                            onChange={(e) =>
                              updateEntry(selectedEntryRow.layerId, selectedEntryRow.entryIndex, (prev) => ({
                                ...prev,
                                fallback_policy: e.target.value,
                              }))
                            }
                          >
                            <option value="inherit">Inherit global</option>
                            <option value="strict">Strict</option>
                            <option value="smart">Smart</option>
                            <option value="loose">Loose</option>
                          </select>
                        </label>
                      </div>

                      <label style={{ display: "grid", gap: 6 }}>
                        <span className="muted">Pin (version/file id)</span>
                        <input
                          className="input"
                          value={selectedEntryRow.entry.pin ?? ""}
                          onChange={(e) =>
                            updateEntry(selectedEntryRow.layerId, selectedEntryRow.entryIndex, (prev) => ({
                              ...prev,
                              pin: e.target.value.trim() || null,
                            }))
                          }
                        />
                      </label>

                      <label style={{ display: "grid", gap: 6 }}>
                        <span className="muted">Slug (optional)</span>
                        <input
                          className="input"
                          value={selectedEntryRow.entry.slug ?? ""}
                          onChange={(e) =>
                            updateEntry(selectedEntryRow.layerId, selectedEntryRow.entryIndex, (prev) => ({
                              ...prev,
                              slug: e.target.value.trim() || null,
                            }))
                          }
                        />
                      </label>

                      <label style={{ display: "grid", gap: 6 }}>
                        <span className="muted">Replacement group (optional)</span>
                        <input
                          className="input"
                          value={selectedEntryRow.entry.replacement_group ?? ""}
                          onChange={(e) =>
                            updateEntry(selectedEntryRow.layerId, selectedEntryRow.entryIndex, (prev) => ({
                              ...prev,
                              replacement_group: e.target.value.trim() || null,
                            }))
                          }
                        />
                      </label>
                    </div>
                  </details>
                </div>
              )}
            </div>
          </div>
        </>
      ) : null}

      {conflictWizardOpen ? (
        <div className="modalOverlay" onMouseDown={() => setConflictWizardOpen(false)}>
          <div className="modal wide" onMouseDown={(e) => e.stopPropagation()}>
            <div className="modalHeader">
              <div className="modalTitle">Creator conflict wizard</div>
              <button className="iconBtn" onClick={() => setConflictWizardOpen(false)} aria-label="Close">
                ✕
              </button>
            </div>
            <div className="modalBody">
              <div className="muted">
                Deterministic suggestions only. Review patch previews, then apply to the draft spec.
              </div>
              {conflictSuggestions.length === 0 ? (
                <div className="card" style={{ marginTop: 10, padding: 12, borderRadius: 12 }}>
                  <div className="muted">No conflict suggestions available right now.</div>
                </div>
              ) : (
                <div style={{ marginTop: 10, display: "grid", gap: 8 }}>
                  {conflictSuggestions.map((suggestion) => {
                    const checked = selectedConflictSuggestionIds.includes(suggestion.id);
                    return (
                      <div key={suggestion.id} className="card" style={{ padding: 10, borderRadius: 12 }}>
                        <div className="rowBetween" style={{ alignItems: "flex-start", gap: 10 }}>
                          <label className="row" style={{ marginTop: 0, gap: 8, alignItems: "flex-start", flex: 1 }}>
                            <input
                              type="checkbox"
                              checked={checked}
                              onChange={(e) =>
                                setSelectedConflictSuggestionIds((prev) => {
                                  if (e.target.checked) {
                                    if (prev.includes(suggestion.id)) return prev;
                                    return [...prev, suggestion.id];
                                  }
                                  return prev.filter((id) => id !== suggestion.id);
                                })
                              }
                            />
                            <div>
                              <div style={{ fontWeight: 900 }}>{suggestion.title}</div>
                              <div className="muted" style={{ marginTop: 4 }}>{suggestion.detail}</div>
                            </div>
                          </label>
                          <div className="row" style={{ marginTop: 0, gap: 6 }}>
                            <span className="chip subtle">{suggestion.conflict_code}</span>
                            <span className={`chip ${suggestion.risk === "medium" || suggestion.risk === "high" ? "danger" : "subtle"}`}>
                              {suggestion.risk} risk
                            </span>
                          </div>
                        </div>
                        <pre style={{ marginTop: 8, whiteSpace: "pre-wrap", fontSize: 12 }}>{suggestion.patch_preview}</pre>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
            <div className="footerBar">
              <button className="btn" onClick={() => setConflictWizardOpen(false)}>
                Cancel
              </button>
              <button className="btn primary" onClick={applySelectedConflictSuggestions}>
                Apply selected suggestions
              </button>
            </div>
          </div>
        </div>
      ) : null}

      {addModalOpen ? (
        <div className="modalOverlay" onMouseDown={() => setAddModalOpen(false)}>
          <div className="modal" onMouseDown={(e) => e.stopPropagation()}>
            <div className="modalHeader">
              <div className="modalTitle">Add content in-place</div>
              <button className="iconBtn" onClick={() => setAddModalOpen(false)} aria-label="Close">
                ✕
              </button>
            </div>
            <div className="modalBody">
              <div className="muted">
                Quick add by project id/slug. For full search/filtering, use <strong>Open in Discover</strong>.
              </div>
              <div style={{ marginTop: 10, display: "grid", gap: 8 }}>
                <div style={{ display: "grid", gap: 8, gridTemplateColumns: "1fr 1fr" }}>
                  <select
                    className="input"
                    value={entryDraft.provider}
                    onChange={(e) => setEntryDraft((prev) => ({ ...prev, provider: e.target.value }))}
                  >
                    <option value="modrinth">Modrinth</option>
                    <option value="curseforge">CurseForge</option>
                  </select>
                  <select
                    className="input"
                    value={entryDraft.content_type}
                    onChange={(e) => setEntryDraft((prev) => ({ ...prev, content_type: e.target.value }))}
                  >
                    <option value="mods">Mods</option>
                    <option value="resourcepacks">Resourcepacks</option>
                    <option value="shaderpacks">Shaderpacks</option>
                    <option value="datapacks">Datapacks</option>
                  </select>
                </div>
                <input
                  className="input"
                  value={entryDraft.project_id}
                  onChange={(e) => setEntryDraft((prev) => ({ ...prev, project_id: e.target.value }))}
                  placeholder="Project id or slug"
                />

                <div style={{ display: "grid", gap: 8, gridTemplateColumns: "1fr 1fr" }}>
                  <label style={{ display: "grid", gap: 6 }}>
                    <span className="muted">Requirement</span>
                    <select
                      className="input"
                      value={entryDraft.required ? "required" : "optional"}
                      onChange={(e) =>
                        setEntryDraft((prev) => ({
                          ...prev,
                          required: e.target.value === "required",
                          optional: e.target.value !== "required",
                        }))
                      }
                    >
                      <option value="required">Required</option>
                      <option value="optional">Optional</option>
                    </select>
                  </label>
                  <label style={{ display: "grid", gap: 6 }}>
                    <span className="muted">Default state</span>
                    <select
                      className="input"
                      value={entryDraft.disabled_by_default ? "disabled" : "enabled"}
                      onChange={(e) =>
                        setEntryDraft((prev) => ({ ...prev, disabled_by_default: e.target.value === "disabled" }))
                      }
                    >
                      <option value="enabled">Enabled</option>
                      <option value="disabled">Disabled</option>
                    </select>
                  </label>
                </div>

                <details>
                  <summary style={{ cursor: "pointer", fontWeight: 800 }}>Advanced entry options</summary>
                  <div style={{ marginTop: 8, display: "grid", gap: 8 }}>
                    <div style={{ display: "grid", gap: 8, gridTemplateColumns: "1fr 1fr" }}>
                      <select
                        className="input"
                        value={entryDraft.channel_policy ?? "stable"}
                        onChange={(e) => setEntryDraft((prev) => ({ ...prev, channel_policy: e.target.value }))}
                      >
                        <option value="stable">Stable only</option>
                        <option value="beta">Allow beta</option>
                        <option value="alpha">Allow alpha</option>
                        <option value="inherit">Inherit</option>
                      </select>
                      <select
                        className="input"
                        value={entryDraft.fallback_policy ?? "inherit"}
                        onChange={(e) => setEntryDraft((prev) => ({ ...prev, fallback_policy: e.target.value }))}
                      >
                        <option value="inherit">Inherit global</option>
                        <option value="strict">Strict</option>
                        <option value="smart">Smart</option>
                        <option value="loose">Loose</option>
                      </select>
                    </div>
                    <input
                      className="input"
                      value={entryDraft.pin ?? ""}
                      onChange={(e) => setEntryDraft((prev) => ({ ...prev, pin: e.target.value }))}
                      placeholder="Pinned version/file id (optional)"
                    />
                    <input
                      className="input"
                      value={entryDraft.notes ?? ""}
                      onChange={(e) => setEntryDraft((prev) => ({ ...prev, notes: e.target.value }))}
                      placeholder="Display name / notes (optional)"
                    />
                  </div>
                </details>
              </div>
            </div>
            <div className="footerBar">
              <button className="btn" onClick={openDiscoverForSelectedLayer}>
                Open in Discover
              </button>
              <button className="btn" onClick={() => setAddModalOpen(false)}>
                Cancel
              </button>
              <button className="btn primary" onClick={addDraftToSelectedLayer}>
                Add to layer
              </button>
            </div>
          </div>
        </div>
      ) : null}

      {applyWizardOpen ? (
        <div className="modalOverlay" onMouseDown={() => setApplyWizardOpen(false)}>
          <div className="modal wide" onMouseDown={(e) => e.stopPropagation()}>
            <div className="modalHeader">
              <div className="modalTitle">Apply wizard</div>
              <button className="iconBtn" onClick={() => setApplyWizardOpen(false)} aria-label="Close">
                ✕
              </button>
            </div>
            <div className="modalBody">
              <div className="muted">
                Preview comes first. Resolve against your target instance, inspect failures/conflicts, then apply.
              </div>

              <div style={{ marginTop: 10, display: "grid", gap: 8, gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))" }}>
                <label style={{ display: "grid", gap: 6 }}>
                  <span className="muted">Modpack</span>
                  <select
                    className="input"
                    value={applySpecId}
                    onChange={(e) => {
                      const nextId = e.target.value;
                      setApplySpecId(nextId);
                      const next = specs.find((spec) => spec.id === nextId);
                      setApplyProfileId(next?.profiles[0]?.id ?? "recommended");
                      setApplySettings(next?.settings ?? defaultSettings());
                      setPlan(null);
                    }}
                  >
                    {specs.map((spec) => (
                      <option key={spec.id} value={spec.id}>{spec.name}</option>
                    ))}
                  </select>
                </label>
                <label style={{ display: "grid", gap: 6 }}>
                  <span className="muted">Target instance</span>
                  <select
                    className="input"
                    value={applyInstanceId}
                    onChange={(e) => {
                      setApplyInstanceId(e.target.value);
                      onSelectInstance(e.target.value);
                    }}
                  >
                    <option value="">Select instance...</option>
                    {instances.map((inst) => (
                      <option key={inst.id} value={inst.id}>{inst.name}</option>
                    ))}
                  </select>
                </label>
                <label style={{ display: "grid", gap: 6 }}>
                  <span className="muted">Profile</span>
                  <select
                    className="input"
                    value={applyProfileId}
                    onChange={(e) => setApplyProfileId(e.target.value)}
                  >
                    {(selectedApplySpec?.profiles ?? []).map((profile) => (
                      <option key={profile.id} value={profile.id}>{profile.name}</option>
                    ))}
                  </select>
                </label>
                <label style={{ display: "grid", gap: 6 }}>
                  <span className="muted">Apply mode</span>
                  <select
                    className="input"
                    value={applyLinkMode}
                    onChange={(e) => setApplyLinkMode((e.target.value as "linked" | "unlinked") ?? "linked")}
                  >
                    <option value="linked">Linked mode</option>
                    <option value="unlinked">Unlinked mode</option>
                  </select>
                </label>
              </div>

              <details style={{ marginTop: 10 }} open={applySettingsOpen}>
                <summary
                  style={{ cursor: "pointer", fontWeight: 800 }}
                  onClick={() => setApplySettingsOpen((prev) => !prev)}
                >
                  Optional resolution settings
                </summary>
                <div style={{ marginTop: 8, display: "grid", gap: 8, gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))" }}>
                  <label style={{ display: "grid", gap: 6 }}>
                    <span className="muted">Fallback mode</span>
                    <select
                      className="input"
                      value={applySettings.global_fallback_mode}
                      onChange={(e) =>
                        setApplySettings((prev) => ({ ...prev, global_fallback_mode: e.target.value }))
                      }
                    >
                      <option value="strict">Strict</option>
                      <option value="smart">Smart</option>
                      <option value="loose">Loose</option>
                    </select>
                  </label>
                  <label style={{ display: "grid", gap: 6 }}>
                    <span className="muted">Channel allowance</span>
                    <select
                      className="input"
                      value={applySettings.channel_allowance}
                      onChange={(e) =>
                        setApplySettings((prev) => ({ ...prev, channel_allowance: e.target.value }))
                      }
                    >
                      <option value="stable">Stable only</option>
                      <option value="beta">Allow beta</option>
                      <option value="alpha">Allow alpha</option>
                    </select>
                  </label>
                  <label style={{ display: "grid", gap: 6 }}>
                    <span className="muted">Dependencies</span>
                    <select
                      className="input"
                      value={applySettings.dependency_mode}
                      onChange={(e) =>
                        setApplySettings((prev) => ({ ...prev, dependency_mode: e.target.value }))
                      }
                    >
                      <option value="detect_only">Detect only</option>
                      <option value="auto_add">Auto add</option>
                    </select>
                  </label>
                  <label style={{ display: "grid", gap: 6 }}>
                    <span className="muted">Max fallback distance</span>
                    <input
                      className="input"
                      type="number"
                      min={0}
                      max={20}
                      value={applySettings.max_fallback_distance}
                      onChange={(e) =>
                        setApplySettings((prev) => ({
                          ...prev,
                          max_fallback_distance: Math.max(0, Number(e.target.value || 0)),
                        }))
                      }
                    />
                  </label>
                  <label className="row" style={{ gap: 8 }}>
                    <input
                      type="checkbox"
                      checked={applySettings.allow_cross_minor}
                      onChange={(e) =>
                        setApplySettings((prev) => ({ ...prev, allow_cross_minor: e.target.checked }))
                      }
                    />
                    <span className="muted">Allow cross minor fallback</span>
                  </label>
                  <label className="row" style={{ gap: 8 }}>
                    <input
                      type="checkbox"
                      checked={applySettings.allow_cross_major}
                      onChange={(e) =>
                        setApplySettings((prev) => ({ ...prev, allow_cross_major: e.target.checked }))
                      }
                    />
                    <span className="muted">Allow cross major fallback</span>
                  </label>
                  <label className="row" style={{ gap: 8 }}>
                    <input
                      type="checkbox"
                      checked={applySettings.prefer_stable}
                      onChange={(e) =>
                        setApplySettings((prev) => ({ ...prev, prefer_stable: e.target.checked }))
                      }
                    />
                    <span className="muted">Prefer stable releases</span>
                  </label>
                  <label className="row" style={{ gap: 8 }}>
                    <input
                      type="checkbox"
                      checked={Boolean(applySettings.partial_apply_unsafe)}
                      onChange={(e) =>
                        setApplySettings((prev) => ({ ...prev, partial_apply_unsafe: e.target.checked }))
                      }
                    />
                    <span className="muted">Partial apply (UNSAFE)</span>
                  </label>
                </div>
              </details>

              <div className="row" style={{ marginTop: 10, gap: 8 }}>
                <button
                  className="btn"
                  disabled={busy || !applySpecId || !applyInstanceId}
                  onClick={async () => {
                    if (!applySpecId || !applyInstanceId) {
                      onError("Select modpack and target instance first.");
                      return;
                    }
                    setBusy(true);
                    try {
                      const out = await resolveModpackForInstance({
                        modpackId: applySpecId,
                        instanceId: applyInstanceId,
                        profileId: applyProfileId || null,
                        settings: applySettings,
                      });
                      setPlan(out);
                      onNotice(
                        `Preview complete: ${out.resolved_mods.length} resolved, ${out.failed_mods.length} failed, ${out.conflicts.length} conflicts.`
                      );
                    } catch (err: any) {
                      setPlan(null);
                      onError(err?.toString?.() ?? String(err));
                    } finally {
                      setBusy(false);
                    }
                  }}
                >
                  Preview resolve
                </button>
                <button
                  className="btn primary"
                  disabled={busy || !plan}
                  onClick={async () => {
                    if (!plan) return;
                    setBusy(true);
                    try {
                      const out = await applyModpackPlan({
                        planId: plan.id,
                        linkMode: applyLinkMode,
                        partialApplyUnsafe: Boolean(applySettings.partial_apply_unsafe),
                      });
                      onNotice(
                        `${out.message} Applied ${out.applied_entries}, failed ${out.failed_entries}, skipped ${out.skipped_entries}.`
                      );
                    } catch (err: any) {
                      onError(err?.toString?.() ?? String(err));
                    } finally {
                      setBusy(false);
                    }
                  }}
                >
                  Apply plan
                </button>
                {plan ? (
                  <span className={`chip ${plan.failed_mods.length > 0 || plan.conflicts.length > 0 ? "danger" : "subtle"}`}>
                    Confidence: {plan.confidence_label}
                  </span>
                ) : null}
              </div>

              {plan ? (
                <div className="card" style={{ marginTop: 12, padding: 12, borderRadius: 12 }}>
                  <div className="row" style={{ gap: 8, flexWrap: "wrap" }}>
                    <span className="chip subtle">Resolved: {plan.resolved_mods.length}</span>
                    <span className={`chip ${plan.failed_mods.length > 0 ? "danger" : "subtle"}`}>Failed: {plan.failed_mods.length}</span>
                    <span className={`chip ${plan.conflicts.length > 0 ? "danger" : "subtle"}`}>Conflicts: {plan.conflicts.length}</span>
                    <span className="chip subtle">Warnings: {plan.warnings.length}</span>
                  </div>

                  {plan.failed_mods.length > 0 ? (
                    <div style={{ marginTop: 10, display: "grid", gap: 6 }}>
                      {plan.failed_mods.slice(0, 10).map((failure) => (
                        <div key={`${failure.source}:${failure.project_id}`} className="errorBox" style={{ marginTop: 0 }}>
                          <strong>{failure.name}</strong>: {failure.reason_text}
                          <div className="muted" style={{ marginTop: 4 }}>{failure.actionable_hint}</div>
                        </div>
                      ))}
                    </div>
                  ) : null}

                  {plan.conflicts.length > 0 ? (
                    <div style={{ marginTop: 10, display: "grid", gap: 6 }}>
                      {plan.conflicts.slice(0, 10).map((conflict, idx) => (
                        <div key={`conflict:${idx}`} className="card" style={{ padding: 8, borderRadius: 10 }}>
                          <div style={{ fontWeight: 800 }}>{conflict.code}</div>
                          <div className="muted" style={{ marginTop: 2 }}>{conflict.message}</div>
                        </div>
                      ))}
                    </div>
                  ) : null}

                  {plan.resolved_mods.length > 0 ? (
                    <details style={{ marginTop: 10 }}>
                      <summary style={{ cursor: "pointer", fontWeight: 800 }}>
                        Resolved entries ({plan.resolved_mods.length})
                      </summary>
                      <div style={{ marginTop: 8, display: "grid", gap: 6 }}>
                        {plan.resolved_mods.slice(0, 24).map((item) => (
                          <div key={`${item.source}:${item.project_id}:${item.version_id}`} className="card" style={{ padding: 8, borderRadius: 10 }}>
                            <div style={{ fontWeight: 800 }}>
                              {item.name} <span className="chip subtle">{item.source}</span>
                            </div>
                            <div className="muted" style={{ marginTop: 4 }}>{item.version_number}</div>
                            <div className="muted" style={{ marginTop: 2 }}>{item.rationale_text}</div>
                          </div>
                        ))}
                      </div>
                    </details>
                  ) : null}
                </div>
              ) : null}
            </div>
            <div className="footerBar">
              <button className="btn" onClick={() => setApplyWizardOpen(false)}>
                Close
              </button>
            </div>
          </div>
        </div>
      ) : null}
    </div>
  );
}
