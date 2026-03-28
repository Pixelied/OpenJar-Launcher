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

export type AccountRouteProps = {
  accountAvatarFromSkin: any;
  accountAvatarSourceIdx: any;
  accountDiagnostics: any;
  accountDiagnosticsBusy: any;
  accountDiagnosticsErr: any;
  capeOptions: any;
  defaultSkinOptions: any;
  launcherAccounts: any;
  launcherBusy: any;
  launcherSettings: any;
  minecraftAvatarSources: any;
  msCodePrompt: any;
  msLoginSessionId: any;
  onBeginMicrosoftLogin: any;
  onLogoutAccount: any;
  onSelectAccount: any;
  refreshAccountDiagnostics: any;
  savedSkinOptions: any;
  selectedAccountCape: any;
  selectedAccountSkin: any;
  selectedLauncherAccount: any;
  selectedLauncherAccountId: any;
  setAccountAvatarSourceIdx: any;
  setMsCodePromptVisible: any;
  setRoute: any;
  setSkinPreviewEnabled: any;
  skinPreviewEnabled: any;
  toLocalIconSrc: any;
  updateAutoApplyMode: any;
  updateCheckCadence: any;
};

export default function AccountRoute(props: AccountRouteProps) {
  const {
    accountAvatarFromSkin,
    accountAvatarSourceIdx,
    accountDiagnostics,
    accountDiagnosticsBusy,
    accountDiagnosticsErr,
    capeOptions,
    defaultSkinOptions,
    launcherAccounts,
    launcherBusy,
    launcherSettings,
    minecraftAvatarSources,
    msCodePrompt,
    msLoginSessionId,
    onBeginMicrosoftLogin,
    onLogoutAccount,
    onSelectAccount,
    refreshAccountDiagnostics,
    savedSkinOptions,
    selectedAccountCape,
    selectedAccountSkin,
    selectedLauncherAccount,
    selectedLauncherAccountId,
    setAccountAvatarSourceIdx,
    setMsCodePromptVisible,
    setRoute,
    setSkinPreviewEnabled,
    skinPreviewEnabled,
    toLocalIconSrc,
    updateAutoApplyMode,
    updateCheckCadence
  } = props;

  const diag = accountDiagnostics;
        const account = diag?.account ?? selectedLauncherAccount;
        const uuid = diag?.minecraft_uuid ?? account?.id ?? null;
        const username = diag?.minecraft_username ?? account?.username ?? "No account connected";
        const skinTexture = toLocalIconSrc(diag?.skin_url) ?? "";
        const avatarSources = minecraftAvatarSources(uuid);
        const avatarSrc =
          toLocalIconSrc(
            avatarSources[Math.min(accountAvatarSourceIdx, Math.max(avatarSources.length - 1, 0))] ?? ""
          ) ?? "";
        const connectionRaw = String(diag?.status ?? (account ? "connected" : "not_connected")).toLowerCase();
        const tokenRaw = String(diag?.token_exchange_status ?? "idle").toLowerCase();
        const isDisconnected =
          !account ||
          connectionRaw.includes("not_connected") ||
          connectionRaw.includes("offline") ||
          connectionRaw.includes("idle");
        const isUnverified = !isDisconnected && (!diag?.entitlements_ok || tokenRaw.includes("error"));
        const accountStatusTone = isDisconnected ? "error" : isUnverified ? "warn" : "ok";
        const accountStatusLabel = isDisconnected
          ? "Not Connected"
          : isUnverified
            ? "Not verified"
            : "Connected / verified";
        const authBannerMessage = isDisconnected
          ? "Your launcher is not connected to a Microsoft account, so native launch and profile sync are unavailable."
          : isUnverified
            ? "Account connected, but entitlement verification is incomplete. Reconnect to refresh auth tokens."
            : diag?.last_error || accountDiagnosticsErr
              ? "Authentication returned an error. Reconnect to re-establish a healthy token chain."
              : null;
        const showAuthBrokenBanner = Boolean(authBannerMessage);
  
        return (
          <div className="accountPage">
            <div className="pageRouteHeader">
              <div className="pageRouteEyebrow">Account</div>
              <div className="h1">Account</div>
              <div className="p">Connection status, launcher profile details, and skin setup in one calmer workspace.</div>
            </div>
  
            <div className="accountHero card">
              <div className="accountAvatarWrap">
                {accountAvatarFromSkin ? (
                  <img src={accountAvatarFromSkin} alt="Minecraft avatar" />
                ) : skinTexture ? (
                  <span className="minecraftHeadPreview" role="img" aria-label="Minecraft avatar">
                    <img src={skinTexture} alt="" className="minecraftHeadLayer base" />
                    <img src={skinTexture} alt="" className="minecraftHeadLayer hat" />
                  </span>
                ) : avatarSrc ? (
                  <img
                    src={avatarSrc}
                    alt="Minecraft avatar"
                    onError={() => setAccountAvatarSourceIdx((i) => i + 1)}
                  />
                ) : (
                  <span>{username?.slice(0, 1)?.toUpperCase() ?? "?"}</span>
                )}
              </div>
              <div className="accountHeroMain">
                <div className="accountHeroEyebrow">Minecraft profile</div>
                <div className="accountHeroName">{username}</div>
                <div className="accountHeroMeta">
                  <span className={`accountStatusBadge tone-${accountStatusTone}`}>
                    <span className="accountStatusDot" aria-hidden="true" />
                    {accountStatusLabel}
                  </span>
                  {diag?.entitlements_ok ? <span className="chip">Owns Minecraft</span> : null}
                  {diag?.token_exchange_status ? <span className="chip subtle">{humanizeToken(diag.token_exchange_status)}</span> : null}
                </div>
                <div className="accountHeroSub">
                  UUID: {uuid ?? "Not available"}
                </div>
                <div className="accountHeroLead">
                  {isDisconnected
                    ? "Connect a Microsoft account to unlock native launch, entitlement checks, and profile sync."
                    : isUnverified
                      ? "The account is connected, but verification is still incomplete right now."
                      : "Your launcher account is connected and ready for native launch workflows."}
                </div>
                <div className="row" style={{ marginTop: 10 }}>
                  <button className="btn primary" onClick={onBeginMicrosoftLogin} disabled={launcherBusy}>
                    {msLoginSessionId ? "Waiting for login…" : "Connect / Reconnect"}
                  </button>
                  {msLoginSessionId && msCodePrompt ? (
                    <button className="btn" onClick={() => setMsCodePromptVisible(true)}>
                      Show code
                    </button>
                  ) : null}
                  <button className="btn" onClick={() => refreshAccountDiagnostics().catch(() => null)} disabled={accountDiagnosticsBusy}>
                    {accountDiagnosticsBusy ? "Refreshing…" : "Refresh diagnostics"}
                  </button>
                </div>
              </div>
            </div>
            {showAuthBrokenBanner ? (
              <div className="card accountAuthBanner">
                <div className="accountAuthBannerMain">
                  <div className="accountAuthBannerTitle">Authentication needs attention</div>
                  <div className="accountAuthBannerText">{authBannerMessage}</div>
                </div>
                <button className="btn primary" onClick={onBeginMicrosoftLogin} disabled={launcherBusy}>
                  {msLoginSessionId ? "Waiting for login…" : "Reconnect"}
                </button>
              </div>
            ) : null}
  
            <div className="accountSummaryStrip">
              <div className="accountSummaryCard">
                <div className="accountSummaryLabel">Launch mode</div>
                <div className="accountSummaryValue">{humanizeToken(launcherSettings?.default_launch_method ?? "native")}</div>
              </div>
              <div className="accountSummaryCard">
                <div className="accountSummaryLabel">Update checks</div>
                <div className="accountSummaryValue">{updateCadenceLabel(updateCheckCadence)}</div>
              </div>
              <div className="accountSummaryCard">
                <div className="accountSummaryLabel">Saved skins</div>
                <div className="accountSummaryValue">{savedSkinOptions.length}</div>
              </div>
              <div className="accountSummaryCard">
                <div className="accountSummaryLabel">Connected accounts</div>
                <div className="accountSummaryValue">{launcherAccounts.length}</div>
              </div>
            </div>
  
            <div className="accountGrid">
              <div className="card accountCard accountCardWide">
                <div className="settingTitle">Profile overview</div>
                <div className="settingSub">The account you are using right now, plus the launcher and skin defaults attached to it.</div>
                <div className="accountProfileSplit">
                  <div className="accountSectionBlock">
                    <div className="accountSectionTitle">Launcher defaults</div>
                    <div className="accountDiagList">
                      <div className="accountDiagRow">
                        <span>Default launch mode</span>
                        <strong>{humanizeToken(launcherSettings?.default_launch_method ?? "native")}</strong>
                      </div>
                      <div className="accountDiagRow">
                        <span>Update checks</span>
                        <strong>{updateCadenceLabel(updateCheckCadence)}</strong>
                      </div>
                      <div className="accountDiagRow">
                        <span>Automatic installs</span>
                        <strong>{updateAutoApplyModeLabel(updateAutoApplyMode)}</strong>
                      </div>
                      <div className="accountDiagRow">
                        <span>Current skin</span>
                        <strong>{selectedAccountSkin?.label ?? "None"}</strong>
                      </div>
                      <div className="accountDiagRow">
                        <span>Current cape</span>
                        <strong>{selectedAccountCape?.label ?? "No cape"}</strong>
                      </div>
                    </div>
                  </div>
                  <div className="accountSectionBlock">
                    <div className="accountSectionTitle">Skin setup</div>
                    <div className="accountDiagList">
                      <div className="accountDiagRow">
                        <span>Saved skins</span>
                        <strong>{savedSkinOptions.length}</strong>
                      </div>
                      <div className="accountDiagRow">
                        <span>Default skins</span>
                        <strong>{defaultSkinOptions.length}</strong>
                      </div>
                      <div className="accountDiagRow">
                        <span>Cape options</span>
                        <strong>{capeOptions.length}</strong>
                      </div>
                      <div className="accountDiagRow">
                        <span>Last diagnostics refresh</span>
                        <strong>{diag?.last_refreshed_at ? new Date(diag.last_refreshed_at).toLocaleString() : "Never"}</strong>
                      </div>
                    </div>
                    <div className="accountSectionActions">
                      <button className="btn" onClick={() => setRoute("skins")}>
                        Open skin studio
                      </button>
                      <label className="toggleRow accountInlineToggle">
                        <input
                          type="checkbox"
                          checked={skinPreviewEnabled}
                          onChange={(event) => setSkinPreviewEnabled(event.target.checked)}
                        />
                        <span className="togglePill" />
                        <span>Use 3D preview in Skin Studio</span>
                      </label>
                    </div>
                  </div>
                </div>
              </div>
  
              <div className="card accountCard accountCardWide">
                <div className="settingTitle">Diagnostics</div>
                <div className="settingSub">Connection health and token state for native launch. Network errors can make these checks look worse than the account really is.</div>
                <div className="accountDiagList">
                  <div className="accountDiagRow">
                    <span>Connection</span>
                    <strong className={`accountStatusText tone-${accountStatusTone}`}>{accountStatusLabel}</strong>
                  </div>
                  <div className="accountDiagRow">
                    <span>Entitlements</span>
                    <strong className={`accountStatusText tone-${diag?.entitlements_ok ? "ok" : "warn"}`}>
                      {diag?.entitlements_ok ? "Verified" : "Not verified"}
                    </strong>
                  </div>
                  <div className="accountDiagRow">
                    <span>Token status</span>
                    <strong>{humanizeToken(diag?.token_exchange_status ?? "idle")}</strong>
                  </div>
                  <div className="accountDiagRow">
                    <span>Client ID source</span>
                    <strong>{humanizeToken(diag?.client_id_source ?? "unknown")}</strong>
                  </div>
                  <div className="accountDiagRow">
                    <span>Last refresh</span>
                    <strong>{diag?.last_refreshed_at ? formatDateTime(diag.last_refreshed_at, "Never") : "Never"}</strong>
                  </div>
                  {diag?.last_error ? (
                    <div className="errorBox" style={{ marginTop: 8 }}>{diag.last_error}</div>
                  ) : null}
                  {accountDiagnosticsErr ? (
                    <div className="errorBox" style={{ marginTop: 8 }}>{accountDiagnosticsErr}</div>
                  ) : null}
                </div>
              </div>
  
              <div className="card accountCard">
                <div className="settingTitle">Accounts</div>
                <div className="settingSub">Choose which connected account should be used for native launch.</div>
                <div className="accountAccountsList">
                  {launcherAccounts.length === 0 ? (
                    <div className="muted">No connected accounts.</div>
                  ) : (
                    launcherAccounts.map((acct) => {
                      const selectedAccount = selectedLauncherAccountId === acct.id;
                      return (
                        <div key={acct.id} className="accountAccountRow">
                          <div className="accountAccountInfo">
                            <div className="accountAccountName">{acct.username}</div>
                            <div className="accountAccountId">{acct.id}</div>
                          </div>
                          <div className="row" style={{ marginTop: 0 }}>
                            <button
                              className={`btn stateful ${selectedAccount ? "active" : ""}`}
                              onClick={() => onSelectAccount(acct.id)}
                              disabled={launcherBusy}
                            >
                              {selectedAccount ? "Selected" : "Use"}
                            </button>
                            <button
                              className="btn accountDisconnectBtn"
                              onClick={() => onLogoutAccount(acct.id)}
                              disabled={launcherBusy}
                            >
                              Disconnect
                            </button>
                          </div>
                        </div>
                      );
                    })
                  )}
                </div>
              </div>
  
              <div className="card accountCard">
                <div className="settingTitle">Profile assets</div>
                <div className="settingSub">Skins and capes currently returned by the Minecraft profile API.</div>
                <div className="accountDiagList">
                  <div className="accountDiagRow">
                    <span>Skins</span>
                    <strong>{diag?.skins?.length ?? 0}</strong>
                  </div>
                  <div className="accountDiagRow">
                    <span>Capes</span>
                    <strong>{diag?.cape_count ?? 0}</strong>
                  </div>
                  <div className="accountAssetList">
                    {(diag?.skins ?? []).slice(0, 6).map((skin) => (
                      <div key={`${skin.id}:${skin.url}`} className="accountAssetRow">
                        <span>{skin.variant ?? "Skin"}</span>
                        <a href={skin.url} target="_blank" rel="noreferrer">Open</a>
                      </div>
                    ))}
                    {(diag?.capes ?? []).slice(0, 6).map((cape) => (
                      <div key={`${cape.id}:${cape.url}`} className="accountAssetRow">
                        <span>{cape.alias ?? "Cape"}</span>
                        <a href={cape.url} target="_blank" rel="noreferrer">Open</a>
                      </div>
                    ))}
                  </div>
                </div>
              </div>
            </div>
          </div>
        );
}
