import Icon from "../components/app-shell/Icon";
import MenuSelect from "../components/app-shell/controls/MenuSelect";
import SegmentedControl from "../components/app-shell/controls/SegmentedControl";
import { LocalImage } from "../components/app-shell/AsyncImage";
import { formatBytes, formatDate, formatDateTime, parseDateLike } from "../app/utils/format";
import { launchStageBadgeLabel, relativeTimeFromMs, type LibraryGroupBy } from "../app/routeSupport";
import type { Instance } from "../types";

export type LibraryRouteProps = {
  instanceLastRunMetadataById: any;
  instanceModCountById: any;
  instances: any;
  launchBusyInstanceIds: any;
  launchCancelBusyInstanceId: any;
  launchStageByInstance: any;
  launcherBusy: any;
  libraryGroupBy: any;
  libraryQuery: any;
  libraryScope: any;
  librarySort: any;
  msLoginSessionId: any;
  onBeginMicrosoftLogin: any;
  onPlayInstance: any;
  onStopRunning: any;
  openInstance: any;
  openStorageManager: any;
  runningInstances: any;
  selectedId: any;
  selectedLauncherAccount: any;
  setLibraryContextMenu: any;
  setLibraryGroupBy: any;
  setLibraryQuery: any;
  setLibraryScope: any;
  setLibrarySort: any;
  setRoute: any;
  setShowCreate: any;
  storageOverview: any;
  storageOverviewBusy: any;
  storageOverviewError: any;
};

export default function LibraryRoute(props: LibraryRouteProps) {
  const {
    instanceLastRunMetadataById,
    instanceModCountById,
    instances,
    launchBusyInstanceIds,
    launchCancelBusyInstanceId,
    launchStageByInstance,
    launcherBusy,
    libraryGroupBy,
    libraryQuery,
    libraryScope,
    librarySort,
    msLoginSessionId,
    onBeginMicrosoftLogin,
    onPlayInstance,
    onStopRunning,
    openInstance,
    openStorageManager,
    runningInstances,
    selectedId,
    selectedLauncherAccount,
    setLibraryContextMenu,
    setLibraryGroupBy,
    setLibraryQuery,
    setLibraryScope,
    setLibrarySort,
    setRoute,
    setShowCreate,
    storageOverview,
    storageOverviewBusy,
    storageOverviewError,
  } = props;

  // Library (dashboard layout + custom context menu)
      const loaderLabelFor = (inst: Instance) =>
        inst.loader === "neoforge"
          ? "NeoForge"
          : inst.loader === "fabric"
            ? "Fabric"
            : inst.loader === "forge"
              ? "Forge"
              : inst.loader === "quilt"
                ? "Quilt"
                : "Vanilla";
  
  const normalizeInstanceOrigin = (inst: Instance) =>
    String(inst.origin ?? "custom").trim().toLowerCase() === "downloaded"
      ? "downloaded"
      : "custom";

  const visibleInstances = instances
    .filter((x) => x.name.toLowerCase().includes(libraryQuery.toLowerCase()))
    .filter((inst) => {
      if (libraryScope === "all") return true;
      return normalizeInstanceOrigin(inst) === libraryScope;
    });
  
      const filtered = [...visibleInstances].sort((a, b) => {
        if (librarySort === "name") {
          return a.name.localeCompare(b.name, undefined, { sensitivity: "base", numeric: true });
        }
        const bTs = parseDateLike(b.created_at)?.getTime() ?? 0;
        const aTs = parseDateLike(a.created_at)?.getTime() ?? 0;
        return bTs - aTs;
      });
  
      const grouped = (() => {
        if (libraryGroupBy === "none") {
          return [{ key: "all", label: "All instances", items: filtered }];
        }
        const map = new Map<string, Instance[]>();
        for (const inst of filtered) {
          const key = libraryGroupBy === "loader" ? loaderLabelFor(inst) : inst.mc_version;
          if (!map.has(key)) map.set(key, []);
          map.get(key)!.push(inst);
        }
        return Array.from(map.entries()).map(([key, items]) => ({
          key,
          label: key,
          items,
        }));
      })();
  
      const runningIds = new Set(runningInstances.map((run) => run.instance_id));
      const recentlyPlayed = [...instances]
        .map((inst) => {
          const lastLaunchAt = instanceLastRunMetadataById[inst.id]?.lastLaunchAt ?? inst.created_at;
          return { inst, lastLaunchAtMs: parseDateLike(lastLaunchAt)?.getTime() ?? 0 };
        })
        .sort((a, b) => b.lastLaunchAtMs - a.lastLaunchAtMs)
        .slice(0, 3);
      const knownModsTotal = Object.entries(instanceModCountById)
        .filter(([instanceId, count]) => instances.some((item) => item.id === instanceId) && Number.isFinite(count))
        .reduce((sum, [, count]) => sum + Math.max(0, Number(count ?? 0)), 0);
      const libraryStorageDisplay = storageOverview
        ? formatBytes(Number(storageOverview.total_bytes ?? 0))
        : storageOverviewError
          ? "Unavailable"
          : "Scanning…";
      const storageOverviewWarnings = storageOverview?.warnings ?? [];
      const needsLibraryGrowthPrompt = instances.length < 3;
      const totalRunningCount = runningInstances.length;
      const customInstancesCount = instances.filter((inst) => normalizeInstanceOrigin(inst) === "custom").length;
      const recentInstanceCreatedAt = filtered[0]?.created_at ?? null;
  
      return (
        <div className="page">
          <div className="libraryLayout">
            <section className="libraryMainPane">
              <div className="pageRouteHeader pageRouteHeaderSplit pageRouteHeaderProminent">
                <div className="pageRouteHeaderCopy">
                  <div className="pageRouteEyebrow">Instance library</div>
                  <div className="h1">Library</div>
                  <div className="p">
                    Open an instance to manage content, settings, worlds, and launch state.
                  </div>
                </div>
                <div className="pageRouteHeaderActions">
                  <button className="btn primary" onClick={() => setShowCreate(true)}>
                    <span className="btnIcon">
                      <Icon name="plus" size={18} className="navIcon plusIcon navAnimPlus" />
                    </span>
                    Create new instance
                  </button>
                </div>
              </div>
  
              <div className="card libraryHeroCard">
                <div className="libraryHeroStats">
                  <div className="libraryHeroStat">
                    <div className="libraryHeroStatLabel">Instances</div>
                    <div className="libraryHeroStatValue">{instances.length}</div>
                    <div className="libraryHeroStatMeta">
                      {customInstancesCount} custom instance{customInstancesCount === 1 ? "" : "s"}
                    </div>
                  </div>
                  <div className="libraryHeroStat">
                    <div className="libraryHeroStatLabel">Running</div>
                    <div className="libraryHeroStatValue">{totalRunningCount}</div>
                    <div className="libraryHeroStatMeta">
                      {totalRunningCount === 0 ? "Nothing active right now" : "Minecraft currently in progress"}
                    </div>
                  </div>
                  <div className="libraryHeroStat">
                    <div className="libraryHeroStatLabel">Known mods</div>
                    <div className="libraryHeroStatValue">{knownModsTotal.toLocaleString()}</div>
                    <div className="libraryHeroStatMeta">
                      Tracked across every visible instance
                    </div>
                  </div>
                  <div className="libraryHeroStat">
                    <div className="libraryHeroStatLabel">Newest</div>
                    <div className="libraryHeroStatValue">
                      {recentInstanceCreatedAt ? formatDate(recentInstanceCreatedAt) : "None"}
                    </div>
                    <div className="libraryHeroStatMeta">
                      Most recently created instance
                    </div>
                  </div>
                </div>
              </div>
  
              <>
                  {!selectedLauncherAccount ? (
                    <div className="libraryStatusBanner card">
                      <div className="libraryStatusTitle">Sign in to Microsoft</div>
                      <div className="libraryStatusText">
                        Connect your Minecraft account to launch with the native launcher.
                      </div>
                      <button className="btn primary" onClick={onBeginMicrosoftLogin} disabled={launcherBusy}>
                        {msLoginSessionId ? "Waiting for login..." : "Connect account"}
                      </button>
                    </div>
                  ) : null}
  
                  <div className="libraryTopRow">
                    <div className="libraryPrimaryControls">
                      <SegmentedControl
                        value={libraryScope}
                        onChange={(v) => setLibraryScope(v as any)}
                        options={[
                          { label: "All instances", value: "all" },
                          { label: "Downloaded", value: "downloaded" },
                          { label: "Custom", value: "custom" },
                        ]}
                      />
  
                      <div className="librarySearch">
                        <Icon name="search" size={18} />
                        <input
                          className="input"
                          placeholder="Search instances..."
                          value={libraryQuery}
                          onChange={(e) => setLibraryQuery(e.target.value)}
                        />
                        {libraryQuery && (
                          <button className="iconBtn" onClick={() => setLibraryQuery("")} aria-label="Clear">
                            <Icon name="x" size={18} />
                          </button>
                        )}
                      </div>
                    </div>
  
                    <div className="libraryRight">
                      <MenuSelect
                        value={librarySort}
                        labelPrefix="Sort"
                        onChange={(v) => setLibrarySort(v as "recent" | "name")}
                        options={[
                          { value: "recent", label: "Recently created" },
                          { value: "name", label: "Name" },
                        ]}
                        align="start"
                      />
                      <MenuSelect
                        value={libraryGroupBy}
                        labelPrefix="Group"
                        onChange={(v) => setLibraryGroupBy(v as LibraryGroupBy)}
                        options={[
                          { value: "none", label: "None" },
                          { value: "loader", label: "Loader" },
                          { value: "version", label: "Game version" },
                        ]}
                        align="start"
                      />
                    </div>
                  </div>
  
                  {filtered.length === 0 ? (
                <div className="card" style={{ marginTop: 12 }}>
                  <div className="emptyState">
                    <div className="emptyTitle">
                      {libraryScope === "downloaded" ? "No downloaded instances yet" : "No instances found"}
                    </div>
                    <div className="emptySub">
                      {libraryScope === "downloaded"
                        ? "Imported and installed modpacks will appear here once you add them."
                        : "Create an instance to start managing mods and versions."}
                    </div>
                    {libraryScope !== "downloaded" ? (
                      <div style={{ marginTop: 12 }}>
                        <button className="btn primary" onClick={() => setShowCreate(true)}>
                          <span className="btnIcon">
                            <Icon name="plus" size={18} className="navIcon plusIcon navAnimPlus" />
                          </span>
                          Create new instance
                        </button>
                      </div>
                    ) : null}
                  </div>
                </div>
                  ) : (
                <div className="libraryGroupList">
                  {grouped.map((group) => (
                    <section key={group.key} className="libraryGroupSection">
                      {libraryGroupBy !== "none" ? (
                        <div className="libraryGroupHeader">
                          <div>{group.label}</div>
                          <div className="chip subtle">{group.items.length}</div>
                        </div>
                      ) : null}
                      <div className="libraryGrid">
                        {group.items.map((inst) => {
                          const active = inst.id === selectedId;
                          const loaderLabel = loaderLabelFor(inst);
                          const isRunning = runningIds.has(inst.id);
                          const runningLaunch = runningInstances.find((run) => run.instance_id === inst.id) ?? null;
                          const launchStage = launchStageByInstance[inst.id] ?? null;
                          const launchStageLabel = launchStage?.label?.trim() || launchStageBadgeLabel(
                            launchStage?.status,
                            launchStage?.message
                          );
                          const instanceModCount = Number(instanceModCountById[inst.id] ?? 0);
                          const createdLabel = formatDate(inst.created_at);
                          return (
                            <article
                              key={inst.id}
                              className={`instCard ${active ? "active" : ""} ${isRunning ? "running" : ""}`}
                              onClick={() => openInstance(inst.id)}
                              onContextMenu={(event) => {
                                event.preventDefault();
                                event.stopPropagation();
                                setLibraryContextMenu({
                                  instanceId: inst.id,
                                  x: event.clientX,
                                  y: event.clientY,
                                });
                              }}
                            >
                              <div className="instCardHead">
                                <div className="instCardIcon">
                                  {inst.icon_path ? (
                                    <LocalImage path={inst.icon_path} alt="" fallback={<Icon name="box" size={19} />} />
                                  ) : (
                                    <Icon name="box" size={19} />
                                  )}
                                </div>
                                <div className="instCardHeadText">
                                  <div className="instCardTitle">{inst.name}</div>
                                  <div className="instCardSub">
                                    {loaderLabel} · Minecraft {inst.mc_version}
                                  </div>
                                </div>
                              </div>
  
                              <div className="instCardMeta">
                                <span className="chip">{loaderLabel}</span>
                                <span className="chip">{inst.mc_version}</span>
                                <span className="chip subtle">{instanceModCount} mod{instanceModCount === 1 ? "" : "s"}</span>
                                <span className="chip subtle">Created {createdLabel}</span>
                                {isRunning ? <span className="chip">Running</span> : null}
                                {!isRunning && launchStageLabel ? (
                                  <span className="chip">{launchStage?.status === "starting" ? `Launching: ${launchStageLabel}` : launchStageLabel}</span>
                                ) : null}
                              </div>
  
                              <div className="instCardActions" onClick={(event) => event.stopPropagation()}>
                                {runningLaunch ? (
                                  <button className="btn" onClick={() => onStopRunning(runningLaunch.launch_id)}>
                                    Stop
                                  </button>
                                ) : (
                                  <button
                                    className={`btn ${launchBusyInstanceIds.includes(inst.id) ? "danger" : "primary"}`}
                                    onClick={() => onPlayInstance(inst)}
                                    disabled={launchCancelBusyInstanceId === inst.id}
                                  >
                                    <Icon name={launchBusyInstanceIds.includes(inst.id) ? "x" : "play"} size={16} />
                                    {launchBusyInstanceIds.includes(inst.id)
                                      ? (launchCancelBusyInstanceId === inst.id ? "Cancelling…" : "Cancel launch")
                                      : "Play"}
                                  </button>
                                )}
                                <button className="btn" onClick={() => openInstance(inst.id)}>
                                  View instance
                                </button>
                              </div>
                            </article>
                          );
                        })}
                      </div>
                    </section>
                  ))}
                </div>
                  )}
                </>
            </section>
  
            <aside className="librarySidePane">
              <div className="card librarySideCard">
                <div className="librarySideTitle">Instances running</div>
                <div className="libraryRunCount">{runningInstances.length}</div>
                {runningInstances.length === 0 ? (
                  <div className="compactEmptyState">
                    <span className="compactEmptyIcon" aria-hidden="true">
                      <Icon name="play" size={15} />
                    </span>
                    <div className="compactEmptyBody">
                      <div className="compactEmptyTitle">Nothing running right now</div>
                      <div className="compactEmptyText">Hit Play on any instance to launch Minecraft.</div>
                    </div>
                  </div>
                ) : (
                  <div className="libraryRunList">
                    {runningInstances.slice(0, 5).map((run) => (
                      <div key={run.launch_id} className="libraryRunRow">
                        <span>{run.instance_name}</span>
                        <div style={{ display: "flex", gap: 6, flexWrap: "wrap", justifyContent: "flex-end" }}>
                          <span className="chip subtle">{run.method}</span>
                          {run.isolated ? <span className="chip subtle">Disposable</span> : null}
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>
  
              <div className="card librarySideCard">
                <div className="librarySideTitle">Recently played</div>
                {recentlyPlayed.length === 0 ? (
                  <div className="muted">No launch history yet.</div>
                ) : (
                  <div className="libraryRecentList">
                    {recentlyPlayed.map((row) => (
                      <button
                        key={row.inst.id}
                        className="libraryRecentRow"
                        onClick={() => openInstance(row.inst.id)}
                        title={formatDateTime(row.inst.created_at, "Unknown time")}
                      >
                        <span className="libraryRecentName">{row.inst.name}</span>
                        <span className="libraryRecentMeta">{relativeTimeFromMs(row.lastLaunchAtMs)}</span>
                      </button>
                    ))}
                  </div>
                )}
              </div>
  
              <button
                className="card librarySideCard libraryStorageCard"
                onClick={() => openStorageManager("overview")}
                type="button"
              >
                <div className="libraryStorageCardMain">
                  <div className="librarySideTitle">Storage usage</div>
                  <div className="libraryStorageStat">{libraryStorageDisplay}</div>
                  <div className="muted">
                    {storageOverviewError
                      ? "Storage scan failed. Open the manager for details."
                      : storageOverviewBusy && !storageOverview
                      ? "Scanning launcher + instance storage…"
                      : `${knownModsTotal.toLocaleString()} total mods across ${instances.length} instance${instances.length === 1 ? "" : "s"}.`}
                  </div>
                </div>
                <div className="libraryStorageCardMeta">
                  {storageOverview ? (
                    <>
                      <span className="chip subtle">Reclaimable {formatBytes(storageOverview.reclaimable_bytes)}</span>
                      {storageOverviewWarnings.length > 0 ? (
                        <span className="chip subtle">{storageOverviewWarnings.length} warning{storageOverviewWarnings.length === 1 ? "" : "s"}</span>
                      ) : null}
                    </>
                  ) : (
                    <span className="chip subtle">{storageOverviewBusy ? "Scanning…" : "Open manager"}</span>
                  )}
                </div>
              </button>
  
              {needsLibraryGrowthPrompt ? (
                <div className="card librarySideCard libraryPromptCard">
                  <div className="librarySideTitle">Create your next instance</div>
                  <div className="muted">
                    Try a Vanilla profile, a Fabric performance build, or a Creator Studio test instance.
                  </div>
                  <button className="btn primary" onClick={() => setShowCreate(true)}>
                    <Icon name="plus" size={16} />
                    Create instance
                  </button>
                </div>
              ) : null}
  
              <div className="card librarySideCard">
                <div className="librarySideTitle">Playing as</div>
                {selectedLauncherAccount ? (
                  <>
                    <div className="libraryAccountName">{selectedLauncherAccount.username}</div>
                    <div className="libraryAccountId muted">{selectedLauncherAccount.id}</div>
                    <div className="row" style={{ marginTop: 10 }}>
                      <button className="btn" onClick={() => setRoute("account")}>
                        Account page
                      </button>
                    </div>
                  </>
                ) : (
                  <>
                    <div className="muted">No Minecraft account connected.</div>
                    <div className="row" style={{ marginTop: 10 }}>
                      <button className="btn primary" onClick={onBeginMicrosoftLogin} disabled={launcherBusy}>
                        {msLoginSessionId ? "Waiting..." : "Sign in"}
                      </button>
                    </div>
                  </>
                )}
              </div>
  
              <div className="card librarySideCard">
                <div className="librarySideTitle">Quick actions</div>
                <div className="libraryQuickActions">
                  <button className="btn" onClick={() => setRoute("discover")}>Discover mods</button>
                  <button className="btn primary" onClick={() => setShowCreate(true)}>Create instance</button>
                </div>
              </div>
            </aside>
          </div>
        </div>
      );
}
