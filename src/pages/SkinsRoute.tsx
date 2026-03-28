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

export type SkinsRouteProps = {
  accountAppearanceBusy: any;
  accountSkinThumbs: any;
  accountSkinViewerCanvasRef: any;
  accountSkinViewerStageRef: any;
  capeOptions: any;
  defaultSkinOptions: any;
  onAddCustomSkin: any;
  onApplySelectedAppearance: any;
  onCycleAccountCape: any;
  onPlaySkinViewerEmote: any;
  onRemoveSelectedCustomSkin: any;
  onRenameSelectedCustomSkin: any;
  savedSkinOptions: any;
  selectedAccountCape: any;
  selectedAccountSkin: any;
  selectedLauncherAccount: any;
  selectedLauncherAccountId: any;
  setSelectedAccountSkinId: any;
  setSkinPreviewEnabled: any;
  setSkinRenameDraft: any;
  skinPreviewEnabled: any;
  skinRenameDraft: any;
  skinViewerErr: any;
  skinViewerHintText: any;
  skinViewerPreparing: any;
  skinViewerShadowStyle: any;
  toLocalIconSrc: any;
};

export default function SkinsRoute(props: SkinsRouteProps) {
  const {
    accountAppearanceBusy,
    accountSkinThumbs,
    accountSkinViewerCanvasRef,
    accountSkinViewerStageRef,
    capeOptions,
    defaultSkinOptions,
    onAddCustomSkin,
    onApplySelectedAppearance,
    onCycleAccountCape,
    onPlaySkinViewerEmote,
    onRemoveSelectedCustomSkin,
    onRenameSelectedCustomSkin,
    savedSkinOptions,
    selectedAccountCape,
    selectedAccountSkin,
    selectedLauncherAccount,
    selectedLauncherAccountId,
    setSelectedAccountSkinId,
    setSkinPreviewEnabled,
    setSkinRenameDraft,
    skinPreviewEnabled,
    skinRenameDraft,
    skinViewerErr,
    skinViewerHintText,
    skinViewerPreparing,
    skinViewerShadowStyle,
    toLocalIconSrc
  } = props;

  return (
          <div className="page">
            <div style={{ maxWidth: 1360 }}>
              <div className="pageRouteHeader">
                <div className="pageRouteEyebrow">Appearance</div>
                <div className="h1">Skins</div>
                <div className="p">Manage skins and capes with live 3D preview.</div>
              </div>
  
              <div className="accountSkinsStudio accountSkinsStudioLibrary skinsRouteLayoutRef card">
                <div className="accountSkinViewerPane">
                  <div className="accountSkinTitleRow">
                    <div className="accountSkinHeading">Skins</div>
                    <span className="accountSkinBeta">Beta</span>
                  </div>
                  <div className="accountSkinSub">Interactive 3D preview. Drag to rotate your player.</div>
                  <div className="accountSkinNamePlate" title={selectedLauncherAccount?.username ?? "No account connected"}>
                    {selectedLauncherAccount?.username ?? "No account connected"}
                  </div>
                  <div
                    ref={accountSkinViewerStageRef}
                    className="accountSkinViewerStage"
                    style={skinViewerShadowStyle}
                  >
                    <canvas ref={accountSkinViewerCanvasRef} className="accountSkinViewerCanvas" />
                    <div className="accountSkinViewerShadow" />
                  </div>
                  {skinViewerErr ? <div className="errorBox">{skinViewerErr}</div> : null}
                  <div className="accountSkinViewerHint">{skinViewerHintText}</div>
                  {!skinPreviewEnabled ? (
                    <div className="row" style={{ marginTop: 8 }}>
                      <button className="btn" onClick={() => setSkinPreviewEnabled(true)}>
                        Enable 3D preview
                      </button>
                    </div>
                  ) : null}
                  <div className="accountSkinViewerActions">
                    <button
                      className="btn primary"
                      onClick={() => void onApplySelectedAppearance()}
                      disabled={accountAppearanceBusy || !selectedLauncherAccountId || !selectedAccountSkin}
                    >
                      {accountAppearanceBusy ? "Applying…" : "Apply skin & cape in-game"}
                    </button>
                    <button
                      className="btn"
                      onClick={onPlaySkinViewerEmote}
                      disabled={!skinPreviewEnabled || skinViewerPreparing}
                    >
                      Play emote
                    </button>
                    <button className="btn" onClick={onCycleAccountCape} disabled={capeOptions.length <= 1}>
                      Change cape
                    </button>
                    {selectedAccountSkin?.origin === "custom" ? (
                      <button className="btn danger" onClick={onRemoveSelectedCustomSkin}>
                        Remove skin
                      </button>
                    ) : null}
                  </div>
                  <div className="accountSkinViewerHint">
                    Cape: {selectedAccountCape?.label ?? "No cape"}
                  </div>
                </div>
  
                <div className="accountSkinLibraryPane skinsLibraryPane skinsLibraryRef">
                  <div className="skinsRefHeadRow">
                    <div className="skinsRefSectionTitle">Saved skins</div>
                    <div className="skinsLibraryStats">
                      <span className="chip subtle">{savedSkinOptions.length} saved</span>
                      <span className="chip subtle">{defaultSkinOptions.length} default</span>
                    </div>
                  </div>
  
                  <div className="skinsLibrarySelection skinsLibrarySelectionRef">
                    <span className="chip">Selected</span>
                    <strong>{selectedAccountSkin?.label ?? "None"}</strong>
                    <span className="skinsLibrarySelectionMeta">
                      {selectedAccountSkin?.origin === "custom"
                        ? "Custom"
                        : selectedAccountSkin?.origin === "profile"
                          ? "Profile"
                          : "Default"}
                    </span>
                    {selectedAccountSkin?.origin === "custom" ? (
                      <div className="skinsLibraryRenameRow">
                        <input
                          className="input skinsLibraryRenameInput"
                          value={skinRenameDraft}
                          onChange={(event) => setSkinRenameDraft(event.target.value)}
                          onKeyDown={(event) => {
                            if (event.key === "Enter") {
                              event.preventDefault();
                              onRenameSelectedCustomSkin();
                            }
                          }}
                          placeholder="Rename selected skin"
                        />
                        <button
                          className="btn"
                          onClick={onRenameSelectedCustomSkin}
                          disabled={
                            !skinRenameDraft.trim() ||
                            skinRenameDraft.trim() === (selectedAccountSkin.label ?? "").trim()
                          }
                        >
                          Rename
                        </button>
                      </div>
                    ) : null}
                  </div>
  
                  <div className="accountSkinCardGrid accountSkinCardGridSaved skinsRefSavedGrid">
                    <button className="accountSkinAddCard accountSkinAddCardRef" onClick={onAddCustomSkin}>
                      <span className="accountSkinAddPlus">+</span>
                      <span>Add a skin</span>
                    </button>
                    {savedSkinOptions.map((skin) => {
                      const active = selectedAccountSkin?.id === skin.id;
                      const thumbSet = accountSkinThumbs[skin.id];
                      const frontThumb =
                        thumbSet?.front ??
                        toLocalIconSrc(skin.preview_url) ??
                        toLocalIconSrc(skin.skin_url) ??
                        "";
                      const backThumb = thumbSet?.back ?? frontThumb;
                      return (
                        <button
                          key={skin.id}
                          className={`accountSkinChoiceCard skinChoiceSaved skinChoiceSavedRef ${active ? "active" : ""}`}
                          onClick={() => setSelectedAccountSkinId(skin.id)}
                        >
                          {active ? (
                            <span className="accountSkinSelectedCheck" aria-hidden="true">
                              <Icon name="check_circle" size={15} />
                            </span>
                          ) : null}
                          <div className="accountSkinChoiceThumb">
                            <div className="accountSkinChoiceThumbInner">
                              <div className="accountSkinChoiceFace accountSkinChoiceFaceFront">
                                {frontThumb ? (
                                  <img src={frontThumb} alt={`${skin.label} front preview`} />
                                ) : (
                                  <span>{skin.label.slice(0, 1).toUpperCase()}</span>
                                )}
                              </div>
                              <div className="accountSkinChoiceFace accountSkinChoiceFaceBack">
                                {backThumb ? (
                                  <img src={backThumb} alt={`${skin.label} back preview`} />
                                ) : (
                                  <span>{skin.label.slice(0, 1).toUpperCase()}</span>
                                )}
                              </div>
                            </div>
                          </div>
                          <div className="accountSkinChoiceLabel">{skin.label}</div>
                          <div className="accountSkinChoiceMeta">
                            {skin.origin === "custom" ? "Custom" : "Profile"}
                          </div>
                        </button>
                      );
                    })}
                  </div>
  
                  <div className="skinsRefSectionTitle skinsRefDefaultTitle">Default skins</div>
                  <div className="accountSkinCardGrid accountSkinCardGridDefault skinsRefDefaultGrid">
                    {defaultSkinOptions.map((skin) => {
                      const active = selectedAccountSkin?.id === skin.id;
                      const thumbSet = accountSkinThumbs[skin.id];
                      const frontThumb =
                        thumbSet?.front ??
                        toLocalIconSrc(skin.preview_url) ??
                        toLocalIconSrc(skin.skin_url) ??
                        "";
                      const backThumb = thumbSet?.back ?? frontThumb;
                      return (
                        <button
                          key={skin.id}
                          className={`accountSkinChoiceCard skinChoiceCompact skinChoiceCompactRef ${active ? "active" : ""}`}
                          onClick={() => setSelectedAccountSkinId(skin.id)}
                        >
                          {active ? (
                            <span className="accountSkinSelectedCheck" aria-hidden="true">
                              <Icon name="check_circle" size={15} />
                            </span>
                          ) : null}
                          <div className="accountSkinChoiceThumb">
                            <div className="accountSkinChoiceThumbInner">
                              <div className="accountSkinChoiceFace accountSkinChoiceFaceFront">
                                {frontThumb ? (
                                  <img src={frontThumb} alt={`${skin.label} front preview`} />
                                ) : (
                                  <span>{skin.label.slice(0, 1).toUpperCase()}</span>
                                )}
                              </div>
                              <div className="accountSkinChoiceFace accountSkinChoiceFaceBack">
                                {backThumb ? (
                                  <img src={backThumb} alt={`${skin.label} back preview`} />
                                ) : (
                                  <span>{skin.label.slice(0, 1).toUpperCase()}</span>
                                )}
                              </div>
                            </div>
                          </div>
                          <div className="accountSkinChoiceLabel">{skin.label}</div>
                          <div className="accountSkinChoiceMeta">Default</div>
                        </button>
                      );
                    })}
                  </div>
                </div>
              </div>
            </div>
          </div>
        );
}
