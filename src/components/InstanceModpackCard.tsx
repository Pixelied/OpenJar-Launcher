import { useEffect, useMemo, useRef, useState } from "react";
import type {
  DriftReport,
  FriendLinkDriftItem,
  FriendLinkDriftPreview,
  FriendLinkReconcileResult,
  FriendLinkStatus,
  Instance,
  InstanceModpackStatus,
  LayerDiffResult,
} from "../types";
import {
  applyUpdateModpackFromInstance,
  createFriendLinkSession,
  detectInstanceModpackDrift,
  exportFriendLinkDebugBundle,
  getFriendLinkStatus,
  getInstanceModpackStatus,
  joinFriendLinkSession,
  leaveFriendLinkSession,
  previewFriendLinkDrift,
  previewUpdateModpackFromInstance,
  reconcileFriendLink,
  resolveFriendLinkConflicts,
  setFriendLinkGuardrails,
  setFriendLinkPeerAlias,
  syncFriendLinkSelected,
  realignInstanceToModpack,
  rollbackInstanceToLastModpackSnapshot,
} from "../tauri";

type FriendSyncPolicy = "manual" | "ask" | "auto_metadata" | "auto_all";

type FriendSyncPrefs = {
  policy: FriendSyncPolicy;
  snoozed_until: number;
};

const FRIEND_SYNC_PREFS_KEY = "mpm.friend_link.sync_prefs.v1";
const DEFAULT_FRIEND_SYNC_PREFS: FriendSyncPrefs = {
  policy: "ask",
  snoozed_until: 0,
};

function readFriendSyncPrefs(instanceId: string): FriendSyncPrefs {
  if (typeof window === "undefined") return DEFAULT_FRIEND_SYNC_PREFS;
  try {
    const raw = localStorage.getItem(FRIEND_SYNC_PREFS_KEY);
    if (!raw) return DEFAULT_FRIEND_SYNC_PREFS;
    const parsed = JSON.parse(raw) as Record<string, any>;
    const row = parsed?.[instanceId] as Record<string, any> | undefined;
    const policy = String(row?.policy ?? "ask").trim() as FriendSyncPolicy;
    return {
      policy:
        policy === "manual" || policy === "ask" || policy === "auto_metadata" || policy === "auto_all"
          ? policy
          : "ask",
      snoozed_until: Number(row?.snoozed_until ?? 0) || 0,
    };
  } catch {
    return DEFAULT_FRIEND_SYNC_PREFS;
  }
}

function writeFriendSyncPrefs(instanceId: string, prefs: FriendSyncPrefs) {
  if (typeof window === "undefined") return;
  try {
    const raw = localStorage.getItem(FRIEND_SYNC_PREFS_KEY);
    const parsed = raw ? (JSON.parse(raw) as Record<string, any>) : {};
    const next = {
      ...parsed,
      [instanceId]: {
        policy: prefs.policy,
        snoozed_until: Math.max(0, Math.floor(prefs.snoozed_until || 0)),
      },
    };
    localStorage.setItem(FRIEND_SYNC_PREFS_KEY, JSON.stringify(next));
  } catch {
    // ignore persistence failures
  }
}

function stableDriftSignature(preview: FriendLinkDriftPreview | null): string {
  if (!preview) return "";
  const rows = preview.items
    .map((item) => `${item.kind}|${item.key}|${item.change}|${item.peer_id}`)
    .sort();
  return `${preview.status}|${preview.added}|${preview.removed}|${preview.changed}|${rows.join("||")}`;
}

function describeDriftItem(item: FriendLinkDriftItem): string {
  const preview = String(item.theirs_preview ?? item.mine_preview ?? item.key ?? "").trim();
  const line = preview.split("\n")[0] ?? item.key;
  return line.length > 96 ? `${line.slice(0, 93)}...` : line;
}

function driftToastMessage(preview: FriendLinkDriftPreview): string {
  const peers = Array.from(new Set(preview.items.map((item) => item.peer_display_name).filter(Boolean)));
  const peerLabel =
    peers.length === 0 ? "A peer" : peers.length === 1 ? peers[0] : `${peers[0]} +${peers.length - 1}`;
  return `${peerLabel} changed content: +${preview.added} / -${preview.removed} / ~${preview.changed} not synced yet. Sync now?`;
}

function driftLegendTooltip(preview?: FriendLinkDriftPreview | null): string {
  if (!preview) {
    return "Unsynced counters compare friends to your instance: + items friends have that you do not, - items you have that friends do not, ~ same item changed version/settings.";
  }
  const modItems = (preview.items ?? []).filter((item) => item.kind === "lock_entry");
  const configItems = (preview.items ?? []).filter((item) => item.kind !== "lock_entry");
  const modAdded = modItems.filter((item) => item.change === "added").length;
  const modRemoved = modItems.filter((item) => item.change === "removed").length;
  const modChanged = modItems.filter((item) => item.change === "changed").length;
  if (modItems.length === 0) {
    return `Mods are aligned. Config drift detected in ${configItems.length} item${configItems.length === 1 ? "" : "s"}.`;
  }
  return `Mod drift vs your instance: +${modAdded} mods friends have that you do not, -${modRemoved} mods you have that friends do not, ~${modChanged} mods changed version/settings.${configItems.length > 0 ? ` (${configItems.length} config drift item${configItems.length === 1 ? "" : "s"} also present.)` : ""}`;
}

type Props = {
  instance: Instance;
  onNotice: (message: string) => void;
  onError: (message: string) => void;
  onFriendConflict?: (instanceId: string, result: FriendLinkReconcileResult) => void;
  onContentSync?: (instanceId: string) => void;
  onFriendStatusChange?: (instanceId: string, status: FriendLinkStatus | null) => void;
  onDriftPreviewChange?: (instanceId: string, preview: FriendLinkDriftPreview | null) => void;
  onActivity?: (instanceId: string, message: string, tone?: "info" | "success" | "warn" | "error") => void;
};

export default function InstanceModpackCard({
  instance,
  onNotice,
  onError,
  onFriendConflict,
  onContentSync,
  onFriendStatusChange,
  onDriftPreviewChange,
  onActivity,
}: Props) {
  const [status, setStatus] = useState<InstanceModpackStatus | null>(null);
  const [drift, setDrift] = useState<DriftReport | null>(null);
  const [diff, setDiff] = useState<LayerDiffResult | null>(null);
  const [friendStatus, setFriendStatus] = useState<FriendLinkStatus | null>(null);
  const [friendDrift, setFriendDrift] = useState<FriendLinkDriftPreview | null>(null);
  const [inviteCode, setInviteCode] = useState("");
  const [invitePreview, setInvitePreview] = useState<string | null>(null);
  const [friendConflicts, setFriendConflicts] = useState<FriendLinkReconcileResult | null>(null);
  const [resolvingConflicts, setResolvingConflicts] = useState(false);
  const [busy, setBusy] = useState(false);
  const [syncBusy, setSyncBusy] = useState(false);
  const [guardrailsBusy, setGuardrailsBusy] = useState(false);
  const [driftReviewOpen, setDriftReviewOpen] = useState(false);
  const [selectedDriftKeys, setSelectedDriftKeys] = useState<string[]>([]);
  const [guardTrustedPeerIds, setGuardTrustedPeerIds] = useState<string[]>([]);
  const [guardMaxAutoChanges, setGuardMaxAutoChanges] = useState(25);
  const [syncModsEnabled, setSyncModsEnabled] = useState(true);
  const [syncResourcepacksEnabled, setSyncResourcepacksEnabled] = useState(false);
  const [syncShaderpacksEnabled, setSyncShaderpacksEnabled] = useState(true);
  const [syncDatapacksEnabled, setSyncDatapacksEnabled] = useState(true);
  const [guardrailsDirty, setGuardrailsDirty] = useState(false);
  const [peerAliasDrafts, setPeerAliasDrafts] = useState<Record<string, string>>({});
  const [peerAliasSavingId, setPeerAliasSavingId] = useState<string | null>(null);
  const [syncPolicy, setSyncPolicy] = useState<FriendSyncPolicy>(() => readFriendSyncPrefs(instance.id).policy);
  const [snoozedUntil, setSnoozedUntil] = useState<number>(() => readFriendSyncPrefs(instance.id).snoozed_until);

  const lastDriftSignatureRef = useRef("");
  const lastAutoAttemptSignatureRef = useRef("");

  useEffect(() => {
    const prefs = readFriendSyncPrefs(instance.id);
    setSyncPolicy(prefs.policy);
    setSnoozedUntil(prefs.snoozed_until);
    setDriftReviewOpen(false);
    setSelectedDriftKeys([]);
    setGuardrailsDirty(false);
    setSyncModsEnabled(true);
    setSyncResourcepacksEnabled(false);
    setSyncShaderpacksEnabled(true);
    setSyncDatapacksEnabled(true);
    setPeerAliasDrafts({});
    setPeerAliasSavingId(null);
    lastDriftSignatureRef.current = "";
    lastAutoAttemptSignatureRef.current = "";
  }, [instance.id]);

  useEffect(() => {
    if (!friendStatus?.linked) {
      setPeerAliasDrafts({});
      return;
    }
    const peers = friendStatus.peers ?? [];
    setPeerAliasDrafts((prev) => {
      const next: Record<string, string> = {};
      for (const peer of peers) {
        next[peer.peer_id] = prev[peer.peer_id] ?? peer.display_name;
      }
      return next;
    });
  }, [friendStatus?.linked, friendStatus?.peers]);

  useEffect(() => {
    writeFriendSyncPrefs(instance.id, { policy: syncPolicy, snoozed_until: snoozedUntil });
  }, [instance.id, syncPolicy, snoozedUntil]);

  const linked = status?.link?.mode === "linked";
  const now = Date.now();
  const isSnoozed = snoozedUntil > now;

  const driftSummary = useMemo(() => {
    if (!drift) return "No drift data";
    return `${drift.added.length} added · ${drift.removed.length} removed · ${drift.version_changed.length} changed`;
  }, [drift]);

  const friendDriftSummary = useMemo(() => {
    if (!friendDrift) return "No drift preview";
    return `+${friendDrift.added} / -${friendDrift.removed} / ~${friendDrift.changed}`;
  }, [friendDrift]);
  const friendDriftLegend = useMemo(() => driftLegendTooltip(friendDrift), [friendDrift]);
  const friendDriftBreakdown = useMemo(() => {
    const items = friendDrift?.items ?? [];
    const mods = items.filter((item) => item.kind === "lock_entry");
    const config = items.filter((item) => item.kind !== "lock_entry");
    const summarize = (rows: FriendLinkDriftItem[]) => ({
      added: rows.filter((item) => item.change === "added").length,
      removed: rows.filter((item) => item.change === "removed").length,
      changed: rows.filter((item) => item.change === "changed").length,
      total: rows.length,
    });
    return {
      mods: summarize(mods),
      config: summarize(config),
    };
  }, [friendDrift]);
  const friendModDriftSummary = `+${friendDriftBreakdown.mods.added} / -${friendDriftBreakdown.mods.removed} / ~${friendDriftBreakdown.mods.changed}`;

  const friendUnsynced = friendDrift?.status === "unsynced" && friendDriftBreakdown.mods.total > 0;
  const friendConfigOnlyUnsynced =
    friendDrift?.status === "unsynced" && friendDriftBreakdown.mods.total === 0 && friendDriftBreakdown.config.total > 0;
  const peerCount = friendStatus?.peers?.length ?? 0;
  const onlinePeerCount = (friendStatus?.peers ?? []).filter((peer) => peer.online).length;
  const selectedDriftKeySet = useMemo(() => new Set(selectedDriftKeys), [selectedDriftKeys]);

  const syncPolicyLabel =
    syncPolicy === "manual"
      ? "Manual"
      : syncPolicy === "ask"
        ? "Ask every time"
        : syncPolicy === "auto_metadata"
          ? "Auto metadata"
          : "Auto everything";

  function updateFriendStatus(next: FriendLinkStatus | null, options?: { forceGuardrailSync?: boolean }) {
    setFriendStatus(next);
    onFriendStatusChange?.(instance.id, next);
    if (!next) {
      setGuardTrustedPeerIds([]);
      setGuardMaxAutoChanges(25);
      setSyncModsEnabled(true);
      setSyncResourcepacksEnabled(false);
      setSyncShaderpacksEnabled(true);
      setSyncDatapacksEnabled(true);
      return;
    }
    const shouldSyncGuardrails = Boolean(options?.forceGuardrailSync) || !guardrailsDirty;
    if (!shouldSyncGuardrails) {
      return;
    }
    setGuardTrustedPeerIds(next.trusted_peer_ids ?? []);
    setGuardMaxAutoChanges(Math.max(1, Number(next.max_auto_changes ?? 25) || 25));
    setSyncModsEnabled(Boolean(next.sync_mods ?? true));
    setSyncResourcepacksEnabled(Boolean(next.sync_resourcepacks ?? false));
    setSyncShaderpacksEnabled(Boolean(next.sync_shaderpacks ?? true));
    setSyncDatapacksEnabled(Boolean(next.sync_datapacks ?? true));
  }

  function updateFriendDrift(preview: FriendLinkDriftPreview | null, options?: { quiet?: boolean }) {
    setFriendDrift(preview);
    onDriftPreviewChange?.(instance.id, preview);

    if (!preview) {
      lastDriftSignatureRef.current = "";
      return;
    }

    const signature = stableDriftSignature(preview);
    const hasModDrift = (preview.items ?? []).some((item) => item.kind === "lock_entry");
    if (preview.status === "unsynced" && preview.total_changes > 0 && hasModDrift && signature !== lastDriftSignatureRef.current) {
      const message = driftToastMessage(preview);
      if (!options?.quiet && !isSnoozed && (syncPolicy === "ask" || syncPolicy === "manual")) {
        onNotice(message);
      }
      if (!isSnoozed && syncPolicy === "ask") {
        setDriftReviewOpen(true);
      }
      if (!selectedDriftKeys.length) {
        setSelectedDriftKeys(preview.items.map((item) => item.key));
      }
    }

    lastDriftSignatureRef.current = signature;
  }

  async function refresh(options?: { quiet?: boolean }) {
    try {
      const [statusInfo, driftInfo, linkedStatus] = await Promise.all([
        getInstanceModpackStatus({ instanceId: instance.id }),
        detectInstanceModpackDrift({ instanceId: instance.id }).catch(() => null),
        getFriendLinkStatus({ instanceId: instance.id }).catch(() => null),
      ]);
      setStatus(statusInfo);
      setDrift(driftInfo);
      updateFriendStatus(linkedStatus);

      if (linkedStatus?.linked) {
        const preview = await previewFriendLinkDrift({ instanceId: instance.id }).catch(() => null);
        updateFriendDrift(preview, options);
      } else {
        updateFriendDrift(null, options);
      }
    } catch (err: any) {
      onError(err?.toString?.() ?? String(err));
    }
  }

  async function runSelectiveSync(input: { keys: string[]; metadataOnly: boolean; reason: "manual" | "auto" }) {
    setSyncBusy(true);
    try {
      const out = await syncFriendLinkSelected({
        instanceId: instance.id,
        keys: input.keys,
        metadataOnly: input.metadataOnly,
      });
      if (out.status === "conflicted") {
        setFriendConflicts(out);
        onFriendConflict?.(instance.id, out);
        onNotice("Friend Link found conflicts. Resolve before launching.");
      } else if (out.status === "blocked_untrusted") {
        const warningSuffix =
          out.warnings.length > 0 ? ` ${out.warnings.length} warning${out.warnings.length === 1 ? "" : "s"}.` : "";
        onNotice(`Friend Link skipped untrusted peers. Trust that peer before syncing.${warningSuffix}`);
      } else {
        const warningSuffix =
          out.warnings.length > 0 ? ` ${out.warnings.length} warning${out.warnings.length === 1 ? "" : "s"}.` : "";
        onNotice(`Friend Link sync: ${out.status}. Applied ${out.actions_applied} changes.${warningSuffix}`);
      }
      onActivity?.(
        instance.id,
        `Friend Link ${input.reason === "auto" ? "auto-sync" : "sync"}: ${out.status}. Applied ${out.actions_applied} change${out.actions_applied === 1 ? "" : "s"}.`,
        out.status === "synced" || out.status === "in_sync" ? "success" : "info"
      );
      if (out.actions_applied > 0) {
        onContentSync?.(instance.id);
      }
      await refresh({ quiet: true });
    } catch (err: any) {
      onError(err?.toString?.() ?? String(err));
    } finally {
      setSyncBusy(false);
    }
  }

  async function runManualSync(reason: "manual" | "auto" = "manual") {
    setSyncBusy(true);
    try {
      let out = await reconcileFriendLink({ instanceId: instance.id, mode: "manual" });
      if (out.status === "degraded_missing_files") {
        const preview = await previewFriendLinkDrift({ instanceId: instance.id }).catch(() => null);
        const retry = await syncFriendLinkSelected({
          instanceId: instance.id,
          keys: preview?.items.map((item) => item.key) ?? [],
          metadataOnly: false,
        }).catch(() => null);
        if (retry) {
          out = retry;
        }
      }

      if (out.status === "conflicted") {
        setFriendConflicts(out);
        onFriendConflict?.(instance.id, out);
        onNotice("Friend Link found conflicts. Resolve before launching.");
      } else if (out.status === "blocked_untrusted") {
        const warningSuffix =
          out.warnings.length > 0 ? ` ${out.warnings.length} warning${out.warnings.length === 1 ? "" : "s"}.` : "";
        onNotice(`Friend Link skipped untrusted peers. Trust that peer before syncing.${warningSuffix}`);
      } else {
        const warningSuffix =
          out.warnings.length > 0 ? ` ${out.warnings.length} warning${out.warnings.length === 1 ? "" : "s"}.` : "";
        onNotice(`Friend Link sync: ${out.status}. Applied ${out.actions_applied} changes.${warningSuffix}`);
      }
      onActivity?.(
        instance.id,
        `Friend Link ${reason === "auto" ? "auto-sync" : "sync"}: ${out.status}. Applied ${out.actions_applied} change${out.actions_applied === 1 ? "" : "s"}.`,
        out.status === "synced" || out.status === "in_sync" ? "success" : "info"
      );
      if (out.actions_applied > 0) {
        onContentSync?.(instance.id);
      }
      await refresh({ quiet: true });
    } catch (err: any) {
      onError(err?.toString?.() ?? String(err));
    } finally {
      setSyncBusy(false);
    }
  }

  useEffect(() => {
    let cancelled = false;
    const tick = () => {
      if (cancelled) return;
      void refresh({ quiet: true }).catch(() => null);
    };
    tick();
    const timer = window.setInterval(tick, 10000);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [instance.id]);

  useEffect(() => {
    if (!friendDrift || !friendStatus?.linked) {
      lastAutoAttemptSignatureRef.current = "";
      return;
    }
    if (friendDrift.status !== "unsynced" || friendDrift.total_changes === 0) {
      lastAutoAttemptSignatureRef.current = "";
      return;
    }
    if (syncPolicy !== "auto_metadata" && syncPolicy !== "auto_all") return;
    if (isSnoozed || syncBusy) return;

    const signature = stableDriftSignature(friendDrift);
    if (!signature || lastAutoAttemptSignatureRef.current === signature) return;

    if (friendDrift.total_changes > Math.max(1, guardMaxAutoChanges)) {
      lastAutoAttemptSignatureRef.current = signature;
      onNotice(
        `Friend Link auto-sync paused: ${friendDrift.total_changes} changes exceed your guardrail (${guardMaxAutoChanges}).`
      );
      setDriftReviewOpen(true);
      return;
    }

    lastAutoAttemptSignatureRef.current = signature;
    if (syncPolicy === "auto_all") {
      void runManualSync("auto");
    } else {
      const keys = friendDrift.items.map((item) => item.key);
      void runSelectiveSync({
        keys,
        metadataOnly: true,
        reason: "auto",
      });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [friendDrift, friendStatus?.linked, syncPolicy, isSnoozed, syncBusy, guardMaxAutoChanges]);

  useEffect(() => {
    if (!friendDrift) {
      setSelectedDriftKeys([]);
      return;
    }
    const known = new Set(friendDrift.items.map((item) => item.key));
    setSelectedDriftKeys((prev) => {
      const next = prev.filter((key) => known.has(key));
      if (next.length > 0) return next;
      if (!driftReviewOpen) return [];
      return friendDrift.items.map((item) => item.key);
    });
  }, [friendDrift, driftReviewOpen]);

  return (
    <div className="card" style={{ padding: 14, borderRadius: 16 }}>
      <div className="rowBetween">
        <div style={{ fontWeight: 900 }}>Modpack link</div>
        <span className="chip subtle">{status?.link ? status.link.mode : "unlinked"}</span>
      </div>

      {!status?.link ? (
        <div className="muted" style={{ marginTop: 8 }}>
          This instance is not linked to a modpack yet.
        </div>
      ) : (
        <>
          <div style={{ marginTop: 8 }}>
            <div className="muted">Modpack</div>
            <div style={{ fontWeight: 800 }}>{status.link.modpack_id}</div>
          </div>

          <div style={{ marginTop: 8 }}>
            <div className="muted">Confidence</div>
            <div style={{ fontWeight: 700 }}>{status.link.last_confidence_label ?? "Unknown"}</div>
          </div>

          <div style={{ marginTop: 8 }}>
            <div className="muted">Drift</div>
            <div style={{ fontWeight: 700 }}>{drift?.status ?? "unknown"}</div>
            <div className="muted">{driftSummary}</div>
          </div>

          <div className="row" style={{ marginTop: 10, gap: 8, flexWrap: "wrap" }}>
            <button
              className="btn"
              disabled={busy || syncBusy}
              onClick={async () => {
                setBusy(true);
                try {
                  await refresh();
                  onNotice("Modpack status refreshed.");
                } finally {
                  setBusy(false);
                }
              }}
            >
              Refresh
            </button>

            <button
              className="btn"
              disabled={busy || syncBusy || !linked}
              onClick={async () => {
                setBusy(true);
                try {
                  const out = await realignInstanceToModpack({ instanceId: instance.id });
                  onNotice(out.message);
                  await refresh({ quiet: true });
                } catch (err: any) {
                  onError(err?.toString?.() ?? String(err));
                } finally {
                  setBusy(false);
                }
              }}
            >
              Re-align
            </button>

            <button
              className="btn"
              disabled={busy || syncBusy || !status?.link?.last_lock_snapshot_id}
              onClick={async () => {
                setBusy(true);
                try {
                  const out = await rollbackInstanceToLastModpackSnapshot({ instanceId: instance.id });
                  onNotice(`${out.message} Restored ${out.restored_files} file(s).`);
                  await refresh({ quiet: true });
                } catch (err: any) {
                  onError(err?.toString?.() ?? String(err));
                } finally {
                  setBusy(false);
                }
              }}
            >
              Rollback
            </button>
          </div>

          <div className="row" style={{ marginTop: 8, gap: 8, flexWrap: "wrap" }}>
            <button
              className="btn"
              disabled={busy || syncBusy || !status.link.modpack_id}
              onClick={async () => {
                setBusy(true);
                try {
                  const out = await previewUpdateModpackFromInstance({
                    instanceId: instance.id,
                    modpackId: status.link!.modpack_id,
                  });
                  setDiff(out);
                } catch (err: any) {
                  onError(err?.toString?.() ?? String(err));
                } finally {
                  setBusy(false);
                }
              }}
            >
              Preview update modpack from instance
            </button>

            <button
              className="btn"
              disabled={busy || syncBusy || !status.link.modpack_id || !diff}
              onClick={async () => {
                setBusy(true);
                try {
                  await applyUpdateModpackFromInstance({
                    instanceId: instance.id,
                    modpackId: status.link!.modpack_id,
                  });
                  onNotice("Applied instance diff to modpack overrides layer.");
                  setDiff(null);
                  await refresh({ quiet: true });
                } catch (err: any) {
                  onError(err?.toString?.() ?? String(err));
                } finally {
                  setBusy(false);
                }
              }}
            >
              Apply diff to modpack
            </button>
          </div>

          {diff ? (
            <div className="card" style={{ marginTop: 10, padding: 10, borderRadius: 12 }}>
              <div style={{ fontWeight: 800 }}>Pending diff</div>
              <div className="muted" style={{ marginTop: 4 }}>
                {diff.added.length} added · {diff.removed.length} removed · {diff.overridden.length} overridden
              </div>
              {diff.warnings.length > 0 ? (
                <div className="muted" style={{ marginTop: 6 }}>
                  {diff.warnings.join(" | ")}
                </div>
              ) : null}
            </div>
          ) : null}
        </>
      )}

      <div style={{ marginTop: 16, borderTop: "1px solid rgba(255,255,255,0.08)", paddingTop: 12 }}>
        <div className="rowBetween" style={{ gap: 8, flexWrap: "wrap" }}>
          <div style={{ fontWeight: 900 }}>Friend Link</div>
          <div className="row" style={{ gap: 6, flexWrap: "wrap" }}>
            <span
              className="chip subtle"
              title="Current Friend Link session status for this instance."
            >
              {friendStatus?.linked ? friendStatus.status || "linked" : "unlinked"}
            </span>
            {friendStatus?.linked && friendDrift ? (
              <span className={`chip ${friendUnsynced ? "danger" : "subtle"}`} title={friendDriftLegend}>
                {friendUnsynced
                  ? `Unsynced mods ${friendModDriftSummary}`
                  : friendConfigOnlyUnsynced
                    ? `Config drift ${friendDriftBreakdown.config.total}`
                    : "Synced"}
              </span>
            ) : null}
          </div>
        </div>
        {friendStatus?.linked ? (
          <>
            <div className="card friendLinkPanelCard" style={{ marginTop: 10 }}>
              <div className="friendLinkOverview">
                <div>
                  <div className="friendLinkOverviewTitle">Session overview</div>
                  <div className="muted" title="Friend group ID and current online peers in this link session.">
                    Group {friendStatus.group_id} · {onlinePeerCount}/{peerCount} peer{peerCount === 1 ? "" : "s"} online
                  </div>
                  <div className="muted">
                    Policy: {syncPolicyLabel}
                    {isSnoozed
                      ? ` · Snoozed until ${new Date(snoozedUntil).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}`
                      : ""}
                  </div>
                  <div className="muted" title="Compared to your instance: + friends have it and you do not, - you have it and friends do not, ~ same item changed version/settings.">
                    Compared to your instance: + they have it, - you have it, ~ changed version/settings.
                  </div>
                  {friendConfigOnlyUnsynced ? (
                    <div className="muted" title={friendDriftLegend}>
                      Mods are aligned. Config drift: {friendDriftBreakdown.config.total} item{friendDriftBreakdown.config.total === 1 ? "" : "s"}.
                    </div>
                  ) : null}
                </div>
                <div className="friendLinkOverviewBadges">
                  <span className={`chip ${friendUnsynced ? "danger" : ""}`} title={friendDriftLegend}>
                    {friendUnsynced
                      ? `Unsynced mods ${friendModDriftSummary}`
                      : friendConfigOnlyUnsynced
                        ? "Config drift only"
                        : "Synced"}
                  </span>
                  <span className="chip subtle" title="Number of unresolved Friend Link conflicts for this instance.">
                    {friendStatus.pending_conflicts_count} conflict{friendStatus.pending_conflicts_count === 1 ? "" : "s"}
                  </span>
                </div>
              </div>
            </div>

            {friendStatus.peers?.length ? (
              <div className="card friendLinkPanelCard friendLinkPeersCard" style={{ marginTop: 10 }}>
                <div className="friendLinkSectionTitle">Peers and safety overrides</div>
                <div className="muted friendLinkPeerMeta" style={{ marginTop: 4 }}>
                  Untrusted peers are fully blocked: no manual or automatic sync is allowed from them until you trust them again.
                </div>
                <div className="friendLinkPeersList">
                {friendStatus.peers.map((peer) => {
                  const trusted = guardTrustedPeerIds.includes(peer.peer_id);
                  const aliasValue = peerAliasDrafts[peer.peer_id] ?? peer.display_name;
                  const aliasDirty = aliasValue.trim() !== peer.display_name.trim();
                  return (
                    <div
                      key={peer.peer_id}
                      className="friendLinkPeerRow"
                    >
                      <div>
                        <div className="friendLinkPeerName">{peer.display_name}</div>
                        <div className="muted friendLinkPeerMeta">
                          {peer.online ? "online" : "offline"} · {peer.peer_id}
                        </div>
                      </div>
                      <div className="friendLinkPeerActions">
                        <div className="friendLinkPeerEditRow">
                          <input
                            className="input friendLinkPeerAliasInput"
                            value={aliasValue}
                            maxLength={48}
                            title="Set a local nickname for this peer so it is easier to identify them."
                            placeholder="Peer nickname"
                            onChange={(event) => {
                              const value = event.target.value;
                              setPeerAliasDrafts((prev) => ({ ...prev, [peer.peer_id]: value }));
                            }}
                          />
                          <button
                            className="btn"
                            disabled={peerAliasSavingId === peer.peer_id || !aliasDirty}
                            title="Save this peer nickname on your launcher."
                            onClick={async () => {
                              setPeerAliasSavingId(peer.peer_id);
                              try {
                                const updated = await setFriendLinkPeerAlias({
                                  instanceId: instance.id,
                                  peerId: peer.peer_id,
                                  displayName: aliasValue,
                                });
                                updateFriendStatus(updated, { forceGuardrailSync: true });
                                onNotice("Peer nickname updated.");
                                await refresh({ quiet: true });
                              } catch (err: any) {
                                onError(err?.toString?.() ?? String(err));
                              } finally {
                                setPeerAliasSavingId(null);
                              }
                            }}
                          >
                            {peerAliasSavingId === peer.peer_id ? "Saving…" : "Save name"}
                          </button>
                          <button
                            className="btn"
                            disabled={peerAliasSavingId === peer.peer_id}
                            title="Reset back to the peer's original shared name."
                            onClick={async () => {
                              setPeerAliasSavingId(peer.peer_id);
                              try {
                                const updated = await setFriendLinkPeerAlias({
                                  instanceId: instance.id,
                                  peerId: peer.peer_id,
                                  displayName: "",
                                });
                                updateFriendStatus(updated, { forceGuardrailSync: true });
                                onNotice("Peer nickname reset.");
                                await refresh({ quiet: true });
                              } catch (err: any) {
                                onError(err?.toString?.() ?? String(err));
                              } finally {
                                setPeerAliasSavingId(null);
                              }
                            }}
                          >
                            Reset
                          </button>
                        </div>
                        <div className="friendLinkPeerTrustRow">
                          <span
                            className={`chip ${trusted ? "" : "subtle"}`}
                            title={
                              trusted
                                ? "Trusted peer: sync is allowed from this peer."
                                : "Untrusted peer: sync is blocked from this peer until trusted."
                            }
                          >
                            {trusted ? "Trusted" : "Untrusted"}
                          </span>
                          <button
                            className={`btn ${trusted ? "" : "primary"}`}
                            title={trusted ? "Block syncing from this peer." : "Allow syncing from this peer again."}
                            onClick={() => {
                              setGuardrailsDirty(true);
                              setGuardTrustedPeerIds((prev) =>
                                trusted ? prev.filter((id) => id !== peer.peer_id) : [...prev, peer.peer_id]
                              );
                            }}
                          >
                            {trusted ? "Untrust peer" : "Trust peer"}
                          </button>
                        </div>
                      </div>
                    </div>
                  );
                })}
                </div>
              </div>
            ) : (
              <div className="muted" style={{ marginTop: 6 }}>
                No followers joined yet.
              </div>
            )}

            <div className="card friendLinkPanelCard" style={{ marginTop: 10 }}>
              <div className="friendLinkSectionTitle">Policy and guardrails</div>
              <div className="friendLinkSyncTypes" style={{ marginTop: 10 }}>
                <div className="friendLinkSubsectionTitle">What to sync</div>
                <div className="friendLinkSyncTypesGrid" style={{ marginTop: 8 }}>
                  <div className="friendLinkSyncTypeRow">
                    <div>
                      <div className="friendLinkSyncTypeLabel">Mods</div>
                      <div className="muted friendLinkPeerMeta">Sync mod metadata and files.</div>
                    </div>
                    <button
                      className={`btn ${syncModsEnabled ? "primary" : ""}`}
                      disabled={guardrailsBusy}
                      title="Toggle mod syncing for this instance."
                      onClick={() => {
                        setGuardrailsDirty(true);
                        setSyncModsEnabled((prev) => !prev);
                      }}
                    >
                      {syncModsEnabled ? "On" : "Off"}
                    </button>
                  </div>
                  <div className="friendLinkSyncTypeRow">
                    <div>
                      <div className="friendLinkSyncTypeLabel">Resource packs (texture packs)</div>
                      <div className="muted friendLinkPeerMeta">Optional visual packs. Default is Off.</div>
                    </div>
                    <button
                      className={`btn ${syncResourcepacksEnabled ? "primary" : ""}`}
                      disabled={guardrailsBusy}
                      title="Toggle resource/texture pack syncing for this instance."
                      onClick={() => {
                        setGuardrailsDirty(true);
                        setSyncResourcepacksEnabled((prev) => !prev);
                      }}
                    >
                      {syncResourcepacksEnabled ? "On" : "Off"}
                    </button>
                  </div>
                  <div className="friendLinkSyncTypeRow">
                    <div>
                      <div className="friendLinkSyncTypeLabel">Shader packs</div>
                      <div className="muted friendLinkPeerMeta">Sync shader pack metadata and files.</div>
                    </div>
                    <button
                      className={`btn ${syncShaderpacksEnabled ? "primary" : ""}`}
                      disabled={guardrailsBusy}
                      title="Toggle shader pack syncing for this instance."
                      onClick={() => {
                        setGuardrailsDirty(true);
                        setSyncShaderpacksEnabled((prev) => !prev);
                      }}
                    >
                      {syncShaderpacksEnabled ? "On" : "Off"}
                    </button>
                  </div>
                  <div className="friendLinkSyncTypeRow">
                    <div>
                      <div className="friendLinkSyncTypeLabel">Datapacks</div>
                      <div className="muted friendLinkPeerMeta">Sync world datapack metadata and files.</div>
                    </div>
                    <button
                      className={`btn ${syncDatapacksEnabled ? "primary" : ""}`}
                      disabled={guardrailsBusy}
                      title="Toggle datapack syncing for this instance."
                      onClick={() => {
                        setGuardrailsDirty(true);
                        setSyncDatapacksEnabled((prev) => !prev);
                      }}
                    >
                      {syncDatapacksEnabled ? "On" : "Off"}
                    </button>
                  </div>
                </div>
              </div>
              <div className="row" style={{ marginTop: 8, gap: 8, flexWrap: "wrap" }}>
                <span className="muted" style={{ alignSelf: "center" }}>
                  Max auto changes:
                </span>
                <input
                  className="input"
                  style={{ width: 110 }}
                  type="number"
                  min={1}
                  max={500}
                  title="Safety limit for automatic sync actions. If a drift has more changes than this, auto-sync pauses for review."
                  value={guardMaxAutoChanges}
                  onChange={(event) => {
                    setGuardrailsDirty(true);
                    const next = Number(event.target.value || 25);
                    setGuardMaxAutoChanges(Math.max(1, Math.min(500, Number.isFinite(next) ? next : 25)));
                  }}
                />
                <button
                  className="btn"
                  disabled={guardrailsBusy || !guardrailsDirty}
                  title="Save trusted peers and auto-sync safety threshold."
                  onClick={async () => {
                    setGuardrailsBusy(true);
                    try {
                      const updated = await setFriendLinkGuardrails({
                        instanceId: instance.id,
                        trustedPeerIds: guardTrustedPeerIds,
                        maxAutoChanges: guardMaxAutoChanges,
                        syncMods: syncModsEnabled,
                        syncResourcepacks: syncResourcepacksEnabled,
                        syncShaderpacks: syncShaderpacksEnabled,
                        syncDatapacks: syncDatapacksEnabled,
                      });
                      updateFriendStatus(updated, { forceGuardrailSync: true });
                      setGuardrailsDirty(false);
                      onNotice("Friend Link sync settings updated.");
                    } catch (err: any) {
                      onError(err?.toString?.() ?? String(err));
                    } finally {
                      setGuardrailsBusy(false);
                    }
                  }}
                >
                  {guardrailsBusy ? "Saving…" : "Save guardrails"}
                </button>
              </div>
              <div className="row" style={{ marginTop: 10, gap: 8, flexWrap: "wrap" }}>
                <button
                  className={`btn ${syncPolicy === "manual" ? "primary" : ""}`}
                  title="Manual mode: no automatic syncing. You control all sync actions."
                  onClick={() => setSyncPolicy("manual")}
                  disabled={syncBusy}
                >
                  Manual
                </button>
                <button
                  className={`btn ${syncPolicy === "ask" ? "primary" : ""}`}
                  title="Ask every time: show prompt when unsynced changes are detected."
                  onClick={() => setSyncPolicy("ask")}
                  disabled={syncBusy}
                >
                  Ask every time
                </button>
                <button
                  className={`btn ${syncPolicy === "auto_metadata" ? "primary" : ""}`}
                  title="Auto metadata only: update lock/config metadata automatically, without forcing full file fetch for every change."
                  onClick={() => setSyncPolicy("auto_metadata")}
                  disabled={syncBusy}
                >
                  Auto metadata only
                </button>
                <button
                  className={`btn ${syncPolicy === "auto_all" ? "primary" : ""}`}
                  title="Auto-sync everything: automatically reconcile and sync full changes when guardrails allow it."
                  onClick={() => setSyncPolicy("auto_all")}
                  disabled={syncBusy}
                >
                  Auto-sync everything
                </button>
              </div>
            </div>

            {friendUnsynced && !isSnoozed ? (
              <div className="card" style={{ marginTop: 10, padding: 10, borderRadius: 12 }}>
                <div style={{ fontWeight: 850 }} title={friendDriftLegend}>Unsynced changes detected</div>
                <div className="muted" style={{ marginTop: 4 }} title={friendDriftLegend}>
                  {friendDriftSummary} across {friendDrift?.total_changes ?? 0} item{(friendDrift?.total_changes ?? 0) === 1 ? "" : "s"}.
                </div>
                <div className="row" style={{ marginTop: 8, gap: 8, flexWrap: "wrap" }}>
                  <button
                    className="btn"
                    disabled={syncBusy}
                    title="Open selective review to choose which changes to sync."
                    onClick={() => {
                      setDriftReviewOpen(true);
                      if ((friendDrift?.items.length ?? 0) > 0 && selectedDriftKeys.length === 0) {
                        setSelectedDriftKeys(friendDrift!.items.map((item) => item.key));
                      }
                    }}
                  >
                    Review changes
                  </button>
                  <button
                    className="btn primary"
                    disabled={syncBusy}
                    title="Run full Friend Link sync now (with retry fallback if files are missing)."
                    onClick={() => void runManualSync("manual")}
                  >
                    {syncBusy ? "Syncing…" : "Sync now"}
                  </button>
                  <button
                    className="btn"
                    disabled={syncBusy}
                    title="Mute sync prompts for 30 minutes."
                    onClick={() => {
                      const until = Date.now() + 30 * 60 * 1000;
                      setSnoozedUntil(until);
                      onNotice("Friend Link sync reminders snoozed for 30 minutes.");
                    }}
                  >
                    Snooze 30m
                  </button>
                </div>
              </div>
            ) : null}

            {friendUnsynced && isSnoozed ? (
              <div className="card" style={{ marginTop: 10, padding: 10, borderRadius: 12 }}>
                <div className="rowBetween" style={{ gap: 8 }}>
                  <div className="muted">
                    Sync prompts are snoozed until {new Date(snoozedUntil).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}.
                  </div>
                  <button
                    className="btn"
                    title="Resume sync prompts immediately."
                    onClick={() => {
                      setSnoozedUntil(0);
                      onNotice("Friend Link sync reminders resumed.");
                    }}
                  >
                    Resume now
                  </button>
                </div>
              </div>
            ) : null}

            {(driftReviewOpen || friendStatus.pending_conflicts_count > 0) && friendDrift ? (
              <div className="card" style={{ marginTop: 10, padding: 10, borderRadius: 12 }}>
                <div className="rowBetween" style={{ gap: 8, flexWrap: "wrap" }}>
                  <div style={{ fontWeight: 800 }}>Review drift changes</div>
                  <div className="row" style={{ gap: 6, flexWrap: "wrap" }}>
                    <span className="chip subtle" title="How many drift rows are currently selected for sync.">Selected {selectedDriftKeySet.size}</span>
                    <span className="chip subtle" title="Total drift rows detected from peers.">Total {friendDrift.total_changes}</span>
                    <button className="btn" title="Close drift review panel." onClick={() => setDriftReviewOpen(false)}>
                      Close
                    </button>
                  </div>
                </div>
                <div className="row" style={{ marginTop: 8, gap: 8, flexWrap: "wrap" }}>
                  <button
                    className="btn"
                    title="Select every drift row."
                    onClick={() => setSelectedDriftKeys(friendDrift.items.map((item) => item.key))}
                    disabled={friendDrift.items.length === 0}
                  >
                    Select all
                  </button>
                  <button className="btn" title="Unselect every drift row." onClick={() => setSelectedDriftKeys([])}>
                    Clear selection
                  </button>
                  <button
                    className="btn"
                    disabled={syncBusy || selectedDriftKeySet.size === 0}
                    title="Sync only selected rows, including file changes."
                    onClick={() =>
                      void runSelectiveSync({
                        keys: Array.from(selectedDriftKeySet),
                        metadataOnly: false,
                        reason: "manual",
                      })
                    }
                  >
                    {syncBusy ? "Syncing…" : "Sync selected"}
                  </button>
                  <button
                    className="btn"
                    disabled={syncBusy || selectedDriftKeySet.size === 0}
                    title="Sync selected rows in metadata mode (lock/config updates only)."
                    onClick={() =>
                      void runSelectiveSync({
                        keys: Array.from(selectedDriftKeySet),
                        metadataOnly: true,
                        reason: "manual",
                      })
                    }
                  >
                    Sync metadata only
                  </button>
                </div>
                <div style={{ display: "grid", gap: 7, marginTop: 10, maxHeight: 300, overflowY: "auto", paddingRight: 2 }}>
                  {friendDrift.items.map((item) => {
                    const checked = selectedDriftKeySet.has(item.key);
                    return (
                      <label
                        key={`${item.kind}:${item.key}:${item.peer_id}:${item.change}`}
                        className="rowBetween"
                        style={{
                          alignItems: "flex-start",
                          gap: 10,
                          border: "1px solid var(--stroke)",
                          borderRadius: 10,
                          padding: "8px 10px",
                        }}
                      >
                        <div style={{ display: "grid", gap: 4, minWidth: 0 }}>
                          <div style={{ fontWeight: 700, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                            {describeDriftItem(item)}
                          </div>
                          <div className="muted" style={{ fontSize: 12 }}>
                            {item.kind.replace("_", " ")} · {item.change} · {item.peer_display_name}
                          </div>
                          <div className="muted" style={{ fontSize: 12, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                            {item.key}
                          </div>
                        </div>
                        <div style={{ display: "inline-flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
                          <span
                            className={`chip ${item.trusted_peer ? "" : "subtle"}`}
                            title={item.trusted_peer ? "Trusted peer: this row can be synced." : "Untrusted peer: this row cannot be synced until you trust that peer."}
                          >
                            {item.trusted_peer ? "Trusted" : "Untrusted"}
                          </span>
                          <input
                            type="checkbox"
                            checked={checked}
                            disabled={!item.trusted_peer}
                            onChange={(event) => {
                              if (!item.trusted_peer) {
                                return;
                              }
                              setSelectedDriftKeys((prev) => {
                                if (event.target.checked) {
                                  if (prev.includes(item.key)) return prev;
                                  return [...prev, item.key];
                                }
                                return prev.filter((value) => value !== item.key);
                              });
                            }}
                            title={item.trusted_peer ? "Select this row for sync." : "Cannot select untrusted peer changes."}
                            aria-label={`Select ${item.key}`}
                          />
                        </div>
                      </label>
                    );
                  })}
                </div>
              </div>
            ) : null}

            <div className="card friendLinkPanelCard" style={{ marginTop: 10 }}>
              <div className="friendLinkSectionTitle">Session actions</div>
              <div className="row" style={{ marginTop: 8, gap: 8, flexWrap: "wrap" }}>
              <button
                className="btn"
                disabled={busy || syncBusy}
                title="Regenerate and copy a fresh invite code for this session."
                onClick={async () => {
                  setBusy(true);
                  try {
                    const invite = await createFriendLinkSession({ instanceId: instance.id });
                    setInvitePreview(invite.invite_code);
                    setGuardrailsDirty(false);
                    onNotice("Generated fresh invite code.");
                    await refresh({ quiet: true });
                  } catch (err: any) {
                    onError(err?.toString?.() ?? String(err));
                  } finally {
                    setBusy(false);
                  }
                }}
              >
                Refresh invite
              </button>
              <button
                className="btn"
                disabled={busy || syncBusy}
                title="Run full Friend Link sync now (with retry fallback if files are missing)."
                onClick={() => void runManualSync("manual")}
              >
                Sync now
              </button>
              <button
                className="btn"
                disabled={busy || syncBusy}
                title="Export a Friend Link debug bundle for troubleshooting."
                onClick={async () => {
                  setBusy(true);
                  try {
                    const out = await exportFriendLinkDebugBundle({ instanceId: instance.id });
                    onNotice(`Exported debug bundle to ${out.path}`);
                  } catch (err: any) {
                    onError(err?.toString?.() ?? String(err));
                  } finally {
                    setBusy(false);
                  }
                }}
              >
                Export debug
              </button>
              <button
                className="btn danger"
                disabled={busy || syncBusy}
                title="Leave this Friend Link session for this instance."
                onClick={async () => {
                  setBusy(true);
                  try {
                    await leaveFriendLinkSession({ instanceId: instance.id });
                    setInvitePreview(null);
                    setFriendConflicts(null);
                    setGuardrailsDirty(false);
                    updateFriendDrift(null, { quiet: true });
                    updateFriendStatus(null);
                    onNotice("Left friend link session.");
                    await refresh({ quiet: true });
                  } catch (err: any) {
                    onError(err?.toString?.() ?? String(err));
                  } finally {
                    setBusy(false);
                  }
                }}
              >
                Leave link
              </button>
              </div>
            </div>
            {friendStatus.pending_conflicts_count > 0 ? (
              <div className="muted" style={{ marginTop: 6 }}>
                {friendStatus.pending_conflicts_count} pending conflict{friendStatus.pending_conflicts_count === 1 ? "" : "s"}.
              </div>
            ) : null}
          </>
        ) : (
          <>
            <div className="muted" style={{ marginTop: 8 }}>
              Create a link as host or join a friend with an invite code.
            </div>
            <div className="row" style={{ marginTop: 8, gap: 8, flexWrap: "wrap" }}>
              <button
                className="btn"
                disabled={busy || syncBusy}
                title="Create this instance as a Friend Link host and generate an invite."
                onClick={async () => {
                  setBusy(true);
                  try {
                    const invite = await createFriendLinkSession({ instanceId: instance.id });
                    setInvitePreview(invite.invite_code);
                    setGuardrailsDirty(false);
                    onNotice("Friend link created. Share the invite code.");
                    await refresh({ quiet: true });
                  } catch (err: any) {
                    onError(err?.toString?.() ?? String(err));
                  } finally {
                    setBusy(false);
                  }
                }}
              >
                Create host link
              </button>
            </div>
            <div style={{ marginTop: 8 }}>
              <input
                className="input"
                placeholder="Paste invite code"
                title="Paste an invite code from another host to join their Friend Link."
                value={inviteCode}
                onChange={(e) => setInviteCode(e.target.value)}
              />
            </div>
            <div className="row" style={{ marginTop: 8, gap: 8 }}>
              <button
                className="btn primary"
                disabled={busy || syncBusy || !inviteCode.trim()}
                title="Join this Friend Link session and run an initial sync."
                onClick={async () => {
                  setBusy(true);
                  try {
                    await joinFriendLinkSession({
                      instanceId: instance.id,
                      inviteCode: inviteCode.trim(),
                    });
                    const firstSync = await reconcileFriendLink({ instanceId: instance.id, mode: "manual" });
                    if (firstSync.status === "conflicted") {
                      setFriendConflicts(firstSync);
                      onFriendConflict?.(instance.id, firstSync);
                    } else {
                      setFriendConflicts(null);
                      if (firstSync.actions_applied > 0) {
                        onContentSync?.(instance.id);
                      }
                    }
                    setInviteCode("");
                    setGuardrailsDirty(false);
                    const warningSuffix =
                      firstSync.warnings.length > 0
                        ? ` ${firstSync.warnings.length} warning${firstSync.warnings.length === 1 ? "" : "s"}.`
                        : "";
                    onNotice(
                      `Joined friend link. Sync ${firstSync.status}; applied ${firstSync.actions_applied} change${
                        firstSync.actions_applied === 1 ? "" : "s"
                      }.${warningSuffix}`
                    );
                    await refresh({ quiet: true });
                  } catch (err: any) {
                    onError(err?.toString?.() ?? String(err));
                  } finally {
                    setBusy(false);
                  }
                }}
              >
                Join link
              </button>
            </div>
          </>
        )}
        {invitePreview ? (
          <div style={{ marginTop: 8 }}>
            <div className="muted">Invite code</div>
            <textarea className="textarea" value={invitePreview} readOnly style={{ minHeight: 74 }} />
          </div>
        ) : null}
        {friendConflicts?.conflicts?.length ? (
          <div className="card" style={{ marginTop: 10, padding: 10, borderRadius: 12 }}>
            <div style={{ fontWeight: 800 }}>Conflicts ({friendConflicts.conflicts.length})</div>
            <div className="muted" style={{ marginTop: 4 }}>
              Resolve by keeping your local state or taking peer state.
            </div>
            <div style={{ display: "grid", gap: 6, marginTop: 8 }}>
              {friendConflicts.conflicts.slice(0, 12).map((conflict) => (
                <div key={conflict.id} className="rowBetween">
                  <span>{conflict.key}</span>
                  <span className="chip subtle">{conflict.kind}</span>
                </div>
              ))}
            </div>
            <div className="row" style={{ marginTop: 10, gap: 8, flexWrap: "wrap" }}>
              <button
                className="btn"
                disabled={resolvingConflicts}
                onClick={async () => {
                  setResolvingConflicts(true);
                  try {
                    const out = await resolveFriendLinkConflicts({
                      instanceId: instance.id,
                      resolution: { keep_all_mine: true },
                    });
                    setFriendConflicts(out.status === "conflicted" ? out : null);
                    onNotice(`Resolved conflicts with local preference. Status: ${out.status}.`);
                    await refresh({ quiet: true });
                  } catch (err: any) {
                    onError(err?.toString?.() ?? String(err));
                  } finally {
                    setResolvingConflicts(false);
                  }
                }}
              >
                Keep all mine
              </button>
              <button
                className="btn primary"
                disabled={resolvingConflicts}
                onClick={async () => {
                  setResolvingConflicts(true);
                  try {
                    const out = await resolveFriendLinkConflicts({
                      instanceId: instance.id,
                      resolution: { take_all_theirs: true },
                    });
                    setFriendConflicts(out.status === "conflicted" ? out : null);
                    onNotice(`Resolved conflicts with peer preference. Status: ${out.status}.`);
                    await refresh({ quiet: true });
                  } catch (err: any) {
                    onError(err?.toString?.() ?? String(err));
                  } finally {
                    setResolvingConflicts(false);
                  }
                }}
              >
                Take all theirs
              </button>
            </div>
          </div>
        ) : null}
      </div>
    </div>
  );
}
