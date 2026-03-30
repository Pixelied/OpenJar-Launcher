import { useEffect, useRef, useState } from "react";
import Icon from "../components/app-shell/Icon";
import Modal from "../components/app-shell/Modal";
import Dropdown from "../components/app-shell/controls/Dropdown";
import MenuSelect from "../components/app-shell/controls/MenuSelect";
import MultiSelectDropdown from "../components/app-shell/controls/MultiSelectDropdown";
import SegmentedControl from "../components/app-shell/controls/SegmentedControl";
import ModpackMaker from "./ModpackMaker";
import ModpacksConfigEditor from "./ModpacksConfigEditor";
import { RemoteImage } from "../components/app-shell/AsyncImage";
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
  skinViewerComboPulseKey: any;
  skinRenameDraft: any;
  skinViewerErr: any;
  skinViewerHitCombo: any;
  skinViewerHintText: any;
  skinViewerMineBurstKey: any;
  skinViewerMiningActive: any;
  skinViewerMiningBreakStage: any;
  skinViewerPageBroken: any;
  skinViewerPreparing: any;
  skinViewerShadowStyle: any;
  toLocalIconSrc: any;
};

const MINE_CRACK_SEGMENTS: Array<[number, number, number, number]> = [
  [28, 2, 3, 12],
  [28, 12, 2, 10],
  [18, 18, 12, 2],
  [10, 18, 8, 2],
  [28, 20, 2, 8],
  [30, 26, 10, 2],
  [38, 26, 2, 8],
  [40, 32, 8, 2],
  [46, 32, 2, 10],
  [44, 42, 2, 8],
  [36, 48, 10, 2],
  [20, 48, 16, 2],
  [18, 40, 2, 10],
  [12, 34, 8, 2],
  [12, 24, 2, 10],
  [6, 20, 8, 2],
  [4, 20, 2, 8],
  [4, 28, 8, 2],
  [16, 6, 8, 2],
  [40, 10, 10, 2],
  [48, 12, 2, 10],
  [50, 22, 8, 2],
  [54, 24, 2, 10],
  [50, 34, 8, 2],
  [22, 28, 2, 8],
  [24, 34, 10, 2],
  [32, 36, 2, 10],
  [24, 56, 12, 2],
];

export const SKINS_PAGE_RUIN_FRAGMENTS = [
  { left: "4%", top: "16%", width: 180, height: 18, rotate: -11, className: "strip" },
  { left: "27%", top: "12%", width: 36, height: 210, rotate: 1, className: "pillar" },
  { left: "59%", top: "22%", width: 160, height: 22, rotate: 0, className: "strip" },
  { left: "72%", top: "28%", width: 22, height: 250, rotate: 0, className: "pillar" },
  { left: "8%", top: "53%", width: 120, height: 20, rotate: 7, className: "strip" },
  { left: "18%", top: "44%", width: 280, height: 22, rotate: 0, className: "beam" },
  { left: "42%", top: "71%", width: 148, height: 20, rotate: 0, className: "strip" },
  { left: "81%", top: "66%", width: 18, height: 170, rotate: -2, className: "pillar" },
  { left: "12%", top: "76%", width: 20, height: 150, rotate: 0, className: "pillar" },
  { left: "63%", top: "84%", width: 110, height: 18, rotate: -7, className: "strip" },
] as const;

const COMBO_STAGES = [
  { threshold: 2, label: "combo!" },
  { threshold: 4, label: "chain!" },
  { threshold: 7, label: "rush!" },
  { threshold: 11, label: "frenzy!" },
  { threshold: 16, label: "berserk!" },
  { threshold: 24, label: "overdrive!" },
  { threshold: 35, label: "cracked!" },
  { threshold: 50, label: "rift!" },
  { threshold: 70, label: "corrupt!" },
  { threshold: 100, label: "broken!" },
] as const;

export function MinecraftBreakOverlay({ stage, className = "" }: { stage: number; className?: string }) {
  if (stage <= 0) return null;
  const visible = Math.max(1, Math.ceil((Math.min(stage, 9) / 9) * MINE_CRACK_SEGMENTS.length));
  return (
    <svg
      className={className}
      viewBox="0 0 64 64"
      aria-hidden="true"
      preserveAspectRatio="none"
      shapeRendering="crispEdges"
    >
      <rect x="0" y="0" width="64" height="64" fill={`rgba(20, 20, 20, ${0.03 + Math.min(stage, 9) * 0.012})`} />
      {MINE_CRACK_SEGMENTS.slice(0, visible).map(([x, y, w, h], idx) => (
        <g key={`${x}-${y}-${idx}`}>
          <rect x={x} y={y} width={w} height={h} fill="rgba(10,10,10,0.8)" />
          <rect x={x + 1} y={y + 1} width={Math.max(1, w - 1)} height={Math.max(1, h - 1)} fill="rgba(242,242,242,0.7)" />
          <rect x={x} y={y} width={Math.max(1, w - 1)} height="1" fill="rgba(255,255,255,0.5)" />
        </g>
      ))}
    </svg>
  );
}

function SkinThumbFallback({
  label,
  tone = "default",
}: {
  label: string;
  tone?: "default" | "missing";
}) {
  return (
    <div className={`accountSkinThumbFallback ${tone === "missing" ? "is-missing" : ""}`}>
      <span className="accountSkinThumbFallbackGlyph" aria-hidden="true">
        {label.slice(0, 1).toUpperCase()}
      </span>
      <span className="accountSkinThumbFallbackText">
        {tone === "missing" ? "Preview blocked" : "Preview unavailable"}
      </span>
    </div>
  );
}

function SkinThumbFace({
  src,
  alt,
  label,
  tone = "default",
}: {
  src?: string | null;
  alt: string;
  label: string;
  tone?: "default" | "missing";
}) {
  return <RemoteImage src={src} alt={alt} fallback={<SkinThumbFallback label={label} tone={tone} />} />;
}

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
    skinViewerComboPulseKey,
    setSkinRenameDraft,
    skinPreviewEnabled,
    skinRenameDraft,
    skinViewerHitCombo,
    skinViewerErr,
    skinViewerHintText,
    skinViewerMineBurstKey,
    skinViewerMiningActive,
    skinViewerMiningBreakStage,
    skinViewerPageBroken,
    skinViewerPreparing,
    skinViewerShadowStyle,
    toLocalIconSrc
  } = props;
  const comboExitTimeoutRef = useRef<number | null>(null);
  const [comboHudDisplayCount, setComboHudDisplayCount] = useState(0);
  const [comboHudVisible, setComboHudVisible] = useState(false);
  const [comboHudExiting, setComboHudExiting] = useState(false);

  useEffect(() => {
    return () => {
      if (comboExitTimeoutRef.current != null) {
        window.clearTimeout(comboExitTimeoutRef.current);
      }
    };
  }, []);

  useEffect(() => {
    if (skinViewerHitCombo > 1) {
      if (comboExitTimeoutRef.current != null) {
        window.clearTimeout(comboExitTimeoutRef.current);
        comboExitTimeoutRef.current = null;
      }
      setComboHudDisplayCount(skinViewerHitCombo);
      setComboHudVisible(true);
      setComboHudExiting(false);
      return;
    }
    if (!comboHudVisible || comboHudExiting || comboHudDisplayCount <= 1) {
      return;
    }
    setComboHudExiting(true);
    comboExitTimeoutRef.current = window.setTimeout(() => {
      setComboHudVisible(false);
      setComboHudExiting(false);
      setComboHudDisplayCount(0);
      comboExitTimeoutRef.current = null;
    }, 860);
  }, [comboHudDisplayCount, comboHudExiting, comboHudVisible, skinViewerHitCombo]);

  const comboDisplayCount = skinViewerHitCombo > 1 ? skinViewerHitCombo : comboHudDisplayCount;
  const comboTier = COMBO_STAGES.reduce(
    (tier, stage, index) => (comboDisplayCount >= stage.threshold ? index + 1 : tier),
    0
  );
  const comboLabel = comboTier > 0 ? COMBO_STAGES[comboTier - 1]?.label ?? "combo!" : "combo!";
  const comboFloor = comboTier > 0 ? COMBO_STAGES[comboTier - 1]?.threshold ?? 2 : 2;
  const comboCeiling = COMBO_STAGES[comboTier]?.threshold ?? comboFloor;
  const comboProgress =
    comboTier >= COMBO_STAGES.length
      ? 1
      : Math.min(1, Math.max(0, (comboDisplayCount - comboFloor) / Math.max(1, comboCeiling - comboFloor)));
  const comboTargetLabel = comboTier >= COMBO_STAGES.length ? "MAX" : `${comboCeiling}x`;
  const comboBurstMode =
    comboTier >= 9
      ? "cataclysm"
      : comboTier >= 7
        ? "corrupt"
        : comboTier >= 5
          ? "shock"
          : comboTier >= 3
            ? "slash"
            : "pixel";
  const comboHudStyle = {
    ["--combo-intensity" as any]: String(Math.min(1, Math.max(0, (comboDisplayCount - 1) / 40))),
    ["--combo-pop-scale" as any]: String(1 + Math.min(0.18, comboDisplayCount * 0.0025)),
    ["--combo-shadow-boost" as any]: String(Math.min(1, comboDisplayCount / 34)),
    ["--combo-chaos" as any]: String(Math.min(1, Math.max(0, (comboDisplayCount - 2) / 28))),
    ["--combo-overflow" as any]: String(Math.min(1, Math.max(0, (comboDisplayCount - 100) / 60))),
    ["--combo-progress" as any]: String(comboProgress),
    ["--combo-tier" as any]: String(comboTier),
  };

  return (
          <div className={`page skinsPageRoot${skinViewerPageBroken ? " is-broken" : ""}`}>
            <div className="pageRouteStack" style={{ maxWidth: 1360 }}>
              <div className="pageRouteHeader">
                <div className="pageRouteEyebrow">Appearance</div>
                <div className="h1">Skins</div>
                <div className="p">Manage skins and capes with live 3D preview.</div>
              </div>
  
              <div className={`accountSkinsStudio accountSkinsStudioLibrary skinsRouteLayoutRef card${skinViewerPageBroken ? " is-broken" : ""}`}>
                <div className="accountSkinViewerPane">
                  <div className="accountSkinTitleRow">
                    <div className="accountSkinHeading">3D Skin Preview</div>
                  </div>
                  <div className="accountSkinSub">3D skin preview. Drag to rotate your player.</div>
                  <div className="accountSkinNamePlate" title={selectedLauncherAccount?.username ?? "No account connected"}>
                    {selectedLauncherAccount?.username ?? "No account connected"}
                  </div>
                  <div
                    ref={accountSkinViewerStageRef}
                    className="accountSkinViewerStage"
                    style={skinViewerShadowStyle}
                  >
                    <canvas ref={accountSkinViewerCanvasRef} className="accountSkinViewerCanvas" />
                    {skinViewerErr ? (
                      <div className="accountSkinViewerFallback" role="status">
                        <div className="accountSkinViewerFallbackBadge">Preview unavailable</div>
                        <div className="accountSkinViewerFallbackTitle">Skin preview could not load</div>
                        <div className="accountSkinViewerFallbackText">
                          This skin may be blocked by the network or missing from the source. You can still keep browsing and apply another saved skin.
                        </div>
                      </div>
                    ) : null}
                    <div className="accountSkinViewerShadow" />
                    {comboHudVisible ? (
                      <div
                        className={`accountSkinComboHud combo-tier-${comboTier}${comboHudExiting ? " combo-exiting" : ""}`}
                        style={comboHudStyle}
                      >
                        <div key={skinViewerComboPulseKey} className={`accountSkinComboHitFx burst-${comboBurstMode}`}>
                          <span className="accountSkinComboPulseBurst" />
                          <span className="accountSkinComboBurstStar star-a" />
                          <span className="accountSkinComboBurstStar star-b" />
                          <span className="accountSkinComboBurstStar star-c" />
                          <span className="accountSkinComboBurstShard shard-a" />
                          <span className="accountSkinComboBurstShard shard-b" />
                          <span className="accountSkinComboBurstShard shard-c" />
                          <span className="accountSkinComboBurstShard shard-d" />
                          <span className="accountSkinComboBurstPixel pixel-a" />
                          <span className="accountSkinComboBurstPixel pixel-b" />
                          <span className="accountSkinComboBurstPixel pixel-c" />
                          <span className="accountSkinComboBurstPixel pixel-d" />
                          <span className="accountSkinComboBurstPixel pixel-e" />
                          <span className="accountSkinComboBurstPixel pixel-f" />
                          <span className="accountSkinComboBurstSmoke smoke-a" />
                          <span className="accountSkinComboBurstSmoke smoke-b" />
                          <span className="accountSkinComboBurstWave wave-a" />
                          <span className="accountSkinComboBurstWave wave-b" />
                          <span className="accountSkinComboBurstCore" />
                          <span className="accountSkinComboBurstBeam beam-a" />
                          <span className="accountSkinComboBurstBeam beam-b" />
                          <span className="accountSkinComboBurstBeam beam-c" />
                          <span className="accountSkinComboBurstEdge edge-a" />
                          <span className="accountSkinComboBurstEdge edge-b" />
                          <span className="accountSkinComboBurstEdge edge-c" />
                          <span className="accountSkinComboBurstEdge edge-d" />
                          <span className="accountSkinComboBurstRune rune-a" />
                          <span className="accountSkinComboBurstRune rune-b" />
                          <span className="accountSkinComboBurstGlitch glitch-a" />
                          <span className="accountSkinComboBurstGlitch glitch-b" />
                        </div>
                        <span className="accountSkinComboSpark spark-a" />
                        <span className="accountSkinComboSpark spark-b" />
                        <span className="accountSkinComboSpark spark-c" />
                        <span className="accountSkinComboSpark spark-d" />
                        <span className="accountSkinComboSpark spark-e" />
                        <span className="accountSkinComboSpark spark-f" />
                        <span className="accountSkinComboAura aura-a" />
                        <span className="accountSkinComboAura aura-b" />
                        <span className="accountSkinComboAura aura-c" />
                        <span className="accountSkinComboScanline" />
                        <span className="accountSkinComboEcho echo-a" />
                        <span className="accountSkinComboEcho echo-b" />
                        <span className="accountSkinComboStatic" />
                        <span className="accountSkinComboSlice slice-a" />
                        <span className="accountSkinComboSlice slice-b" />
                        <span className="accountSkinComboSlice slice-c" />
                        <span className="accountSkinComboSlice slice-d" />
                        <span className="accountSkinComboText">
                          <span className="accountSkinComboCount">{comboDisplayCount}x</span>
                          <span className="accountSkinComboLabel">{comboLabel}</span>
                        </span>
                        <span className="accountSkinComboMeter" aria-hidden="true">
                          <span className="accountSkinComboMeterFill" />
                        </span>
                        <span className="accountSkinComboTarget">{comboTargetLabel}</span>
                      </div>
                    ) : null}
                    {skinViewerMineBurstKey > 0 ? (
                      <div key={skinViewerMineBurstKey} className="accountSkinMineBurst" />
                    ) : null}
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
                    <div className="skinsRefSectionTitle accountSkinHeading">Saved Skins</div>
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
                                <SkinThumbFace
                                  src={frontThumb}
                                  alt={`${skin.label} front preview`}
                                  label={skin.label}
                                  tone="missing"
                                />
                              </div>
                              <div className="accountSkinChoiceFace accountSkinChoiceFaceBack">
                                <SkinThumbFace
                                  src={backThumb}
                                  alt={`${skin.label} back preview`}
                                  label={skin.label}
                                  tone="missing"
                                />
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
  
                  <div className="skinsRefSectionTitle skinsRefDefaultTitle accountSkinHeading">Default Skins</div>
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
                                <SkinThumbFace
                                  src={frontThumb}
                                  alt={`${skin.label} front preview`}
                                  label={skin.label}
                                  tone="missing"
                                />
                              </div>
                              <div className="accountSkinChoiceFace accountSkinChoiceFaceBack">
                                <SkinThumbFace
                                  src={backThumb}
                                  alt={`${skin.label} back preview`}
                                  label={skin.label}
                                  tone="missing"
                                />
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
