import { useEffect, useMemo, useState } from "react";
import type {
  DriftReport,
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
  previewUpdateModpackFromInstance,
  reconcileFriendLink,
  resolveFriendLinkConflicts,
  realignInstanceToModpack,
  rollbackInstanceToLastModpackSnapshot,
} from "../tauri";

type Props = {
  instance: Instance;
  onNotice: (message: string) => void;
  onError: (message: string) => void;
  onFriendConflict?: (instanceId: string, result: FriendLinkReconcileResult) => void;
};

export default function InstanceModpackCard({ instance, onNotice, onError, onFriendConflict }: Props) {
  const [status, setStatus] = useState<InstanceModpackStatus | null>(null);
  const [drift, setDrift] = useState<DriftReport | null>(null);
  const [diff, setDiff] = useState<LayerDiffResult | null>(null);
  const [friendStatus, setFriendStatus] = useState<FriendLinkStatus | null>(null);
  const [inviteCode, setInviteCode] = useState("");
  const [invitePreview, setInvitePreview] = useState<string | null>(null);
  const [friendConflicts, setFriendConflicts] = useState<FriendLinkReconcileResult | null>(null);
  const [resolvingConflicts, setResolvingConflicts] = useState(false);
  const [busy, setBusy] = useState(false);

  async function refresh() {
    try {
      const [statusInfo, driftInfo, linked] = await Promise.all([
        getInstanceModpackStatus({ instanceId: instance.id }),
        detectInstanceModpackDrift({ instanceId: instance.id }).catch(() => null),
        getFriendLinkStatus({ instanceId: instance.id }).catch(() => null),
      ]);
      setStatus(statusInfo);
      setDrift(driftInfo);
      setFriendStatus(linked);
    } catch (err: any) {
      onError(err?.toString?.() ?? String(err));
    }
  }

  useEffect(() => {
    refresh().catch(() => null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [instance.id]);

  const linked = status?.link?.mode === "linked";
  const driftSummary = useMemo(() => {
    if (!drift) return "No drift data";
    return `${drift.added.length} added · ${drift.removed.length} removed · ${drift.version_changed.length} changed`;
  }, [drift]);

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
              disabled={busy}
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
              disabled={busy || !linked}
              onClick={async () => {
                setBusy(true);
                try {
                  const out = await realignInstanceToModpack({ instanceId: instance.id });
                  onNotice(out.message);
                  await refresh();
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
              disabled={busy || !status?.link?.last_lock_snapshot_id}
              onClick={async () => {
                setBusy(true);
                try {
                  const out = await rollbackInstanceToLastModpackSnapshot({ instanceId: instance.id });
                  onNotice(`${out.message} Restored ${out.restored_files} file(s).`);
                  await refresh();
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
              disabled={busy || !status.link.modpack_id}
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
              disabled={busy || !status.link.modpack_id || !diff}
              onClick={async () => {
                setBusy(true);
                try {
                  await applyUpdateModpackFromInstance({
                    instanceId: instance.id,
                    modpackId: status.link!.modpack_id,
                  });
                  onNotice("Applied instance diff to modpack overrides layer.");
                  setDiff(null);
                  await refresh();
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
        <div className="rowBetween">
          <div style={{ fontWeight: 900 }}>Friend Link</div>
          <span className="chip subtle">{friendStatus?.linked ? (friendStatus.status || "linked") : "unlinked"}</span>
        </div>
        {friendStatus?.linked ? (
          <>
            <div style={{ marginTop: 8 }} className="muted">
              Group: {friendStatus.group_id} · Peers: {(friendStatus.peers?.length ?? 0) + 1}
            </div>
            {friendStatus.peers?.length ? (
              <div style={{ marginTop: 6, display: "grid", gap: 6 }}>
                {friendStatus.peers.map((peer) => (
                  <div key={peer.peer_id} className="rowBetween">
                    <span>{peer.display_name}</span>
                    <span className="chip subtle">{peer.online ? "online" : "offline"}</span>
                  </div>
                ))}
              </div>
            ) : (
              <div className="muted" style={{ marginTop: 6 }}>
                No followers joined yet.
              </div>
            )}
            <div className="row" style={{ marginTop: 8, gap: 8, flexWrap: "wrap" }}>
              <button
                className="btn"
                disabled={busy}
                onClick={async () => {
                  setBusy(true);
                  try {
                    const invite = await createFriendLinkSession({ instanceId: instance.id });
                    setInvitePreview(invite.invite_code);
                    onNotice("Generated fresh invite code.");
                    await refresh();
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
                disabled={busy}
                onClick={async () => {
                  setBusy(true);
                  try {
                    const out = await reconcileFriendLink({ instanceId: instance.id, mode: "manual" });
                    if (out.status === "conflicted") {
                      setFriendConflicts(out);
                      onFriendConflict?.(instance.id, out);
                    }
                    onNotice(`Friend sync: ${out.status}. Applied ${out.actions_applied} changes.`);
                    await refresh();
                  } catch (err: any) {
                    onError(err?.toString?.() ?? String(err));
                  } finally {
                    setBusy(false);
                  }
                }}
              >
                Sync now
              </button>
              <button
                className="btn"
                disabled={busy}
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
                disabled={busy}
                onClick={async () => {
                  setBusy(true);
                  try {
                    await leaveFriendLinkSession({ instanceId: instance.id });
                    setInvitePreview(null);
                    setFriendConflicts(null);
                    onNotice("Left friend link session.");
                    await refresh();
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
                disabled={busy}
                onClick={async () => {
                  setBusy(true);
                  try {
                    const invite = await createFriendLinkSession({ instanceId: instance.id });
                    setInvitePreview(invite.invite_code);
                    onNotice("Friend link created. Share the invite code.");
                    await refresh();
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
                value={inviteCode}
                onChange={(e) => setInviteCode(e.target.value)}
              />
            </div>
            <div className="row" style={{ marginTop: 8, gap: 8 }}>
              <button
                className="btn primary"
                disabled={busy || !inviteCode.trim()}
                onClick={async () => {
                  setBusy(true);
                  try {
                    await joinFriendLinkSession({
                      instanceId: instance.id,
                      inviteCode: inviteCode.trim(),
                    });
                    setInviteCode("");
                    onNotice("Joined friend link.");
                    await refresh();
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
                    await refresh();
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
                    await refresh();
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
