import { useEffect, useState } from "react";
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
  onCancelMicrosoftLogin: any;
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
    onCancelMicrosoftLogin,
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
  const [uuidCopied, setUuidCopied] = useState(false);

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
  const tokenStatusLabel = humanizeToken(diag?.token_exchange_status ?? "idle");
  const lastRefreshedAt = parseDateLike(diag?.last_refreshed_at);
  const hasEntitlement = Boolean(diag?.entitlements_ok);
  const hasDiagnosticsError = Boolean(diag?.last_error || accountDiagnosticsErr);
  const isWaitingForLogin = Boolean(msLoginSessionId);
  const isDisconnected =
    !account ||
    connectionRaw.includes("not_connected") ||
    connectionRaw.includes("offline") ||
    connectionRaw.includes("idle");
  const isUnverified = !isDisconnected && (!hasEntitlement || tokenRaw.includes("error") || hasDiagnosticsError);
  const cardState = isWaitingForLogin
    ? "waiting"
    : isDisconnected
      ? "disconnected"
      : isUnverified
        ? "attention"
        : "ready";
  const lastCheckedLabel = lastRefreshedAt
    ? relativeTimeFromMs(lastRefreshedAt.getTime())
    : "Not checked yet";
  const lastCheckedTitle = lastRefreshedAt
    ? formatDateTime(diag?.last_refreshed_at, "Never")
    : "Never";
  const shortUuid =
    typeof uuid === "string" && uuid.length > 18
      ? `${uuid.slice(0, 8)}…${uuid.slice(-8)}`
      : (uuid ?? "Not available");
  const showTokenMetaChip = Boolean(diag?.token_exchange_status) && tokenRaw !== "ok" && tokenRaw !== "idle";

  const statusTone =
    cardState === "ready"
      ? "ok"
      : cardState === "attention"
        ? "warn"
        : cardState === "disconnected"
          ? "error"
          : "pending";
  const statusBadgeLabel =
    cardState === "waiting"
      ? "Waiting for login"
      : cardState === "disconnected"
        ? "Not connected"
        : cardState === "attention"
          ? "Needs attention"
          : "Connected";
  const statusTitle =
    cardState === "waiting"
      ? "Finish Microsoft sign-in"
      : cardState === "disconnected"
        ? "Connect your Microsoft account"
        : cardState === "attention"
          ? "Reconnect to restore a healthy session"
          : "Ready for native launch";
  const statusDescription =
    cardState === "waiting"
      ? (msCodePrompt
          ? "Finish the Microsoft sign-in flow with the device code, then come back here to continue."
          : "Complete the Microsoft sign-in flow in your browser to finish connecting this launcher profile.")
      : cardState === "disconnected"
        ? "Native launch, entitlement checks, and profile sync will stay unavailable until a Microsoft account is connected."
        : cardState === "attention"
          ? (diag?.last_error || accountDiagnosticsErr
              ? "Authentication hit an error during the last refresh. Reconnect to renew the token chain and re-run verification."
              : "The account is connected, but ownership or token verification is still incomplete right now.")
          : "This profile is connected, verified, and ready for entitlement checks, profile sync, and native launch.";
  const statusSecondaryFacts = [
    cardState === "ready" && hasEntitlement ? "Owns Minecraft" : null,
    cardState === "attention" && !hasEntitlement ? "Ownership not verified yet" : null,
    cardState === "attention" && showTokenMetaChip ? tokenStatusLabel : null,
  ].filter(Boolean) as string[];
  const healthSummaryLabel =
    cardState === "ready"
      ? "Healthy"
      : cardState === "waiting"
        ? "Waiting for sign-in"
        : cardState === "attention"
          ? "Needs recovery"
          : "Not connected";
  const healthSummaryText =
    cardState === "ready"
      ? "Diagnostics are passing and the profile is ready for use."
      : cardState === "waiting"
        ? "Finish the Microsoft sign-in flow to complete setup."
        : cardState === "attention"
          ? "Refresh or reconnect to restore a healthy token chain."
          : "Connect Microsoft to enable native launch and sync.";
  const primaryActionLabel =
    cardState === "waiting"
      ? (msCodePrompt ? "Show code" : "Waiting for browser…")
      : cardState === "disconnected"
        ? "Connect Microsoft"
        : cardState === "attention"
          ? "Reconnect"
          : (accountDiagnosticsBusy ? "Checking…" : "Refresh diagnostics");
  const primaryActionClass =
    cardState === "ready"
      ? "btn subtle"
      : "btn primary";
  const primaryActionDisabled =
    launcherBusy ||
    (cardState === "waiting" && !msCodePrompt) ||
    (cardState === "ready" && accountDiagnosticsBusy);

  function handlePrimaryAction() {
    if (cardState === "waiting") {
      if (msCodePrompt) setMsCodePromptVisible(true);
      return;
    }
    if (cardState === "ready") {
      void refreshAccountDiagnostics().catch(() => null);
      return;
    }
    onBeginMicrosoftLogin();
  }
  
  useEffect(() => {
    if (!uuidCopied) return;
    const timer = window.setTimeout(() => setUuidCopied(false), 1400);
    return () => window.clearTimeout(timer);
  }, [uuidCopied]);

  async function copyUuid() {
    if (!uuid) return;
    try {
      await navigator.clipboard.writeText(uuid);
      setUuidCopied(true);
    } catch {
      setUuidCopied(false);
    }
  }
  
  return (
    <div className="accountPage">
      <div className="pageRouteHeader">
        <div className="pageRouteEyebrow">Account</div>
        <div className="h1">Account</div>
        <div className="p">Connection status, launcher profile details, and skin setup in one calmer workspace.</div>
      </div>
  
      <div className="accountHeroSplit">
        <div className="card accountProfileCard">
          <div className="accountHeroContent">
            <div className="accountHeroHeader">
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
              <div className="accountHeroIdentity">
                <div className="accountHeroEyebrow">Minecraft profile</div>
                <div className="accountHeroName">{username}</div>
                <div className="accountHeroMeta">
                  <span className={`accountMetaPill accountStatusBadge tone-${statusTone}`}>
                    <span className="accountStatusDot" aria-hidden="true" />
                    {statusBadgeLabel}
                  </span>
                  {statusSecondaryFacts.length ? (
                    <div className="accountHeroSecondaryFacts" aria-label="Account details">
                      {statusSecondaryFacts.map((fact) => (
                        <span key={fact} className="accountMetaPill accountHeroSecondaryFact">
                          {fact}
                        </span>
                      ))}
                    </div>
                  ) : null}
                </div>
              </div>
            </div>
            <div className="accountHeroStatusBlock">
              <div className="accountHeroStatusTitle">{statusTitle}</div>
              <div className="accountHeroLead">{statusDescription}</div>
            </div>
          </div>
          <div className="accountHeroInfoRow">
            {uuid ? (
              <div className="accountHeroInfoItem accountHeroInfoItemPrimary" title={uuid}>
                <span className="accountHeroInfoLabel">UUID</span>
                <span className="accountHeroInfoValue accountHeroInfoValueStrong">{shortUuid}</span>
                <button
                  className={`btn ghost accountHeroCopyBtn ${uuidCopied ? "isCopied" : ""}`}
                  onClick={() => void copyUuid()}
                  type="button"
                >
                  {uuidCopied ? "Copied" : "Copy"}
                </button>
              </div>
            ) : null}
          </div>
        </div>

        <div className="card accountCard accountHealthCard">
          <div className="accountHealthHeader">
            <div className="accountHeroUtilityLabel">Health</div>
            <div className={`accountHeroUtilityValue tone-${statusTone}`}>{healthSummaryLabel}</div>
          </div>
          <div className="accountHeroUtilityText">{healthSummaryText}</div>
          <div className="accountHealthMeta">
            <div className="accountHeroUtilityLabel">Last check</div>
            <div className="accountHeroUtilityValue">{lastCheckedLabel}</div>
            <div className="accountHeroUtilityText" title={lastCheckedTitle}>
              {lastCheckedTitle}
            </div>
          </div>
          <div className={`accountHeroActions ${cardState === "ready" ? "isQuiet" : ""}`}>
            <button
              className={primaryActionClass}
              onClick={handlePrimaryAction}
              disabled={primaryActionDisabled}
              type="button"
            >
              {primaryActionLabel}
            </button>
            {cardState === "attention" ? (
              <button
                className="btn ghost"
                onClick={onBeginMicrosoftLogin}
                disabled={launcherBusy}
                type="button"
              >
                Reconnect anyway
              </button>
            ) : null}
            {cardState === "waiting" ? (
              <button
                className="btn ghost"
                onClick={() => refreshAccountDiagnostics().catch(() => null)}
                disabled={accountDiagnosticsBusy}
                type="button"
              >
                {accountDiagnosticsBusy ? "Checking…" : "Check status"}
              </button>
            ) : null}
            {cardState === "waiting" ? (
              <button
                className="btn ghost"
                onClick={onCancelMicrosoftLogin}
                disabled={launcherBusy}
                type="button"
              >
                Cancel sign-in
              </button>
            ) : null}
            {cardState === "attention" ? (
              <button
                className="btn ghost"
                onClick={() => refreshAccountDiagnostics().catch(() => null)}
                disabled={accountDiagnosticsBusy}
                type="button"
              >
                {accountDiagnosticsBusy ? "Checking…" : "Check again"}
              </button>
            ) : null}
          </div>
          {hasDiagnosticsError ? (
            <div className="accountHealthFoot">
              <span className="accountHeroInfoLabel">Status</span>
              <span className="accountHeroInfoValue">Refresh needs attention</span>
            </div>
          ) : null}
        </div>
      </div>
  
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
                    <strong className={`accountStatusText tone-${statusTone}`}>{statusBadgeLabel}</strong>
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
                <div className="accountSectionHeader">
                  <div className="accountSectionHeaderCopy">
                    <div className="settingTitle">Accounts</div>
                    <div className="settingSub">Choose which connected account should be used for native launch.</div>
                    {msLoginSessionId ? (
                      <div className="accountSectionHint">
                        Microsoft sign-in is already in progress.
                      </div>
                    ) : null}
                  </div>
                  <div className="accountSectionHeaderActions">
                    {msLoginSessionId && msCodePrompt ? (
                      <button
                        className="btn subtle"
                        onClick={() => setMsCodePromptVisible(true)}
                        type="button"
                      >
                        Show code
                      </button>
                    ) : null}
                    {msLoginSessionId ? (
                      <button
                        className="btn ghost"
                        onClick={onCancelMicrosoftLogin}
                        disabled={launcherBusy}
                        type="button"
                      >
                        Cancel
                      </button>
                    ) : null}
                    <button
                      className="btn subtle"
                      onClick={onBeginMicrosoftLogin}
                      disabled={launcherBusy || Boolean(msLoginSessionId)}
                      type="button"
                    >
                      {msLoginSessionId ? "Waiting for login…" : "Add account"}
                    </button>
                  </div>
                </div>
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
