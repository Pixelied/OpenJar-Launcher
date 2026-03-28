import Icon from "../components/app-shell/Icon";
import Modal from "../components/app-shell/Modal";
import Dropdown from "../components/app-shell/controls/Dropdown";
import MenuSelect from "../components/app-shell/controls/MenuSelect";
import MultiSelectDropdown from "../components/app-shell/controls/MultiSelectDropdown";
import SegmentedControl from "../components/app-shell/controls/SegmentedControl";
import ModpackMaker from "./ModpackMaker";
import ModpacksConfigEditor from "./ModpacksConfigEditor";
import { LocalImage, RemoteImage } from "../components/app-shell/AsyncImage";
import { formatBytes, formatCompact, formatDate, formatDateTime, humanizeToken, parseDateLike } from "../app/utils/format";
import { getAppLanguageOption, type AppLanguage } from "../lib/i18n";
import type { DiscoverContentType, Instance, LaunchMethod, LaunchPermissionChecklistItem } from "../types";
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
} from "../app/routeSupport";

export type ModpacksRouteProps = {
  instances: any;
  isDevMode: any;
  launcherSettings: any;
  modpacksStudioTab: any;
  runningInstances: any;
  selectedId: any;
  setDiscoverAddContext: any;
  setDiscoverAddTrayExpanded: any;
  setError: any;
  setInstallNotice: any;
  setModpacksStudioTab: any;
  setRoute: any;
  setSelectedId: any;
};

export default function ModpacksRoute(props: ModpacksRouteProps) {
  const {
    instances,
    isDevMode,
    launcherSettings,
    modpacksStudioTab,
    runningInstances,
    selectedId,
    setDiscoverAddContext,
    setDiscoverAddTrayExpanded,
    setError,
    setInstallNotice,
    setModpacksStudioTab,
    setRoute,
    setSelectedId
  } = props;

  return (
          <div className="creatorStudioRoute" style={{ maxWidth: 1380 }}>
            <div className="pageRouteHeader pageRouteHeaderSplit pageRouteHeaderProminent">
              <div className="pageRouteHeaderCopy">
                <div className="pageRouteEyebrow">Creator Studio</div>
                <div className="h1">Creator Studio</div>
                <div className="p">
                  Build layered packs and edit live config without leaving the workspace.
                </div>
              </div>
              <div className="pageRouteHeaderActions creatorStudioHeaderActions">
                <SegmentedControl
                  value={modpacksStudioTab === "config" ? "config" : "creator"}
                  onChange={(v) => setModpacksStudioTab((v as any) ?? "creator")}
                  options={[
                    { value: "creator", label: "Creator" },
                    { value: "config", label: "Config Editor" },
                  ]}
                  variant="scroll"
                />
              </div>
            </div>
  
            {modpacksStudioTab === "config" ? (
              <div className="creatorStudioPanelWrap">
                <ModpacksConfigEditor
                  instances={instances}
                  selectedInstanceId={selectedId}
                  onSelectInstance={setSelectedId}
                  onManageInstances={() => setRoute("library")}
                  runningInstanceIds={runningInstances.map((run) => run.instance_id)}
                />
              </div>
            ) : (
              <div className="creatorStudioPanelWrap">
                <ModpackMaker
                  instances={instances}
                  selectedInstanceId={selectedId}
                  autoIdentifyLocalJarsEnabled={Boolean(launcherSettings?.auto_identify_local_jars)}
                  onSelectInstance={setSelectedId}
                  onOpenDiscover={(context) => {
                    setDiscoverAddContext(context ?? null);
                    setDiscoverAddTrayExpanded(true);
                    setRoute("discover");
                  }}
                  isDevMode={isDevMode}
                  onNotice={(message) => setInstallNotice(message)}
                  onError={(message) => setError(message)}
                />
              </div>
            )}
          </div>
        );
}
