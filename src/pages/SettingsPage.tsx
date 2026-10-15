import { Eye, EyeOff, FolderOpen, History, RotateCcw } from "lucide-react";
import { useState, type FormEvent } from "react";

import { Button, IconButton } from "../components/ui/Button";
import { ExternalLink } from "../components/ui/ExternalLink";
import { Notice } from "../components/ui/Notice";
import { PageHeader } from "../components/ui/PageHeader";
import { LoadingBlock } from "../components/ui/Spinner";
import { Switch } from "../components/ui/Switch";
import { TextField } from "../components/ui/TextField";
import { useLaunchAtLogin, useSetLaunchAtLogin } from "../hooks/useBackground";
import { useDesktopBackup, useRestoreDesktop } from "../hooks/useTheme";
import { useCacheStats, useSetCacheDir, useSettings, useUpdateProviderSettings } from "../hooks/useSettings";
import { chooseFolder, confirmRestoreDesktop } from "../services/dialogs";
import { errorMessage } from "../services/errors";
import { formatBytes, pluralize } from "../services/format";
import { accentLabel, colorSchemeLabel } from "../services/theme";
import { notify } from "../store/ui";
import type { ProviderSettings, Settings } from "../types";

export function SettingsPage() {
  const settings = useSettings();

  return (
    <>
      <PageHeader title="Settings" />
      {settings.isPending ? (
        <LoadingBlock label="Loading settings" />
      ) : settings.isError ? (
        <Notice>{errorMessage(settings.error)}</Notice>
      ) : (
        <div className="flex max-w-3xl flex-col gap-12">
          <BackgroundSettings />
          <DesktopBackupSettings />
          <CacheSettings settings={settings.data} />
          <section aria-labelledby="providers-title">
            <h2 id="providers-title" className="text-lg font-semibold">
              Providers
            </h2>
            <p className="mt-1 text-muted">
              Choose where Kankulirchi searches for wallpapers. Safe search is always on, so adult and suggestive
              wallpapers are left out.
            </p>
            <ul className="mt-5 flex flex-col gap-4">
              {settings.data.providers.map((provider) => (
                <ProviderSettingsCard key={provider.id} provider={provider} />
              ))}
            </ul>
          </section>
        </div>
      )}
    </>
  );
}

function BackgroundSettings() {
  const launchAtLogin = useLaunchAtLogin();
  const setLaunchAtLogin = useSetLaunchAtLogin();

  return (
    <section aria-labelledby="background-title">
      <h2 id="background-title" className="text-lg font-semibold">
        Background
      </h2>
      <p className="mt-1 text-muted">
        While a rotation is on, closing the window keeps Kankulirchi running, so your wallpaper keeps changing. Run{" "}
        <code className="rounded bg-raised px-1.5 py-0.5 text-[13px] text-fg">kankulirchi</code> to open the window
        again, or choose Quit in the sidebar to stop it.
      </p>

      <div className="mt-5 flex items-start justify-between gap-4 rounded-panel border border-line bg-surface/70 p-5">
        <div>
          <h3 className="font-semibold">Start at login</h3>
          <p className={launchAtLogin.isError ? "text-[13px] text-danger" : "text-[13px] text-muted"}>
            {launchAtLogin.isError
              ? errorMessage(launchAtLogin.error)
              : "Starts Kankulirchi in the background when you log in. It stays running only while a rotation is on."}
          </p>
        </div>
        <Switch
          label="Start at login"
          hideLabel
          checked={launchAtLogin.data ?? false}
          disabled={!launchAtLogin.isSuccess || setLaunchAtLogin.isPending}
          onChange={(enabled) => setLaunchAtLogin.mutate(enabled)}
        />
      </div>
    </section>
  );
}

const backupDate = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" });

function DesktopBackupSettings() {
  const backup = useDesktopBackup();
  const restore = useRestoreDesktop();

  async function confirmAndRestore() {
    if (await confirmRestoreDesktop()) restore.mutate();
  }

  return (
    <section aria-labelledby="desktop-backup-title">
      <h2 id="desktop-backup-title" className="text-lg font-semibold">
        Desktop backup
      </h2>
      <p className="mt-1 text-muted">
        Before Kankulirchi first changes your desktop, it saves your wallpaper, lock screen, accent color and style, so you
        can put them back.
      </p>
      <div className="mt-5 rounded-panel border border-line bg-surface/70 p-5">
        {backup.isPending ? (
          <p className="text-muted">Loading…</p>
        ) : backup.isError ? (
          <p className="text-danger">{errorMessage(backup.error)}</p>
        ) : backup.data === null ? (
          <p className="text-muted">Nothing is saved yet: Kankulirchi has not changed your desktop.</p>
        ) : (
          <div className="flex flex-col gap-4">
            <dl className="grid grid-cols-[max-content_1fr] gap-x-6 gap-y-1.5">
              <dt className="text-muted">Saved</dt>
              <dd>{backupDate.format(new Date(backup.data.takenAt))}</dd>
              {backup.data.wallpaper !== undefined && (
                <>
                  <dt className="text-muted">Wallpaper</dt>
                  <dd data-selectable className="break-all">
                    {backup.data.wallpaper ?? "GNOME's default"}
                  </dd>
                </>
              )}
              {backup.data.accentColor && (
                <>
                  <dt className="text-muted">Accent color</dt>
                  <dd>{accentLabel(backup.data.accentColor)}</dd>
                </>
              )}
              {backup.data.colorScheme && (
                <>
                  <dt className="text-muted">Style</dt>
                  <dd>{colorSchemeLabel(backup.data.colorScheme)}</dd>
                </>
              )}
            </dl>
            {backup.data.wallpaper === null && (
              <p className="text-[13px] text-muted">
                Your wallpaper from before Kankulirchi was not recorded, so restoring sets GNOME's default wallpaper.
              </p>
            )}
            <span>
              <Button icon={<History className="size-4" />} busy={restore.isPending} onClick={() => void confirmAndRestore()}>
                Restore desktop
              </Button>
            </span>
          </div>
        )}
      </div>
    </section>
  );
}

function CacheSettings({ settings }: { settings: Settings }) {
  const stats = useCacheStats();
  const setCacheDir = useSetCacheDir();
  const isDefault = settings.cacheDir === settings.defaultCacheDir;

  async function choose() {
    try {
      const folder = await chooseFolder(settings.cacheDir);
      if (folder && folder !== settings.cacheDir) setCacheDir.mutate(folder);
    } catch (error) {
      notify(errorMessage(error), "error");
    }
  }

  return (
    <section aria-labelledby="cache-title">
      <h2 id="cache-title" className="text-lg font-semibold">
        Cache
      </h2>
      <p className="mt-1 text-muted">Downloaded wallpapers are kept here so they can be applied offline.</p>

      <dl className="mt-5 divide-y divide-line/70 rounded-panel border border-line bg-surface/70">
        <div className="flex flex-col gap-3 p-5">
          <dt className="font-medium">Cache location</dt>
          <dd className="flex flex-col gap-3">
            <span data-selectable className="break-all text-muted">
              {settings.cacheDir}
            </span>
            <span className="flex flex-wrap gap-2">
              <Button icon={<FolderOpen className="size-4" />} busy={setCacheDir.isPending} onClick={() => void choose()}>
                Choose folder
              </Button>
              {!isDefault && (
                <Button variant="ghost" icon={<RotateCcw className="size-4" />} onClick={() => setCacheDir.mutate(null)}>
                  Use default location
                </Button>
              )}
            </span>
            <span className="text-[13px] text-muted">
              New downloads go to this folder. Wallpapers you already downloaded stay where they are.
            </span>
          </dd>
        </div>
        <div className="flex items-baseline justify-between gap-4 p-5">
          <dt className="font-medium">Cache size</dt>
          <dd className="text-muted">
            {stats.isPending
              ? "Calculating…"
              : stats.isError
                ? errorMessage(stats.error)
                : `${formatBytes(stats.data.totalBytes)} in ${pluralize(stats.data.fileCount, "file")}`}
          </dd>
        </div>
      </dl>
    </section>
  );
}

function providerSummary(provider: ProviderSettings): { text: string; warn: boolean } {
  if (!provider.enabled) return { text: "Turned off. Searches skip this provider.", warn: false };
  if (provider.apiKeyRequirement === "required" && !provider.hasApiKey) {
    return { text: "Needs an API key before it can be searched.", warn: true };
  }
  return { text: "Ready to search.", warn: false };
}

function ProviderSettingsCard({ provider }: { provider: ProviderSettings }) {
  const update = useUpdateProviderSettings();
  const [apiKey, setApiKey] = useState(provider.apiKey ?? "");
  const [revealed, setRevealed] = useState(false);
  const summary = providerSummary(provider);
  const changed = apiKey.trim() !== (provider.apiKey ?? "");
  const inputId = `api-key-${provider.id}`;

  function saveKey(event: FormEvent) {
    event.preventDefault();
    const key = apiKey.trim() || null;
    update.mutate(
      { providerId: provider.id, enabled: provider.enabled, apiKey: key },
      { onSuccess: () => notify(key ? `${provider.name} API key saved` : `${provider.name} API key removed`) },
    );
  }

  return (
    <li className="rounded-panel border border-line bg-surface/70 p-5">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h3 className="font-semibold">{provider.name}</h3>
          <p className={summary.warn ? "text-[13px] text-danger" : "text-[13px] text-muted"}>{summary.text}</p>
        </div>
        <Switch
          label={`Search ${provider.name}`}
          hideLabel
          checked={provider.enabled}
          disabled={update.isPending}
          onChange={(enabled) => update.mutate({ providerId: provider.id, enabled, apiKey: provider.apiKey })}
        />
      </div>

      {provider.apiKeyRequirement !== "none" && (
        <form onSubmit={saveKey} className="mt-5 flex flex-col gap-2">
          <label htmlFor={inputId} className="text-[13px] text-muted">
            {provider.apiKeyRequirement === "required" ? "API key" : "API key (optional)"}
          </label>
          <div className="flex gap-2">
            <TextField
              id={inputId}
              type={revealed ? "text" : "password"}
              value={apiKey}
              autoComplete="off"
              spellCheck={false}
              placeholder={provider.apiKeyRequirement === "required" ? "Paste your API key" : "Not set"}
              onChange={(event) => setApiKey(event.target.value)}
              className="w-full max-w-md"
            />
            <IconButton label={revealed ? "Hide API key" : "Show API key"} onClick={() => setRevealed(!revealed)}>
              {revealed ? <EyeOff className="size-4" /> : <Eye className="size-4" />}
            </IconButton>
            <Button type="submit" busy={update.isPending} disabled={!changed}>
              Save key
            </Button>
          </div>
          {provider.apiKeyUrl && (
            <p className="text-[13px] text-muted">
              {provider.apiKeyRequirement === "required" ? "Get a free key at " : "Find your key in "}
              <ExternalLink href={provider.apiKeyUrl}>{new URL(provider.apiKeyUrl).host}</ExternalLink>.
            </p>
          )}
        </form>
      )}
      {provider.attribution && <p className="mt-4 text-[13px] text-muted">{provider.attribution}.</p>}
    </li>
  );
}
