import { useEffect, useMemo, useState } from "react";
import type { DriftReport, Instance, InstanceModpackStatus, LayerDiffResult } from "../types";
import {
  applyUpdateModpackFromInstance,
  detectInstanceModpackDrift,
  getInstanceModpackStatus,
  previewUpdateModpackFromInstance,
  realignInstanceToModpack,
  rollbackInstanceToLastModpackSnapshot,
} from "../tauri";

type Props = {
  instance: Instance;
  onNotice: (message: string) => void;
  onError: (message: string) => void;
};

export default function InstanceModpackCard({ instance, onNotice, onError }: Props) {
  const [status, setStatus] = useState<InstanceModpackStatus | null>(null);
  const [drift, setDrift] = useState<DriftReport | null>(null);
  const [diff, setDiff] = useState<LayerDiffResult | null>(null);
  const [busy, setBusy] = useState(false);

  async function refresh() {
    try {
      const [statusInfo, driftInfo] = await Promise.all([
        getInstanceModpackStatus({ instanceId: instance.id }),
        detectInstanceModpackDrift({ instanceId: instance.id }).catch(() => null),
      ]);
      setStatus(statusInfo);
      setDrift(driftInfo);
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
    </div>
  );
}
