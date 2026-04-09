import { useEffect } from "react";
import { Icon, LocalImage, Modal, RemoteImage } from "../../components/app-shell";
import { Dropdown, MenuSelect, MultiSelectDropdown, SegmentedControl } from "../../components/app-shell/controls";
import ModpackMaker from "../modpacks/ModpackMaker";
import ModpacksConfigEditor from "../modpacks/ModpacksConfigEditor";
import { formatBytes, formatCompact, formatDate, formatDateTime, humanizeToken, parseDateLike } from "../../app/utils/format";
import { getAppLanguageOption, type AppLanguage } from "../../lib/i18n";
import type { DiscoverContentType, Instance, LaunchMethod, LaunchPermissionChecklistItem } from "../../types";
import {
  ACCENT_OPTIONS,
  ACCENT_STRENGTH_OPTIONS,
  DENSITY_OPTIONS,
  DISCOVER_CONTENT_OPTIONS,
  DISCOVER_LOADER_GROUPS,
  DISCOVER_PROVIDER_SOURCES,
  DISCOVER_SORT_OPTIONS,
  DISCOVER_SOURCE_GROUPS,
  DISCOVER_VIEW_OPTIONS,
  LOG_MAX_LINES_OPTIONS,
  MOD_CATEGORY_GROUPS,
  MOTION_OPTIONS,
  MOTION_PROFILE_DETAILS,
  javaRuntimeDisplayLabel,
  githubInstallStateChipLabel,
  githubInstallSummary,
  githubResultInstallNote,
  githubResultInstallSupported,
  githubStatusChipClass,
  normalizeDiscoverProviderSources,
  normalizeDiscoverSource,
  openExternalLink,
  permissionStatusChipClass,
  permissionStatusLabel,
  providerSourceLabel,
  relativeTimeFromMs,
  updateAutoApplyModeLabel,
  updateCadenceLabel,
  type AccentStrength,
  type DensityPreset,
  type MotionPreset,
  type SettingsMode,
} from "../../app/routeSupport";

export type DiscoverRouteProps = {
  discoverAddContext: any;
  discoverAddTrayExpanded: any;
  discoverAddTrayItems: any;
  discoverAddTraySticky: any;
  discoverAllVersions: any;
  discoverBusy: any;
  discoverContentType: any;
  discoverErr: any;
  discoverSources: any;
  effectiveDiscoverSources: any;
  filterCategories: any;
  filterLoaders: any;
  filterVersion: any;
  groupedDiscoverVersions: any;
  hits: any;
  index: any;
  instances: any;
  limit: any;
  offset: any;
  openAddToModpack: any;
  openCurseforgeProject: any;
  openGithubProject: any;
  openInstall: any;
  openProject: any;
  page: any;
  pages: any;
  q: any;
  runSearch: any;
  selectedId: any;
  setDiscoverAddContext: any;
  setDiscoverAddTrayExpanded: any;
  setDiscoverAddTrayItems: any;
  setDiscoverAddTraySticky: any;
  setDiscoverAllVersions: any;
  setDiscoverContentType: any;
  setDiscoverErr: any;
  setDiscoverSources: any;
  setFilterCategories: any;
  setFilterLoaders: any;
  setFilterVersion: any;
  setIndex: any;
  setLimit: any;
  setModpacksStudioTab: any;
  setOffset: any;
  setQ: any;
  setRoute: any;
  totalHits: any;
};

export default function DiscoverRoute(props: DiscoverRouteProps) {
  const {
    discoverAddContext,
    discoverAddTrayExpanded,
    discoverAddTrayItems,
    discoverAddTraySticky,
    discoverAllVersions,
    discoverBusy,
    discoverContentType,
    discoverErr,
    discoverSources,
    effectiveDiscoverSources,
    filterCategories,
    filterLoaders,
    filterVersion,
    groupedDiscoverVersions,
    hits,
    index,
    instances,
    limit,
    offset,
    openAddToModpack,
    openCurseforgeProject,
    openGithubProject,
    openInstall,
    openProject,
    page,
    pages,
    q,
    runSearch,
    selectedId,
    setDiscoverAddContext,
    setDiscoverAddTrayExpanded,
    setDiscoverAddTrayItems,
    setDiscoverAddTraySticky,
    setDiscoverAllVersions,
    setDiscoverContentType,
    setDiscoverErr,
    setDiscoverSources,
    setFilterCategories,
    setFilterLoaders,
    setFilterVersion,
    setIndex,
    setLimit,
    setModpacksStudioTab,
    setOffset,
    setQ,
    setRoute,
    totalHits
  } = props;

  const selectedInst = instances.find((i) => i.id === selectedId) ?? null;
        const availableDiscoverSources =
          discoverContentType === "mods"
            ? [...DISCOVER_PROVIDER_SOURCES]
            : DISCOVER_PROVIDER_SOURCES.filter((source) => source !== "github");
        const discoverSourceGroups = DISCOVER_SOURCE_GROUPS
          .map((group) => ({
            ...group,
            items: group.items.filter((item) => availableDiscoverSources.includes(item.id as any)),
          }))
          .filter((group) => group.items.length > 0);
        const discoverSourceFilterValues = (() => {
          const normalized = normalizeDiscoverProviderSources(discoverSources).filter((source) =>
            availableDiscoverSources.includes(source)
          );
          return normalized.length > 0 ? normalized : [...availableDiscoverSources];
        })();
        const discoverSourceFilterActive =
          discoverSourceFilterValues.length < availableDiscoverSources.length;
        useEffect(() => {
          const nextSources = normalizeDiscoverProviderSources(discoverSources).filter((source) =>
            availableDiscoverSources.includes(source)
          );
          const resolved = nextSources.length > 0 ? nextSources : [...availableDiscoverSources];
          const current = Array.isArray(discoverSources) ? discoverSources : [];
          if (
            current.length === resolved.length &&
            resolved.every((source, index) => current[index] === source)
          ) {
            return;
          }
          setDiscoverSources(resolved);
        }, [availableDiscoverSources, discoverSources, setDiscoverSources]);
        const discoverIncludesGithub = effectiveDiscoverSources.includes("github");
        const discoverIncludesCurseforge = effectiveDiscoverSources.includes("curseforge");
        const discoverOnlyCurseforge =
          effectiveDiscoverSources.length === 1 && effectiveDiscoverSources[0] === "curseforge";
        const discoverFilterSupportNotes: string[] = [];
        if (discoverIncludesGithub) {
          if (discoverContentType !== "mods") {
            discoverFilterSupportNotes.push("GitHub source currently supports mods only.");
          } else if (
            filterLoaders.length > 0 ||
            Boolean(filterVersion) ||
            filterCategories.length > 0
          ) {
            discoverFilterSupportNotes.push(
              "GitHub source filters are best-effort: loader/version/category checks rely on repository topics and release asset naming."
            );
          }
        }
        if (discoverIncludesCurseforge) {
          if (discoverContentType === "mods" && discoverOnlyCurseforge) {
            discoverFilterSupportNotes.push(
              "CurseForge-only searches currently ignore the loader filter."
            );
          }
          if (filterCategories.length > 0) {
            discoverFilterSupportNotes.push(
              "CurseForge category matching is best-effort because provider category vocabularies differ."
            );
          }
        }
        if (
          effectiveDiscoverSources.length > 1 &&
          (filterLoaders.length > 0 || Boolean(filterVersion) || filterCategories.length > 0)
        ) {
          discoverFilterSupportNotes.push(
            "Multi-source search combines provider results; filter precision varies by provider."
          );
        }
        const discoverFilterSupportNotice = discoverFilterSupportNotes.length
          ? discoverFilterSupportNotes.join(" ")
          : null;
        const activeDiscoverFilterCount =
          (discoverSourceFilterActive ? 1 : 0) +
          (filterVersion ? 1 : 0) +
          (filterLoaders.length > 0 ? 1 : 0) +
          (filterCategories.length > 0 ? 1 : 0) +
          (discoverAllVersions ? 1 : 0);
        const discoverPlaceholder =
          discoverContentType === "shaderpacks"
            ? "Search shaderpacks…"
            : discoverContentType === "resourcepacks"
              ? "Search resourcepacks…"
              : discoverContentType === "datapacks"
              ? "Search datapacks…"
              : discoverContentType === "modpacks"
                ? "Search modpacks…"
                : "Search mods…";
        const discoverContentTypeLabel =
          DISCOVER_CONTENT_OPTIONS.find((option) => option.value === discoverContentType)?.label ?? discoverContentType;
  
        return (
          <div className="discoverPage" style={{ maxWidth: 1400 }}>
            <div className="pageRouteHeader">
              <div className="pageRouteEyebrow">Discover</div>
              <div className="h1">Discover content</div>
              <div className="p">Search Modrinth, CurseForge, or GitHub and install directly into instances.</div>
            </div>
            {discoverAddContext ? (
              <div className={`discoverAddTray${discoverAddTraySticky ? " discoverAddTraySticky" : ""}`}>
                <div className="discoverAddTrayHeader">
                  <div>
                    <div className="discoverAddTrayTitle">
                      Adding to {discoverAddContext.modpackName}
                      {discoverAddContext.layerName ? ` / ${discoverAddContext.layerName}` : ""}
                    </div>
                    <div className="discoverAddTraySub">
                      Use <strong>Add to modpack</strong> on any result. This tray tracks what you added in this session.
                    </div>
                  </div>
                  <div className="discoverAddTrayActions">
                    <button
                      className="btn"
                      onClick={() => {
                        setRoute("modpacks");
                        setModpacksStudioTab("creator");
                      }}
                    >
                      Open Creator Studio
                    </button>
                    <button
                      className="btn"
                      onClick={() => setDiscoverAddTrayExpanded((prev) => !prev)}
                      title="Show or hide added items."
                    >
                      {discoverAddTrayExpanded ? "Hide additions" : "Show additions"}
                    </button>
                    <button
                      className="btn"
                      onClick={() => setDiscoverAddTraySticky((prev) => !prev)}
                      title="Keep this tray pinned while you scroll results."
                    >
                      {discoverAddTraySticky ? "Unpin tray" : "Pin tray"}
                    </button>
                    <button
                      className="btn"
                      onClick={() => {
                        setDiscoverAddContext(null);
                        setDiscoverAddTrayItems([]);
                      }}
                    >
                      Clear add target
                    </button>
                  </div>
                </div>
  
                <div className="discoverAddTrayStats">
                  <span className="chip subtle">Added this session: {discoverAddTrayItems.length}</span>
                  <span className="chip subtle">Target layer: {discoverAddContext.layerName ?? "Default"}</span>
                  {discoverAddTrayItems[0] ? (
                    <span className="chip subtle">Last added: {formatDateTime(discoverAddTrayItems[0].addedAt, "just now")}</span>
                  ) : (
                    <span className="chip subtle">No items added yet</span>
                  )}
                </div>
  
                {discoverAddTrayExpanded ? (
                  <div className="discoverAddTrayList">
                    {discoverAddTrayItems.length === 0 ? (
                      <div className="discoverAddTrayEmpty">
                        Add content from Discover results and it will appear here.
                      </div>
                    ) : (
                      discoverAddTrayItems.slice(0, 8).map((item) => (
                        <div key={item.id} className="discoverAddTrayItem">
                          <div className="discoverAddTrayItemMain">
                            <div className="discoverAddTrayItemTitle">{item.title}</div>
                            <div className="discoverAddTrayItemMeta">
                              {item.projectId} · {item.source} · {item.contentType} · {item.layerName}
                            </div>
                          </div>
                          <span className="chip subtle">{formatDateTime(item.addedAt, "just now")}</span>
                        </div>
                      ))
                    )}
                  </div>
                ) : null}
  
                {discoverAddTrayItems.length > 8 ? (
                  <div className="discoverAddTrayOverflow muted">
                    Showing latest 8 of {discoverAddTrayItems.length} items.
                  </div>
                ) : null}
              </div>
            ) : null}
  
            <div className="discoverLayout">
              <div className="discoverMain">
                <div className="discoverWorkspace">
                  <div className="discoverWorkspaceTop">
                    <div>
                      <div className="discoverWorkspaceTitle">Search and install content</div>
                    </div>
                    <div className="discoverWorkspaceStats">
                      <span className="chip subtle">{totalHits} result{totalHits === 1 ? "" : "s"}</span>
                      <span className="chip subtle">{activeDiscoverFilterCount} active filter{activeDiscoverFilterCount === 1 ? "" : "s"}</span>
                    </div>
                  </div>

                  <div className="topRow discoverTypeRow" style={{ marginBottom: 8 }}>
                    <SegmentedControl
                      value={discoverContentType}
                      onChange={(v) => {
                        const nextType = (v as DiscoverContentType) ?? "mods";
                        const nextAvailableSources =
                          nextType === "mods"
                            ? [...DISCOVER_PROVIDER_SOURCES]
                            : DISCOVER_PROVIDER_SOURCES.filter((source) => source !== "github");
                        const nextSources = normalizeDiscoverProviderSources(discoverSources).filter((source) =>
                          nextAvailableSources.includes(source)
                        );
                        setDiscoverContentType(nextType);
                        setDiscoverSources(nextSources.length > 0 ? nextSources : [...nextAvailableSources]);
                        setFilterLoaders([]);
                        setOffset(0);
                      }}
                      options={DISCOVER_CONTENT_OPTIONS}
                      variant="scroll"
                    />
                  </div>

                  <div className="topRow discoverSearchRow">
                    <div className="searchGrow">
                      <input
                        className="input discoverSearchInput"
                        value={q}
                        onChange={(e) => {
                          setQ(e.target.value);
                          if (discoverErr) setDiscoverErr(null);
                        }}
                        placeholder={discoverPlaceholder}
                        onKeyDown={(e) => {
                          if (e.key === "Enter") runSearch(0);
                        }}
                      />
                    </div>

                    <div className="discoverSearchActions">
                      <MenuSelect
                        value={index}
                        labelPrefix="Sort"
                        options={DISCOVER_SORT_OPTIONS.map((o) => ({ value: o.value, label: o.label }))}
                        onChange={(v) => {
                          setIndex(v as any);
                          setOffset(0);
                        }}
                      />

                      <MenuSelect
                        value={String(limit)}
                        labelPrefix="View"
                        options={DISCOVER_VIEW_OPTIONS}
                        align="end"
                        onChange={(v) => {
                          setLimit(parseInt(v, 10));
                          setOffset(0);
                        }}
                      />

                      <button className="btn discoverSearchBtn" onClick={() => runSearch(0)} disabled={discoverBusy}>
                        {discoverBusy ? "Searching…" : "Search"}
                      </button>
                    </div>
                  </div>
                </div>

                {discoverErr ? <div className="errorBox">{discoverErr}</div> : null}

                <div className="resultsGrid">
              {hits.map((h) => (
                <div
                  className="resultCard"
                  key={`${h.source}:${h.project_id}`}
                  onClick={() => {
                      if (h.source === "modrinth") {
                        openProject(h.project_id, (h.content_type as DiscoverContentType) ?? discoverContentType);
                        return;
                      }
                      if (h.source === "curseforge") {
                        openCurseforgeProject(h.project_id, (h.content_type as DiscoverContentType) ?? discoverContentType);
                        return;
                      }
                      if (h.source === "github") {
                        void openGithubProject(h, (h.content_type as DiscoverContentType) ?? discoverContentType);
                        return;
                      }
                      if (h.external_url?.trim()) {
                        void openExternalLink(h.external_url.trim());
                      }
                  }}
                >
                  <div className="resultIcon">
                    <RemoteImage src={h.icon_url} alt={`${h.title} icon`} fallback={<div>⬚</div>} />
                  </div>
  
                  <div className="resultBody">
                    <div className="resultTitleRow">
                      <div className="resultTitle">{h.title}</div>
                    </div>
                    <div className="resultDesc">{h.description}</div>
                    <div className="resultMetaRow">
                      <span className="chip subtle discoverMetaChip">{providerSourceLabel(h.source)}</span>
                      <div className="resultMetaPrimary">
                        <span className="resultMetaText resultMetaStrong">by {h.author}</span>
                        <span className="resultMetaText resultMetaStrong"><span className="resultArrowGlyph">↓</span> {formatCompact(h.downloads)}</span>
                        <span className="resultMetaText resultMetaStrong"><span className="resultHeartGlyph">♥</span> {formatCompact(h.follows)}</span>
                      </div>
                      {h.source === "github" && githubInstallStateChipLabel(h.install_state) ? (
                        <span className={githubStatusChipClass("installability", h.install_state)}>
                          {githubInstallStateChipLabel(h.install_state)}
                        </span>
                      ) : null}
                      {h.categories?.slice(0, 3)?.map((c) => (
                        <span key={c} className="chip subtle discoverMetaChip">
                          {c}
                        </span>
                      ))}
                    </div>
                  </div>
  
                  <div
                    className="resultActions"
                    onClick={(e) => e.stopPropagation()}
                  >
                    <button
                      className="btn subtle discoverActionBtn discoverViewBtn"
                      onClick={() => {
                        if (h.source === "modrinth") {
                          openProject(h.project_id, (h.content_type as DiscoverContentType) ?? discoverContentType);
                          return;
                        }
                        if (h.source === "curseforge") {
                          openCurseforgeProject(h.project_id, (h.content_type as DiscoverContentType) ?? discoverContentType);
                          return;
                        }
                        if (h.source === "github") {
                          void openGithubProject(h, (h.content_type as DiscoverContentType) ?? discoverContentType);
                          return;
                        }
                        if (h.external_url?.trim()) {
                          void openExternalLink(h.external_url.trim());
                        }
                      }}
                    >
                      View
                    </button>
                    <button
                      className="btn discoverActionBtn discoverAddBtn"
                      onClick={() =>
                        openAddToModpack({
                          source: normalizeDiscoverSource(h.source),
                          projectId: h.project_id,
                          title: h.title,
                          contentType:
                            (h.content_type as DiscoverContentType) === "modpacks"
                              ? "modpacks"
                              : ((h.content_type as DiscoverContentType) ?? discoverContentType),
                          slug: h.slug ?? null,
                          iconUrl: h.icon_url,
                          description: h.description,
                        }, discoverAddContext ? { modpackId: discoverAddContext.modpackId, layerId: discoverAddContext.layerId ?? null } : undefined)
                      }
                      title={
                        h.content_type === "modpacks"
                          ? "Import modpacks as template layers from Creator Studio"
                          : "Add to a Modpack Maker layer"
                      }
                      disabled={h.content_type === "modpacks"}
                    >
                      Add to modpack
                    </button>
                    <button
                      className="btn installAction discoverActionBtn discoverInstallBtn"
                      onClick={() =>
                        openInstall({
                          source: normalizeDiscoverSource(h.source),
                          projectId: h.project_id,
                          title: h.title,
                          contentType:
                            (h.content_type as DiscoverContentType) === "modpacks"
                              ? "modpacks"
                              : ((h.content_type as DiscoverContentType) ?? discoverContentType),
                          iconUrl: h.icon_url,
                          description: h.description,
                          installSupported: githubResultInstallSupported(h),
                          installNote: githubResultInstallNote(h),
                        })
                      }
                      title={
                        h.content_type === "modpacks"
                          ? "Modpacks are imported as templates"
                          : !githubResultInstallSupported(h)
                            ? githubResultInstallNote(h) ?? "This provider result cannot be installed directly yet."
                            : "Install to instance"
                      }
                      disabled={h.content_type === "modpacks" || !githubResultInstallSupported(h)}
                    >
                      <span className="btnIcon">
                        <Icon name="download" />
                      </span>
                      {h.content_type === "modpacks" ? "Template only" : "Install"}
                    </button>
                  </div>
                </div>
              ))}
  
              {hits.length === 0 && !discoverBusy ? (
                <div className="card" style={{ padding: 16, borderRadius: 22, color: "var(--muted)" }}>
                  No results.
                </div>
              ) : null}
                </div>

                <div className="pager">
                  <button
                    className="btn"
                    onClick={() => runSearch(Math.max(0, offset - limit))}
                    disabled={discoverBusy || offset === 0}
                  >
                    Previous
                  </button>
                  <div className="pagerLabel">
                    Page {page} / {pages}
                  </div>
                  <button
                    className="btn"
                    onClick={() => runSearch(Math.min((pages - 1) * limit, offset + limit))}
                    disabled={discoverBusy || offset + limit >= totalHits}
                  >
                    Next
                  </button>
                </div>
              </div>

              <aside className="discoverSidebar">
                <div className="discoverSidebarSection">
                  <div className="discoverSidebarTitle">Refine</div>
                  <div className="discoverSidebarBody">
                    <div className="filterCtrl filterCtrlSource">
                      <MultiSelectDropdown
                        values={discoverSourceFilterActive ? discoverSourceFilterValues : []}
                        placeholder="Sources: All"
                        groups={discoverSourceGroups}
                        showSearch={false}
                        showGroupHeaders={false}
                        onChange={(values) => {
                          const next = normalizeDiscoverProviderSources(values).filter((source) =>
                            availableDiscoverSources.includes(source)
                          );
                          setDiscoverSources(next.length > 0 ? next : [...availableDiscoverSources]);
                          setOffset(0);
                        }}
                        onClear={() => {
                          setDiscoverSources([...availableDiscoverSources]);
                          setOffset(0);
                        }}
                      />
                    </div>

                    <div className="filterCtrl filterCtrlVersion">
                      <Dropdown
                        value={filterVersion}
                        placeholder="Game version: Any"
                        groups={groupedDiscoverVersions}
                        includeAny
                        onPick={(v) => {
                          setFilterVersion(v);
                          setOffset(0);
                        }}
                      />
                    </div>

                    <div className="filterCtrl filterCtrlLoader">
                      <MultiSelectDropdown
                        values={filterLoaders}
                        placeholder="Loaders: Any"
                        groups={DISCOVER_LOADER_GROUPS}
                        showSearch={false}
                        showGroupHeaders={false}
                        disabled={discoverContentType !== "mods" || discoverOnlyCurseforge}
                        onChange={(v) => {
                          if (discoverContentType !== "mods") return;
                          if (discoverOnlyCurseforge) return;
                          setFilterLoaders(v);
                          setOffset(0);
                        }}
                      />
                    </div>

                    <div className="filterCtrl filterCtrlCategory">
                      <MultiSelectDropdown
                        values={filterCategories}
                        placeholder="Categories: Any"
                        groups={MOD_CATEGORY_GROUPS}
                        searchPlaceholder="Search categories…"
                        onChange={(v) => {
                          setFilterCategories(v);
                          setOffset(0);
                        }}
                      />
                    </div>

                    <div className="discoverSidebarFooter">
                      <label className="checkboxRow discoverCheckboxRow">
                        <span
                          className={`checkbox ${discoverAllVersions ? "checked" : ""}`}
                          onClick={() => setDiscoverAllVersions(!discoverAllVersions)}
                          role="checkbox"
                          aria-checked={discoverAllVersions}
                          tabIndex={0}
                          onKeyDown={(e) => {
                            if (e.key === "Enter" || e.key === " ") {
                              e.preventDefault();
                              setDiscoverAllVersions(!discoverAllVersions);
                            }
                          }}
                        >
                          {discoverAllVersions ? "✓" : ""}
                        </span>
                        Show all versions
                      </label>

                      <button
                        className="btn subtle discoverClearBtn"
                        onClick={() => {
                          setDiscoverSources([...availableDiscoverSources]);
                          setFilterVersion(null);
                          setFilterLoaders([]);
                          setFilterCategories([]);
                          setDiscoverAllVersions(false);
                          setOffset(0);
                        }}
                        disabled={
                          !discoverSourceFilterActive &&
                          !filterVersion &&
                          filterLoaders.length === 0 &&
                          filterCategories.length === 0 &&
                          !discoverAllVersions
                        }
                      >
                        Clear filters
                      </button>
                    </div>
                  </div>
                </div>

                {discoverFilterSupportNotice ? (
                  <div className="discoverSidebarNotice warningBox">{discoverFilterSupportNotice}</div>
                ) : null}
              </aside>
            </div>
          </div>
        );
}
