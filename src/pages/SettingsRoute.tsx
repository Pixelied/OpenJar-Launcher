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

export type SettingsRouteProps = {
  accentPreset: any;
  accentStrength: any;
  activeSettingsRail: any;
  appLanguage: any;
  appLanguageBusy: any;
  appLanguageMenuOptions: any;
  appUpdaterAutoCheck: any;
  appUpdaterBusy: any;
  appUpdaterInstallBusy: any;
  appUpdaterState: any;
  appVersion: any;
  autoIdentifyLocalJarsBusy: any;
  autoMicPromptSettingBusy: any;
  densityPreset: any;
  discordPresenceBusy: any;
  discoverAddTraySticky: any;
  githubTokenPoolBusy: any;
  githubTokenPoolDraft: any;
  githubTokenPoolNotice: any;
  githubTokenPoolNoticeIsError: any;
  githubTokenPoolStatus: any;
  instances: any;
  javaPathDraft: any;
  javaRuntimeBusy: any;
  javaRuntimeCandidates: any;
  launchMethodPick: any;
  launcherAccounts: any;
  launcherBusy: any;
  launcherErr: any;
  launcherSettings: any;
  logMaxLines: any;
  motionPreset: any;
  msCodePrompt: any;
  msLoginSessionId: any;
  msLoginState: any;
  oauthClientIdDraft: any;
  onBeginMicrosoftLogin: any;
  onCheckAppUpdate: any;
  onClearGithubTokenPool: any;
  onInstallAppUpdate: any;
  onLogoutAccount: any;
  onPickLauncherJavaPath: any;
  onResetUiSettings: any;
  onSaveGithubTokenPool: any;
  onSaveLauncherPrefs: any;
  onSelectAccount: any;
  onSetAppLanguage: any;
  onSetDiscordPresenceDetailLevel: any;
  onToggleAutoIdentifyLocalJars: any;
  onToggleAutoMicPermissionPrompt: any;
  onToggleDiscordPresenceEnabled: any;
  onValidateGithubTokenPool: any;
  openMicrophoneSystemSettings: any;
  openSettingAnchor: any;
  preflightReportByInstance: any;
  refreshGithubTokenPoolStatus: any;
  refreshInstancePermissionChecklist: any;
  refreshJavaRuntimeCandidates: any;
  selectedId: any;
  selectedLauncherAccount: any;
  setAccentPreset: any;
  setAccentStrength: any;
  setAppUpdaterAutoCheck: any;
  setDensityPreset: any;
  setDiscoverAddTraySticky: any;
  setGithubTokenPoolDraft: any;
  setInstallNotice: any;
  setJavaPathDraft: any;
  setLaunchMethodPick: any;
  setLogMaxLines: any;
  setMotionPreset: any;
  setMsCodePromptVisible: any;
  setOauthClientIdDraft: any;
  setRoute: any;
  setSettingsAccountManageId: any;
  setSettingsMode: any;
  setSkinPreviewEnabled: any;
  setSupportBundleIncludeRawLogs: any;
  setTheme: any;
  settingsAccountManageId: any;
  settingsMode: any;
  settingsRailItems: any;
  skinPreviewEnabled: any;
  supportBundleIncludeRawLogs: any;
  t: any;
  theme: any;
  triggerInstanceMicrophonePrompt: any;
};

export default function SettingsRoute(props: SettingsRouteProps) {
  const {
    accentPreset,
    accentStrength,
    activeSettingsRail,
    appLanguage,
    appLanguageBusy,
    appLanguageMenuOptions,
    appUpdaterAutoCheck,
    appUpdaterBusy,
    appUpdaterInstallBusy,
    appUpdaterState,
    appVersion,
    autoIdentifyLocalJarsBusy,
    autoMicPromptSettingBusy,
    densityPreset,
    discordPresenceBusy,
    discoverAddTraySticky,
    githubTokenPoolBusy,
    githubTokenPoolDraft,
    githubTokenPoolNotice,
    githubTokenPoolNoticeIsError,
    githubTokenPoolStatus,
    instances,
    javaPathDraft,
    javaRuntimeBusy,
    javaRuntimeCandidates,
    launchMethodPick,
    launcherAccounts,
    launcherBusy,
    launcherErr,
    launcherSettings,
    logMaxLines,
    motionPreset,
    msCodePrompt,
    msLoginSessionId,
    msLoginState,
    oauthClientIdDraft,
    onBeginMicrosoftLogin,
    onCheckAppUpdate,
    onClearGithubTokenPool,
    onInstallAppUpdate,
    onLogoutAccount,
    onPickLauncherJavaPath,
    onResetUiSettings,
    onSaveGithubTokenPool,
    onSaveLauncherPrefs,
    onSelectAccount,
    onSetAppLanguage,
    onSetDiscordPresenceDetailLevel,
    onToggleAutoIdentifyLocalJars,
    onToggleAutoMicPermissionPrompt,
    onToggleDiscordPresenceEnabled,
    onValidateGithubTokenPool,
    openMicrophoneSystemSettings,
    openSettingAnchor,
    preflightReportByInstance,
    refreshGithubTokenPoolStatus,
    refreshInstancePermissionChecklist,
    refreshJavaRuntimeCandidates,
    selectedId,
    selectedLauncherAccount,
    setAccentPreset,
    setAccentStrength,
    setAppUpdaterAutoCheck,
    setDensityPreset,
    setDiscoverAddTraySticky,
    setGithubTokenPoolDraft,
    setInstallNotice,
    setJavaPathDraft,
    setLaunchMethodPick,
    setLogMaxLines,
    setMotionPreset,
    setMsCodePromptVisible,
    setOauthClientIdDraft,
    setRoute,
    setSettingsAccountManageId,
    setSettingsMode,
    setSkinPreviewEnabled,
    setSupportBundleIncludeRawLogs,
    setTheme,
    settingsAccountManageId,
    settingsMode,
    settingsRailItems,
    skinPreviewEnabled,
    supportBundleIncludeRawLogs,
    t,
    theme,
    triggerInstanceMicrophonePrompt
  } = props;

  const selectedPermissionsInstance = selectedId
          ? instances.find((item) => item.id === selectedId) ?? null
          : null;
        const selectedPermissionsChecklist: LaunchPermissionChecklistItem[] = selectedPermissionsInstance
          ? preflightReportByInstance[selectedPermissionsInstance.id]?.permissions ?? []
          : [];
        const settingsSelectedAccount = selectedLauncherAccount ?? launcherAccounts[0] ?? null;
        const settingsUpdateStatusLabel = appUpdaterState?.available
          ? `Update ready${appUpdaterState.latest_version ? ` · v${appUpdaterState.latest_version}` : ""}`
          : appUpdaterState
            ? "Up to date"
            : "Not checked yet";
        return (
          <div className="settingsPage">
            <div className="settingsShell">
              <aside className="card settingsSidebar">
                <div className="settingsSidebarHeader">
                  <div className="settingsSidebarEyebrow">Settings navigation</div>
                  <div className="settingsSidebarTitle">Sections</div>
                  <div className="settingsSidebarIntro">
                    Jump between appearance, launch, account, and update controls without losing your place.
                  </div>
                </div>
  
                <div className="settingsSidebarBlock">
                  <div className="settingsSidebarLabel">View mode</div>
                  <SegmentedControl
                    className="settingsModeToggle"
                    value={settingsMode}
                    onChange={(value) => setSettingsMode(((value ?? "basic") as SettingsMode))}
                    options={[
                      { value: "basic", label: t("settings.mode.basic") },
                      { value: "advanced", label: t("settings.mode.advanced") },
                    ]}
                  />
                </div>
  
                <div className="settingsSidebarBlock">
                  <div className="settingsSidebarLabel">Sections</div>
                  <div className="settingsRailList">
                    {settingsRailItems.map((item) => (
                      <button
                        key={item.id}
                        className={`settingsRailButton ${activeSettingsRail === item.id ? "active" : ""}`}
                        onClick={() => openSettingAnchor(item.id, { advanced: item.advanced, target: "global" })}
                      >
                        <span className="settingsRailButtonIcon" aria-hidden="true">
                          <Icon name={item.icon} size={16} />
                        </span>
                        <span className="settingsRailButtonText">
                          <span className="settingsRailButtonTitle">{item.label}</span>
                          {item.advanced ? <span className="settingsRailButtonMeta">Advanced</span> : null}
                        </span>
                      </button>
                    ))}
                  </div>
                </div>
  
                <div className="settingsSidebarFooter">
                  <span className="chip subtle">{getAppLanguageOption(appLanguage).nativeLabel}</span>
                  <span className="chip subtle">
                    {settingsMode === "advanced" ? t("settings.mode.advanced") : t("settings.mode.basic")}
                  </span>
                </div>
              </aside>
  
              <div className="settingsMain">
                <div className="pageRouteHeader">
                  <div className="pageRouteEyebrow">Launcher settings</div>
                  <div className="h1">{t("settings.title")}</div>
                  <div className="p">{t("settings.intro")}</div>
                </div>
  
                <div className="card settingsHeroCard">
                  <div className="settingsHeroLead">
                    <div className="settingsHeroLeadLabel">At a glance</div>
                    <div className="settingsHeroLeadText">
                      Your current mode, language, launch defaults, account state, and updater health in one tighter pass.
                    </div>
                  </div>
  
                  <div className="settingsHeroSummaryGrid">
                    <div className="settingsHeroSummaryCard">
                      <div className="settingsHeroSummaryLabel">View mode</div>
                      <div className="settingsHeroSummaryValue">
                        {settingsMode === "advanced" ? t("settings.mode.advanced") : t("settings.mode.basic")}
                      </div>
                      <div className="settingsHeroSummaryMeta">
                        {settingsMode === "advanced"
                          ? "Power-user controls are visible."
                          : "Only the common settings are emphasized."}
                      </div>
                    </div>
                    <div className="settingsHeroSummaryCard">
                      <div className="settingsHeroSummaryLabel">Language</div>
                      <div className="settingsHeroSummaryValue">{getAppLanguageOption(appLanguage).nativeLabel}</div>
                      <div className="settingsHeroSummaryMeta">App UI language</div>
                    </div>
                    <div className="settingsHeroSummaryCard">
                      <div className="settingsHeroSummaryLabel">Default launch</div>
                      <div className="settingsHeroSummaryValue">{humanizeToken(launchMethodPick)}</div>
                      <div className="settingsHeroSummaryMeta">Used when an instance does not override it</div>
                    </div>
                    <div className="settingsHeroSummaryCard">
                      <div className="settingsHeroSummaryLabel">Connected account</div>
                      <div className="settingsHeroSummaryValue">
                        {settingsSelectedAccount?.username ?? "Not connected"}
                      </div>
                      <div className="settingsHeroSummaryMeta">
                        {settingsSelectedAccount ? "Ready for native launch" : "Connect Microsoft to launch natively"}
                      </div>
                    </div>
                    <div className="settingsHeroSummaryCard">
                      <div className="settingsHeroSummaryLabel">App updates</div>
                      <div className="settingsHeroSummaryValue">{settingsUpdateStatusLabel}</div>
                      <div className="settingsHeroSummaryMeta">
                        {appUpdaterState?.checked_at
                          ? `Last check ${formatDateTime(appUpdaterState.checked_at, "Never")}`
                          : "No launcher update check yet"}
                      </div>
                    </div>
                  </div>
                </div>
  
                <div className="settingsLayout">
                  <section className="settingsCol">
                <div className="card settingsSectionCard" id="setting-anchor-global:appearance">
                  <div className="settingsSectionTitle">{t("settings.appearance.section_title")}</div>
                  <div className="p settingsSectionSub">{t("settings.appearance.section_sub")}</div>
  
                  <div className="settingStack">
                    <div>
                      <div className="settingTitle">{t("settings.appearance.theme.title")}</div>
                      <div className="settingSub">{t("settings.appearance.theme.sub")}</div>
                      <div className="row">
                        <button
                          className={`btn stateful ${theme === "dark" ? "active" : ""}`}
                          onClick={() => setTheme("dark")}
                        >
                          {t("settings.appearance.theme.dark")}
                        </button>
                        <button
                          className={`btn stateful ${theme === "light" ? "active" : ""}`}
                          onClick={() => setTheme("light")}
                        >
                          {t("settings.appearance.theme.light")}
                        </button>
                      </div>
                    </div>
  
                    <div>
                      <div className="settingTitle">{t("settings.appearance.accent.title")}</div>
                      <div className="settingSub">{t("settings.appearance.accent.sub")}</div>
                      <div className="row accentPicker">
                        {ACCENT_OPTIONS.map((opt) => (
                          <button
                            key={opt.value}
                            className={`btn accentChoice ${accentPreset === opt.value ? "selected" : ""}`}
                            onClick={() => setAccentPreset(opt.value)}
                            aria-pressed={accentPreset === opt.value}
                          >
                            <span className={`accentSwatch accent-${opt.value}`} />
                            <span className="accentChoiceLabel">{opt.label}</span>
                            {accentPreset === opt.value ? (
                              <span className="accentChoiceCheck" aria-hidden="true">✓</span>
                            ) : null}
                          </button>
                        ))}
                      </div>
                    </div>
  
                    <div>
                      <div className="settingTitle">{t("settings.appearance.accent_strength.title")}</div>
                      <div className="settingSub">{t("settings.appearance.accent_strength.sub")}</div>
                      <div className="row">
                        <SegmentedControl
                          value={accentStrength}
                          options={ACCENT_STRENGTH_OPTIONS}
                          onChange={(v) => setAccentStrength((v ?? "normal") as AccentStrength)}
                          variant="scroll"
                        />
                      </div>
                    </div>
  
                    <div>
                      <div className="settingTitle">{t("settings.appearance.motion.title")}</div>
                      <div className="settingSub">{t("settings.appearance.motion.sub")}</div>
                      <div className="row">
                        <SegmentedControl
                          value={motionPreset}
                          options={MOTION_OPTIONS}
                          onChange={(v) => setMotionPreset((v ?? "standard") as MotionPreset)}
                        />
                      </div>
                      <div className="settingsMotionNote" aria-live="polite">
                        <span className="chip subtle">{MOTION_PROFILE_DETAILS[motionPreset].label}</span>
                        {MOTION_PROFILE_DETAILS[motionPreset].traits.map((trait) => (
                          <span key={trait} className="chip subtle">
                            {trait}
                          </span>
                        ))}
                        <span className="settingsMotionNoteText">
                          {MOTION_PROFILE_DETAILS[motionPreset].summary}
                        </span>
                      </div>
                    </div>
  
                    <div>
                      <div className="settingTitle">{t("settings.appearance.density.title")}</div>
                      <div className="settingSub">{t("settings.appearance.density.sub")}</div>
                      <div className="row">
                        <SegmentedControl
                          value={densityPreset}
                          options={DENSITY_OPTIONS}
                          onChange={(v) => setDensityPreset((v ?? "comfortable") as DensityPreset)}
                        />
                      </div>
                    </div>
  
                    <div>
                      <div className="settingTitle">{t("settings.appearance.reset.title")}</div>
                      <div className="settingSub">{t("settings.appearance.reset.sub")}</div>
                      <div className="row">
                        <button className="btn" onClick={onResetUiSettings}>
                          {t("settings.appearance.reset.button")}
                        </button>
                      </div>
                    </div>
                  </div>
                </div>
  
                <div className="card settingsSectionCard" id="setting-anchor-global:language">
                  <div className="settingsSectionTitle">{t("settings.language.section_title")}</div>
                  <div className="p settingsSectionSub">{t("settings.language.section_sub")}</div>
  
                  <div className="settingStack">
                    <div>
                      <div className="settingTitle">{t("settings.language.preference.title")}</div>
                      <div className="settingSub">{t("settings.language.preference.sub")}</div>
                      <div className="row" style={{ alignItems: "center" }}>
                        <MenuSelect
                          value={appLanguage}
                          labelPrefix={t("settings.language.preference.menu_prefix")}
                          options={appLanguageMenuOptions}
                          onChange={(value) => void onSetAppLanguage(value as AppLanguage)}
                        />
                        <span className="chip">{appLanguageBusy ? t("settings.language.saving") : getAppLanguageOption(appLanguage).nativeLabel}</span>
                      </div>
                    </div>
  
                    <div className="settingSub">{t("settings.language.warning")}</div>
                  </div>
                </div>
  
                <div className="card settingsSectionCard">
                  <div className="settingsSectionTitle">{t("settings.launch.section_title")}</div>
                  <div className="p settingsSectionSub">{t("settings.launch.section_sub")}</div>
  
                  <div className="settingStack">
                    <div id="setting-anchor-global:launch-method">
                      <div className="settingTitle">{t("settings.launch.method.title")}</div>
                      <div className="settingSub">{t("settings.launch.method.sub")}</div>
                      <div className="row">
                        <SegmentedControl
                          value={launchMethodPick}
                          onChange={(v) => setLaunchMethodPick((v ?? "native") as LaunchMethod)}
                          options={[
                            { label: t("settings.launch.method.native"), value: "native" },
                            { label: t("settings.launch.method.prism"), value: "prism" },
                          ]}
                        />
                      </div>
                    </div>
  
                    {settingsMode === "advanced" ? (
                      <>
                        <div id="setting-anchor-global:java-path">
                          <div className="settingTitle">{t("settings.launch.java.title")}</div>
                          <div className="settingSub">{t("settings.launch.java.sub")}</div>
                          <input
                            className="input"
                            value={javaPathDraft}
                            onChange={(e) => setJavaPathDraft(e.target.value)}
                            placeholder="/usr/bin/java or C:\\Program Files\\Java\\bin\\java.exe"
                          />
                          <div className="settingsActionGrid">
                            <button className="btn" onClick={onPickLauncherJavaPath} disabled={launcherBusy}>
                              <span className="btnIcon">
                                <Icon name="upload" size={17} />
                              </span>
                              {t("settings.launch.java.browse")}
                            </button>
                            <button className="btn" onClick={() => void refreshJavaRuntimeCandidates()} disabled={javaRuntimeBusy}>
                              {javaRuntimeBusy
                                ? t("settings.launch.java.detecting")
                                : t("settings.launch.java.detect")}
                            </button>
                            <button
                              className="btn"
                              onClick={() => void openExternalLink("https://adoptium.net/temurin/releases/?version=21")}
                            >
                              {t("settings.launch.java.get_java_21")}
                            </button>
                          </div>
                          {javaRuntimeCandidates.length > 0 ? (
                            <div className="settingListMini">
                              {javaRuntimeCandidates.map((runtime) => (
                                <div key={runtime.path} className="settingListMiniRow">
                                  <div style={{ minWidth: 0 }}>
                                    <div style={{ fontWeight: 900 }}>{javaRuntimeDisplayLabel(runtime)}</div>
                                    <div className="muted" style={{ wordBreak: "break-all" }}>{runtime.path}</div>
                                  </div>
                                  <button
                                    className={`btn stateful ${javaPathDraft.trim() === runtime.path.trim() ? "active" : ""}`}
                                    onClick={() => setJavaPathDraft(runtime.path)}
                                    disabled={launcherBusy}
                                  >
                                    {javaPathDraft.trim() === runtime.path.trim()
                                      ? t("settings.launch.java.selected")
                                      : t("settings.launch.java.use")}
                                  </button>
                                </div>
                              ))}
                            </div>
                          ) : null}
                        </div>
  
                        <div id="setting-anchor-global:oauth-client">
                          <div className="settingTitle">{t("settings.launch.oauth.title")}</div>
                          <div className="settingSub">{t("settings.launch.oauth.sub")}</div>
                          <input
                            className="input"
                            value={oauthClientIdDraft}
                            onChange={(e) => setOauthClientIdDraft(e.target.value)}
                            placeholder={t("settings.launch.oauth.placeholder")}
                            style={{ marginTop: 8 }}
                          />
                        </div>
                      </>
                    ) : (
                      <div className="muted">
                        {t("settings.launch.basic_hidden")}
                        <button className="btn" style={{ marginLeft: 8 }} onClick={() => setSettingsMode("advanced")}>
                          {t("settings.launch.switch_to_advanced")}
                        </button>
                      </div>
                    )}
  
                    <div className="settingsSaveRow">
                      <button className="btn primary" onClick={onSaveLauncherPrefs} disabled={launcherBusy}>
                        {launcherBusy ? t("settings.launch.saving") : t("settings.launch.save")}
                      </button>
                    </div>
                  </div>
                </div>
              </section>
  
              <section className="settingsCol">
                <div className="card settingsSectionCard" id="setting-anchor-global:account">
                  <div className="settingsSectionTitle">{t("settings.account.section_title")}</div>
                  <div className="p settingsSectionSub">{t("settings.account.section_sub")}</div>
  
                  <div className="row" style={{ marginTop: 8 }}>
                    <button className="btn primary" onClick={onBeginMicrosoftLogin} disabled={launcherBusy}>
                      {msLoginSessionId ? "Waiting for login…" : "Connect Microsoft"}
                    </button>
                    {msLoginSessionId && msCodePrompt ? (
                      <button className="btn" onClick={() => setMsCodePromptVisible(true)}>
                        Show code
                      </button>
                    ) : null}
                    <button className="btn" onClick={() => setRoute("account")}>
                      Open account page
                    </button>
                    {msLoginState?.message ? <div className="muted">{msLoginState.message}</div> : null}
                  </div>
  
                  <div className="settingsAccountList">
                    {launcherAccounts.length === 0 ? (
                      <div className="muted">No connected account yet.</div>
                    ) : (
                      launcherAccounts.map((acct) => {
                        const selectedAccount = launcherSettings?.selected_account_id === acct.id;
                        const manageOpen = settingsAccountManageId === acct.id;
                        return (
                          <div key={acct.id} className="card settingsAccountCard">
                            <div className="settingsAccountRow">
                              <div style={{ minWidth: 0 }}>
                                <div style={{ fontWeight: 900 }}>{acct.username}</div>
                                <div className="muted">{acct.id}</div>
                              </div>
                              <div className="settingsAccountActions">
                                <button
                                  className={`btn stateful ${selectedAccount ? "active" : ""}`}
                                  onClick={() => onSelectAccount(acct.id)}
                                  disabled={launcherBusy}
                                >
                                  {selectedAccount ? "Selected" : "Use"}
                                </button>
                                <button
                                  className={`btn subtle settingsManageBtn ${manageOpen ? "active" : ""}`}
                                  onClick={() =>
                                    setSettingsAccountManageId((prev) => (prev === acct.id ? null : acct.id))
                                  }
                                  disabled={launcherBusy}
                                  aria-expanded={manageOpen}
                                  aria-controls={`settings-account-manage-${acct.id}`}
                                >
                                  Manage…
                                </button>
                              </div>
                            </div>
                            {manageOpen ? (
                              <div className="settingsAccountManagePanel" id={`settings-account-manage-${acct.id}`}>
                                <div className="settingsAccountManageHint">
                                  Disconnect removes this account from this launcher on this device.
                                </div>
                                <button
                                  className="btn accountDisconnectBtn"
                                  onClick={() => {
                                    if (!window.confirm(`Disconnect ${acct.username} from this launcher?`)) return;
                                    void onLogoutAccount(acct.id);
                                  }}
                                  disabled={launcherBusy}
                                >
                                  Disconnect account
                                </button>
                              </div>
                            ) : null}
                          </div>
                        );
                      })
                    )}
                  </div>
                </div>
  
                <div className="card settingsSectionCard" id="setting-anchor-global:app-updates">
                  <div className="settingsSectionTitle">{t("settings.updates.section_title")}</div>
                  <div className="p settingsSectionSub">
                    Check for new OpenJar Launcher releases, then install with explicit restart confirmation.
                  </div>
  
                  <div className="row">
                    <span className="chip subtle">Current: v{appVersion || "unknown"}</span>
                    {appUpdaterState ? (
                      <span className="chip subtle">
                        Last check: {formatDateTime(appUpdaterState.checked_at, "Never")}
                      </span>
                    ) : null}
                    {appUpdaterState?.available ? (
                      <span className="chip">Update: v{appUpdaterState.latest_version ?? "new"}</span>
                    ) : appUpdaterState ? (
                      <span className="chip subtle">Up to date</span>
                    ) : null}
                  </div>
                  <div className="settingsActionGrid">
                    <button
                      className="btn"
                      onClick={() => void onCheckAppUpdate({ silent: false })}
                      disabled={appUpdaterBusy || appUpdaterInstallBusy}
                    >
                      {appUpdaterBusy ? "Checking…" : "Check app updates"}
                    </button>
                    <button
                      className={`btn ${appUpdaterState?.available ? "primary" : ""}`}
                      onClick={() => void onInstallAppUpdate()}
                      disabled={!appUpdaterState?.available || appUpdaterBusy || appUpdaterInstallBusy}
                    >
                      {appUpdaterInstallBusy ? "Installing…" : "Install update + restart"}
                    </button>
                  </div>
                  <div className="settingStackMini">
                    <label className="toggleRow settingsToggleRow">
                      <input
                        type="checkbox"
                        checked={appUpdaterAutoCheck}
                        onChange={() => setAppUpdaterAutoCheck((prev) => !prev)}
                        disabled={appUpdaterBusy || appUpdaterInstallBusy}
                      />
                      <span className="togglePill" />
                      <span>Auto-check on launch</span>
                    </label>
                    <div className="settingSub">Checks for OpenJar Launcher releases when the app starts.</div>
                  </div>
                  {appUpdaterState?.release_notes ? (
                    <div className="settingsReleaseNotesExcerpt">
                      {appUpdaterState.release_notes.slice(0, 280)}
                      {appUpdaterState.release_notes.length > 280 ? "…" : ""}
                    </div>
                  ) : null}
                </div>
  
                <div className="card settingsSectionCard" id="setting-anchor-global:content-visuals">
                  <div className="settingsSectionTitle">{t("settings.content.section_title")}</div>
                  <div className="p settingsSectionSub">Quick toggles for launcher behavior outside game runtime.</div>
  
                  <div className="settingStack">
                    <div>
                      <div className="settingTitle">Automatic identify local files</div>
                      <div className="settingSub">
                        When enabled, local file imports automatically run Identify local files in Instance and Creator Studio.
                      </div>
                      <label className="toggleRow settingsToggleRow">
                        <input
                          type="checkbox"
                          checked={Boolean(launcherSettings?.auto_identify_local_jars)}
                          onChange={() => void onToggleAutoIdentifyLocalJars()}
                          disabled={autoIdentifyLocalJarsBusy}
                        />
                        <span className="togglePill" />
                        <span>{autoIdentifyLocalJarsBusy ? "Saving…" : "Identify local files automatically"}</span>
                      </label>
                    </div>
  
                    <div>
                      <div className="settingTitle">3D skin preview</div>
                      <div className="settingSub">
                        Disable this for faster Account and Skins page loads on lower-end hardware.
                      </div>
                      <label className="toggleRow settingsToggleRow">
                        <input
                          type="checkbox"
                          checked={skinPreviewEnabled}
                          onChange={() => setSkinPreviewEnabled((prev) => !prev)}
                        />
                        <span className="togglePill" />
                        <span>Enable 3D skin preview</span>
                      </label>
                    </div>
  
                    <div id="setting-anchor-global:discord-presence">
                      <div className="settingTitle">Discord Rich Presence</div>
                      <div className="settingSub">
                        Optional status sharing. Never includes server IP, username, world name, or file paths.
                      </div>
                      <label className="toggleRow settingsToggleRow">
                        <input
                          type="checkbox"
                          checked={Boolean(launcherSettings?.discord_presence_enabled ?? true)}
                          onChange={() => void onToggleDiscordPresenceEnabled()}
                          disabled={discordPresenceBusy}
                        />
                        <span className="togglePill" />
                        <span>{discordPresenceBusy ? "Saving…" : "Enable Discord Rich Presence"}</span>
                      </label>
                      <div className="row">
                        <MenuSelect
                          value={String(launcherSettings?.discord_presence_detail_level ?? "minimal")}
                          labelPrefix="Detail"
                          options={[
                            { value: "minimal", label: "Minimal" },
                            { value: "expanded", label: "Expanded" },
                          ]}
                          onChange={(value) =>
                            void onSetDiscordPresenceDetailLevel(
                              String(value ?? "minimal") === "expanded" ? "expanded" : "minimal"
                            )
                          }
                        />
                      </div>
                    </div>
                  </div>
                </div>
  
                  {settingsMode === "advanced" ? (
                    <div className="card settingsSectionCard">
                    <div className="settingsSectionTitle">{t("settings.advanced.section_title")}</div>
                    <div className="p settingsSectionSub">Power-user defaults and launch permission controls.</div>
                    <div className="settingStack">
                      <div>
                        <div className="settingTitle">Power-user defaults</div>
                        <div className="settingSub">
                          Extra launcher behavior toggles for advanced workflows.
                        </div>
                        <label className="toggleRow settingsToggleRow">
                          <input
                            type="checkbox"
                            checked={discoverAddTraySticky}
                            onChange={() => setDiscoverAddTraySticky((prev) => !prev)}
                          />
                          <span className="togglePill" />
                          <span>Keep Discover add tray pinned</span>
                        </label>
                        <label className="toggleRow settingsToggleRow">
                          <input
                            type="checkbox"
                            checked={supportBundleIncludeRawLogs}
                            onChange={() => setSupportBundleIncludeRawLogs((prev) => !prev)}
                          />
                          <span className="togglePill" />
                          <span>Include raw logs by default in support bundles</span>
                        </label>
                        <div className="row">
                          <MenuSelect
                            value={String(logMaxLines)}
                            labelPrefix="Default log window"
                            options={LOG_MAX_LINES_OPTIONS}
                            onChange={(v) => {
                              const parsed = Number.parseInt(String(v ?? ""), 10);
                              if (!Number.isFinite(parsed)) return;
                              setLogMaxLines(Math.max(200, Math.min(12000, parsed)));
                            }}
                          />
                        </div>
                      </div>
  
                      <details className="settingsFoldSection" id="setting-anchor-global:permissions" open>
                        <summary className="settingsFoldSummary">
                          <span className="settingsFoldTitle">Launch permissions</span>
                          <span className="settingsFoldMeta">Microphone checks and prompt controls</span>
                        </summary>
                        <div className="settingsFoldBody">
                          <div className="settingSub">
                            Voice chat instances can auto-trigger a Java microphone permission probe before launch.
                          </div>
                          <label className="toggleRow settingsToggleRow">
                            <input
                              type="checkbox"
                              checked={Boolean(launcherSettings?.auto_trigger_mic_permission_prompt ?? true)}
                              onChange={() => void onToggleAutoMicPermissionPrompt()}
                              disabled={autoMicPromptSettingBusy}
                            />
                            <span className="togglePill" />
                            <span>
                              {autoMicPromptSettingBusy ? "Saving…" : "Enable auto microphone prompt"}
                            </span>
                          </label>
                          <div className="settingsActionGrid">
                            <button className="btn" onClick={() => void openMicrophoneSystemSettings()}>
                              Open microphone settings
                            </button>
                            <button
                              className="btn"
                              onClick={() =>
                                selectedPermissionsInstance
                                  ? void triggerInstanceMicrophonePrompt(selectedPermissionsInstance.id)
                                  : setInstallNotice("Select an instance first to trigger microphone prompt.")
                              }
                              disabled={!selectedPermissionsInstance}
                            >
                              Trigger selected prompt
                            </button>
                            <button
                              className="btn"
                              onClick={() =>
                                selectedPermissionsInstance
                                  ? void refreshInstancePermissionChecklist(selectedPermissionsInstance.id, launchMethodPick)
                                  : setInstallNotice("Select an instance first to run a permission re-check.")
                              }
                              disabled={!selectedPermissionsInstance}
                            >
                              Re-check selected instance
                            </button>
                          </div>
                          <div className="settingsInlineStatus">
                            Selected instance: {selectedPermissionsInstance?.name ?? "None"}.
                          </div>
                          {selectedPermissionsChecklist.length > 0 ? (
                            <div className="preflightPermissionsList" style={{ marginTop: 8 }}>
                              {selectedPermissionsChecklist.map((perm) => (
                                <div key={`settings-perm:${perm.key}`} className="preflightPermissionRow">
                                  <div className="preflightCheckMain">
                                    <div className="preflightCheckTitle">{perm.label}</div>
                                    <div className="preflightCheckMsg">{perm.detail}</div>
                                  </div>
                                  <span className={`chip ${permissionStatusChipClass(perm.status)}`}>
                                    {permissionStatusLabel(perm.status)}
                                  </span>
                                </div>
                              ))}
                            </div>
                          ) : (
                            <div className="settingsEmptyStateHint">
                              No permission report yet for the selected instance. Click Re-check selected instance.
                            </div>
                          )}
                        </div>
                      </details>
  
                      <details className="settingsFoldSection" id="setting-anchor-global:github-api">
                        <summary className="settingsFoldSummary">
                          <span className="settingsFoldTitle">GitHub API authentication</span>
                          <span className="settingsFoldMeta">Token pool for higher API limits</span>
                        </summary>
                        <div className="settingsFoldBody">
                          <div className="settingSub">
                            Save personal access tokens to secure OS keychain storage for higher GitHub rate limits. Tokens are not stored in launcher settings files.
                          </div>
                          <div className="row settingsInlineBadges">
                            <span className="chip">Tokens are stored in Keychain</span>
                            {githubTokenPoolStatus ? (
                              <span className={`chip ${githubTokenPoolStatus.configured ? "" : "subtle"}`}>
                                {githubTokenPoolStatus.configured
                                  ? `${githubTokenPoolStatus.total_tokens} token${githubTokenPoolStatus.total_tokens === 1 ? "" : "s"} configured`
                                  : "Ready for first token"}
                              </span>
                            ) : null}
                          </div>
                          <textarea
                            className={`input githubTokenTextarea ${
                              !githubTokenPoolStatus?.configured && !githubTokenPoolDraft.trim() ? "ready" : ""
                            }`}
                            value={githubTokenPoolDraft}
                            onChange={(e) => setGithubTokenPoolDraft(e.target.value)}
                            placeholder="Paste one token per line (or comma/semicolon separated)"
                            rows={3}
                          />
                          {!githubTokenPoolStatus?.configured && !githubTokenPoolDraft.trim() ? (
                            <div className="githubTokenReadyHint">
                              Paste tokens here and click Validate. We store them in secure Keychain storage only.
                            </div>
                          ) : null}
                          <div className="settingsActionGrid">
                            <button
                              className="btn primary"
                              onClick={() => void onSaveGithubTokenPool()}
                              disabled={githubTokenPoolBusy}
                            >
                              {githubTokenPoolBusy ? "Saving…" : "Save GitHub tokens"}
                            </button>
                            <button
                              className="btn"
                              onClick={() => void onValidateGithubTokenPool()}
                              disabled={githubTokenPoolBusy}
                            >
                              {githubTokenPoolBusy ? "Validating…" : "Validate"}
                            </button>
                            <button
                              className="btn"
                              onClick={() => void onClearGithubTokenPool()}
                              disabled={githubTokenPoolBusy}
                            >
                              Clear saved tokens
                            </button>
                            <button
                              className="btn subtle"
                              onClick={() => void refreshGithubTokenPoolStatus()}
                              disabled={githubTokenPoolBusy}
                            >
                              {githubTokenPoolBusy ? "Checking…" : "Refresh status"}
                            </button>
                          </div>
                          {githubTokenPoolStatus ? (
                            <div className="settingsStatusStack">
                              <div className={githubTokenPoolStatus.keychain_available ? "noticeBox" : "warningBox"}>
                                {githubTokenPoolStatus.message}
                              </div>
                              <div className="muted">
                                Sources: env {githubTokenPoolStatus.env_tokens} · keychain {githubTokenPoolStatus.keychain_tokens}
                                {githubTokenPoolStatus.unauth_rate_limited
                                  ? ` · unauth rate-limited${githubTokenPoolStatus.unauth_rate_limit_reset_at ? ` until ${githubTokenPoolStatus.unauth_rate_limit_reset_at}` : ""}`
                                  : ""}
                              </div>
                            </div>
                          ) : null}
                          {githubTokenPoolNotice ? (
                            <div className={githubTokenPoolNoticeIsError ? "errorBox" : "noticeBox"} style={{ marginTop: 10 }}>
                              {githubTokenPoolNotice}
                            </div>
                          ) : null}
                        </div>
                      </details>
                    </div>
                  </div>
                ) : null}
                  </section>
                </div>
              </div>
            </div>
  
            {launcherErr ? <div className="errorBox" style={{ marginTop: 14 }}>{launcherErr}</div> : null}
          </div>
        );
}
