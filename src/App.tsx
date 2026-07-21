import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import "./App.css";

type Tab = "play" | "install" | "mods" | "account" | "settings";

type VersionInfo = {
  id: string;
  type: string;
  url: string;
  releaseTime: string;
};

type VersionManifest = {
  latest: { release: string; snapshot: string };
  versions: VersionInfo[];
};

type Account = {
  uuid: string;
  name: string;
  access_token: string;
  offline: boolean;
};

type Settings = {
  memory_mb: number;
  java_path: string | null;
  accounts: Account[];
  active_account: string | null;
  last_version: string | null;
  width?: number;
  height?: number;
  fullscreen?: boolean;
  jvm_args?: string;
};

type ProgressEvent = {
  stage: string;
  current: number;
  total: number;
  message: string;
};

type FabricLoader = { version: string; stable: boolean };
type ForgeEntry = { raw: string; mc: string; forge: string };
type ModHit = {
  project_id: string;
  slug: string;
  title: string;
  description: string;
  downloads: number;
  icon_url?: string;
};
type ModVersion = {
  id: string;
  name: string;
  version_number: string;
  loaders: string[];
  files: { url: string; filename: string; primary: boolean }[];
};

const TABS: { id: Tab; label: string; short: string }[] = [
  { id: "play", label: "Spelen", short: "Spel" },
  { id: "install", label: "Installeren", short: "Inst" },
  { id: "mods", label: "Mods", short: "Mods" },
  { id: "account", label: "Account", short: "Acc" },
  { id: "settings", label: "Instellingen", short: "Set" },
];

function NavGlyph({ id }: { id: Tab }) {
  const common = {
    width: 14,
    height: 14,
    viewBox: "0 0 14 14",
    fill: "none",
    className: "nav-icon",
    "aria-hidden": true as const,
  };
  switch (id) {
    case "play":
      return (
        <svg {...common}>
          <path d="M3 2.5 11 7 3 11.5Z" stroke="currentColor" strokeWidth="1.4" strokeLinejoin="round" />
        </svg>
      );
    case "install":
      return (
        <svg {...common}>
          <path d="M7 2v7M4.5 7.5 7 10l2.5-2.5M3 12h8" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      );
    case "mods":
      return (
        <svg {...common}>
          <path d="M3 4h3v3H3zM8 4h3v3H8zM3 9h3v3H3zM8 9h3v3H8z" stroke="currentColor" strokeWidth="1.3" />
        </svg>
      );
    case "account":
      return (
        <svg {...common}>
          <circle cx="7" cy="5" r="2.2" stroke="currentColor" strokeWidth="1.3" />
          <path d="M2.5 12c.8-2.2 2.4-3.3 4.5-3.3S10.7 9.8 11.5 12" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
        </svg>
      );
    default:
      return (
        <svg {...common}>
          <circle cx="7" cy="7" r="2" stroke="currentColor" strokeWidth="1.3" />
          <path d="M7 1.5v1.6M7 10.9v1.6M1.5 7h1.6M10.9 7h1.6M3.1 3.1l1.1 1.1M9.8 9.8l1.1 1.1M10.9 3.1 9.8 4.2M4.2 9.8 3.1 10.9" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
        </svg>
      );
  }
}

function PlateGeometry() {
  return (
    <svg className="plate-geometry" viewBox="0 0 88 96" fill="none" aria-hidden>
      <path d="M12 28 L44 8 L76 28 L76 60 L44 84 L12 60 Z" stroke="rgba(232,168,106,0.45)" strokeWidth="1.2" />
      <path d="M44 8 L76 28 L44 44 L12 28 Z" fill="rgba(232,168,106,0.06)" />
      <path d="M12 28 L44 44 L44 84 L12 60 Z" fill="rgba(0,0,0,0.2)" />
      <path d="M28 40 L42 48 L38 58 L52 64" stroke="#E8A86A" strokeWidth="1.3" strokeLinecap="round" opacity="0.7" />
      <circle cx="52" cy="64" r="2" fill="#E8A86A" opacity="0.8" />
    </svg>
  );
}

function instanceMeta(id: string) {
  const lower = id.toLowerCase();
  if (lower.includes("fabric")) return { loader: "Fabric", kind: "Modded" };
  if (lower.includes("forge")) return { loader: "Forge", kind: "Modded" };
  if (!id) return { loader: "—", kind: "Geen instance" };
  return { loader: "Vanilla", kind: "Release" };
}

function SkinAvatar({
  account,
  sizeClass = "skin",
}: {
  account: Account | null | undefined;
  sizeClass?: string;
}) {
  const [src, setSrc] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setSrc(null);
    if (!account || account.offline) return;

    invoke<string>("get_player_skin", { uuid: account.uuid })
      .then((dataUrl) => {
        if (!cancelled) setSrc(dataUrl);
      })
      .catch(() => {
        if (!cancelled) setSrc(null);
      });

    return () => {
      cancelled = true;
    };
  }, [account?.uuid, account?.offline]);

  if (!account) {
    return <div className={`${sizeClass} placeholder`} aria-hidden>—</div>;
  }

  if (account.offline || !src) {
    return (
      <div className={`${sizeClass} placeholder`} aria-hidden>
        {account.name.slice(0, 1).toUpperCase()}
      </div>
    );
  }

  return (
    <img
      className={`${sizeClass} skin-img`}
      src={src}
      alt=""
      width={64}
      height={64}
      draggable={false}
    />
  );
}

async function tryInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T | null> {
  try {
    return await invoke<T>(cmd, args);
  } catch {
    return null;
  }
}

/** Invoke a command that returns void/`()` — success is no throw. */
async function invokeOk(cmd: string, args?: Record<string, unknown>): Promise<boolean> {
  try {
    await invoke(cmd, args);
    return true;
  } catch {
    return false;
  }
}

export default function App() {
  const [tab, setTab] = useState<Tab>("play");
  const [manifest, setManifest] = useState<VersionManifest | null>(null);
  const [installed, setInstalled] = useState<string[]>([]);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [selected, setSelected] = useState<string>("");
  const [status, setStatus] = useState("");
  const [statusError, setStatusError] = useState(false);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<ProgressEvent | null>(null);
  const [javaPath, setJavaPath] = useState<string | null>(null);

  const [installMc, setInstallMc] = useState("");
  const [loader, setLoader] = useState<"vanilla" | "fabric" | "forge">("vanilla");
  const [fabricLoaders, setFabricLoaders] = useState<FabricLoader[]>([]);
  const [fabricPick, setFabricPick] = useState("");
  const [forgeList, setForgeList] = useState<ForgeEntry[]>([]);
  const [forgePick, setForgePick] = useState("");

  const [offlineName, setOfflineName] = useState("");
  const [deviceMsg, setDeviceMsg] = useState<string | null>(null);
  const [deviceCode, setDeviceCode] = useState<string | null>(null);
  const [deviceUri, setDeviceUri] = useState<string | null>(null);
  const [confirmAction, setConfirmAction] = useState<{
    title: string;
    body: string;
    onConfirm: () => Promise<void>;
  } | null>(null);

  const [modQuery, setModQuery] = useState("");
  const [modHits, setModHits] = useState<ModHit[]>([]);
  const [instanceMods, setInstanceMods] = useState<string[]>([]);

  const [resW, setResW] = useState(1280);
  const [resH, setResH] = useState(720);
  const [fullscreen, setFullscreen] = useState(false);
  const [jvmArgs, setJvmArgs] = useState("");

  const activeAccount = useMemo(() => {
    if (!settings) return null;
    return (
      settings.accounts.find((a) => a.uuid === settings.active_account) ??
      settings.accounts[settings.accounts.length - 1] ??
      null
    );
  }, [settings]);

  const showStatus = useCallback((msg: string, isError = false) => {
    setStatus(msg);
    setStatusError(isError);
  }, []);

  const refresh = useCallback(async () => {
    const [m, inst, s, java] = await Promise.all([
      invoke<VersionManifest>("get_version_manifest"),
      invoke<string[]>("get_installed_versions"),
      invoke<Settings>("get_settings"),
      invoke<{ path: string | null; found: boolean }>("get_java_info"),
    ]);
    setManifest(m);
    setInstalled(inst);
    setSettings(s);
    setJavaPath(java.path);
    if (typeof s.width === "number") setResW(s.width);
    if (typeof s.height === "number") setResH(s.height);
    if (typeof s.fullscreen === "boolean") setFullscreen(s.fullscreen);
    if (typeof s.jvm_args === "string") setJvmArgs(s.jvm_args);
    const initial =
      s.last_version && inst.includes(s.last_version)
        ? s.last_version
        : inst[0] ?? m.latest.release;
    setSelected((prev) => prev || initial);
    setInstallMc((prev) => prev || m.latest.release);
  }, []);

  useEffect(() => {
    refresh().catch((e) => showStatus(String(e), true));
    const unlisten = listen<ProgressEvent>("install-progress", (e) => {
      setProgress(e.payload);
      showStatus(e.payload.message);
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, [refresh, showStatus]);

  useEffect(() => {
    if (!selected) return;
    invoke<string[]>("list_mods", { instanceId: selected })
      .then(setInstanceMods)
      .catch(() => setInstanceMods([]));
  }, [selected]);

  const releases = useMemo(
    () =>
      manifest?.versions.filter((v) => v.type === "release").slice(0, 40) ?? [],
    [manifest],
  );

  async function persistSettings(next: Settings) {
    await invoke("update_settings", { settings: next });
    setSettings(next);
  }

  async function onLaunch() {
    if (!selected) return;
    setBusy(true);
    showStatus("Minecraft starten…");
    try {
      const msg = await invoke<string>("launch_instance", { versionId: selected });
      if (settings) {
        await persistSettings({ ...settings, last_version: selected });
      }
      showStatus(msg);
    } catch (e) {
      showStatus(String(e), true);
    } finally {
      setBusy(false);
    }
  }

  async function onInstall() {
    if (!manifest || !installMc) return;
    const info = manifest.versions.find((v) => v.id === installMc);
    if (!info) {
      showStatus("Versie niet gevonden in manifest", true);
      return;
    }
    setBusy(true);
    setProgress(null);
    try {
      let id = installMc;
      if (loader === "vanilla") {
        id = await invoke<string>("install_vanilla", {
          versionId: installMc,
          versionUrl: info.url,
        });
      } else if (loader === "fabric") {
        id = await invoke<string>("install_fabric", {
          gameVersion: installMc,
          gameVersionUrl: info.url,
          loaderVersion: fabricPick,
        });
      } else {
        id = await invoke<string>("install_forge", {
          mcVersion: installMc,
          mcVersionUrl: info.url,
          forgeFull: forgePick,
        });
      }
      showStatus(`Geïnstalleerd: ${id}`);
      await refresh();
      setSelected(id);
      setTab("play");
    } catch (e) {
      showStatus(String(e), true);
    } finally {
      setBusy(false);
    }
  }

  async function loadFabric() {
    if (!installMc) return;
    const list = await invoke<FabricLoader[]>("list_fabric_loaders", {
      gameVersion: installMc,
    });
    setFabricLoaders(list);
    const stable = list.find((l) => l.stable) ?? list[0];
    setFabricPick(stable?.version ?? "");
  }

  async function loadForge() {
    const list = await invoke<ForgeEntry[]>("list_forge_versions", {
      mcVersion: installMc,
    });
    setForgeList(list.slice(0, 30));
    setForgePick(list[0]?.raw ?? "");
  }

  useEffect(() => {
    if (loader === "fabric" && installMc) {
      loadFabric().catch((e) => showStatus(String(e), true));
    }
    if (loader === "forge" && installMc) {
      loadForge().catch((e) => showStatus(String(e), true));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loader, installMc]);

  async function microsoftLogin() {
    setBusy(true);
    setDeviceMsg(null);
    setDeviceCode(null);
    setDeviceUri(null);
    try {
      const code = await invoke<{
        user_code: string;
        device_code: string;
        verification_uri: string;
        interval: number;
        message: string;
      }>("start_microsoft_login");
      setDeviceCode(code.user_code);
      setDeviceUri(code.verification_uri);
      setDeviceMsg(
        "Open de link, vul de code in en wacht tot Cubera je account ophaalt…",
      );
      await openUrl(code.verification_uri);
      try {
        await navigator.clipboard.writeText(code.user_code);
      } catch {
        /* clipboard optional */
      }
      const account = await invoke<Account>("poll_microsoft_login", {
        deviceCode: code.device_code,
        interval: code.interval,
      });
      setDeviceMsg(null);
      setDeviceCode(null);
      setDeviceUri(null);
      showStatus(`Ingelogd als ${account.name}`);
      await refresh();
      setTab("play");
    } catch (e) {
      showStatus(String(e), true);
    } finally {
      setBusy(false);
    }
  }

  async function offlineLogin() {
    try {
      const account = await invoke<Account>("add_offline_account", {
        name: offlineName,
      });
      showStatus(`Offline-account ${account.name}`);
      setOfflineName("");
      await refresh();
    } catch (e) {
      showStatus(String(e), true);
    }
  }

  async function setActiveAccount(uuid: string) {
    if (!settings) return;
    const ok = await invokeOk("set_active_account", { uuid });
    if (ok) {
      await refresh();
      showStatus("Actief account bijgewerkt");
      return;
    }
    await persistSettings({ ...settings, active_account: uuid });
    showStatus("Actief account bijgewerkt");
  }

  function requestRemoveAccount(uuid: string) {
    if (!settings) return;
    const account = settings.accounts.find((a) => a.uuid === uuid);
    setConfirmAction({
      title: "Account verwijderen",
      body: `Account “${account?.name ?? uuid}” verwijderen? Dit kan niet ongedaan worden gemaakt.`,
      onConfirm: async () => {
        const ok = await invokeOk("remove_account", { uuid });
        if (ok) {
          await refresh();
          showStatus("Account verwijderd");
          return;
        }
        const accounts = settings.accounts.filter((a) => a.uuid !== uuid);
        const active =
          settings.active_account === uuid
            ? (accounts[accounts.length - 1]?.uuid ?? null)
            : settings.active_account;
        await persistSettings({ ...settings, accounts, active_account: active });
        showStatus("Account verwijderd");
      },
    });
  }

  async function openInstanceFolder() {
    if (!selected) return;
    const ok = await tryInvoke("open_instance_folder", { instanceId: selected });
    if (ok === null) {
      showStatus("Map openen is nog niet beschikbaar", true);
    }
  }

  function deleteInstance() {
    if (!selected) return;
    const id = selected;
    setConfirmAction({
      title: "Instance verwijderen",
      body: `Instance “${id}” verwijderen?`,
      onConfirm: async () => {
        const ok = await invokeOk("delete_instance", { instanceId: id });
        if (!ok) {
          showStatus("Instance verwijderen is nog niet beschikbaar", true);
          return;
        }
        showStatus(`Instance verwijderd: ${id}`);
        setSelected("");
        await refresh();
      },
    });
  }

  async function searchMods() {
    setBusy(true);
    try {
      const loaderHint = selected.toLowerCase().includes("fabric")
        ? "fabric"
        : selected.toLowerCase().includes("forge")
          ? "forge"
          : undefined;
      const res = await invoke<{ hits: ModHit[] }>("search_mods", {
        query: modQuery,
        loader: loaderHint,
        gameVersion: null,
      });
      setModHits(res.hits);
    } catch (e) {
      showStatus(String(e), true);
    } finally {
      setBusy(false);
    }
  }

  async function installModFromHit(hit: ModHit) {
    if (!selected) return;
    setBusy(true);
    try {
      const loaderHint = selected.toLowerCase().includes("fabric")
        ? "fabric"
        : selected.toLowerCase().includes("forge")
          ? "forge"
          : undefined;
      const versions = await invoke<ModVersion[]>("get_mod_versions", {
        projectId: hit.project_id,
        gameVersion: null,
        loader: loaderHint,
      });
      const file = versions[0]?.files.find((f) => f.primary) ?? versions[0]?.files[0];
      if (!file) throw new Error("Geen downloadbaar bestand");
      await invoke("install_mod", {
        instanceId: selected,
        fileUrl: file.url,
        filename: file.filename,
      });
      showStatus(`Geïnstalleerd: ${file.filename}`);
      setInstanceMods(await invoke("list_mods", { instanceId: selected }));
    } catch (e) {
      showStatus(String(e), true);
    } finally {
      setBusy(false);
    }
  }

  function deleteMod(filename: string) {
    if (!selected) return;
    const instanceId = selected;
    setConfirmAction({
      title: "Mod verwijderen",
      body: `Mod “${filename}” verwijderen van deze instance?`,
      onConfirm: async () => {
        const ok = await invokeOk("delete_mod", { instanceId, filename });
        if (!ok) {
          showStatus("Mod verwijderen is nog niet beschikbaar", true);
          return;
        }
        showStatus(`Mod verwijderd: ${filename}`);
        setInstanceMods(await invoke("list_mods", { instanceId }));
      },
    });
  }

  async function saveMemory(mb: number) {
    if (!settings) return;
    await persistSettings({ ...settings, memory_mb: mb });
  }

  async function saveJavaPath(path: string) {
    if (!settings) return;
    await persistSettings({
      ...settings,
      java_path: path.trim() ? path.trim() : null,
    });
  }

  async function saveExtendedSettings() {
    if (!settings) return;
    const next: Settings = {
      ...settings,
      width: resW,
      height: resH,
      fullscreen,
      jvm_args: jvmArgs,
    };
    await persistSettings(next);
    showStatus("Instellingen opgeslagen");
  }

  const progressPct = progress?.total
    ? Math.min(100, (100 * progress.current) / progress.total)
    : 8;

  const selectedMeta = instanceMeta(selected);
  const launchReady = Boolean(selected && activeAccount && !busy);

  return (
    <div className="shell">
      <aside className="rail">
        <div className="brand">
          <img src="/cubera.svg" alt="" width={32} height={32} />
          <span>Cubera</span>
          <span className="brand-text-mobile">CB</span>
        </div>
        <nav aria-label="Hoofdnavigatie">
          {TABS.map(({ id, label, short }) => (
            <button
              key={id}
              type="button"
              className={tab === id ? "nav active" : "nav"}
              onClick={() => setTab(id)}
              title={label}
            >
              <NavGlyph id={id} />
              <span className="nav-label-full">{label}</span>
              <span className="nav-label-short">{short}</span>
            </button>
          ))}
        </nav>
        <div className="rail-foot">
          <button
            type="button"
            className="rail-account"
            onClick={() => setTab("account")}
            title={activeAccount ? activeAccount.name : "Account"}
          >
            <SkinAvatar account={activeAccount} />
            <div className="rail-meta">
              <p className="name">
                {activeAccount ? activeAccount.name : "Niet ingelogd"}
              </p>
              <p className={javaPath ? "java ok" : "java"}>
                {javaPath ? "Java gereed" : "Java ontbreekt"}
              </p>
            </div>
          </button>
        </div>
      </aside>

      <main className="stage">
        {tab === "play" && (
          <section className="play-hero" key="play">
            <div className="play-left">
              <div className="play-brand">
                <div className="mark-row">
                  <img src="/cubera.svg" alt="" width={52} height={52} />
                  <h1>Cubera</h1>
                </div>
                <p className="tagline">
                  Precisie-instrument voor macOS — vanilla, Fabric, Forge &amp; Modrinth.
                </p>
              </div>

              <div className="play-controls">
                <div className="play-row">
                  <label>
                    Instance
                    <select
                      value={selected}
                      onChange={(e) => setSelected(e.target.value)}
                    >
                      {installed.length === 0 && (
                        <option value="">Nog geen installs — ga naar Installeren</option>
                      )}
                      {installed.map((id) => (
                        <option key={id} value={id}>
                          {id}
                        </option>
                      ))}
                    </select>
                  </label>
                  <button
                    type="button"
                    className="cta launch"
                    disabled={busy || !selected || !activeAccount}
                    onClick={onLaunch}
                  >
                    {busy ? "Bezig…" : "Starten"}
                  </button>
                </div>

                {selected && (
                  <div className="instance-actions">
                    <button type="button" className="cta ghost" onClick={openInstanceFolder}>
                      Map openen
                    </button>
                    <button type="button" className="cta ghost" onClick={deleteInstance}>
                      Verwijderen
                    </button>
                  </div>
                )}

                {!activeAccount && (
                  <p className="hint">
                    Voeg eerst een account toe (Microsoft of offline) onder Account.
                  </p>
                )}
              </div>
            </div>

            <aside className="instance-plate" aria-label="Instance-overzicht">
              <PlateGeometry />
              <div>
                <p className="plate-eyebrow">Geselecteerde instance</p>
                <h2 className="plate-title">{selected || "Geen instance"}</h2>
                <div className="plate-meta">
                  <span className="meta-chip accent">{selectedMeta.loader}</span>
                  <span className="meta-chip">{selectedMeta.kind}</span>
                  <span className="meta-chip">
                    {instanceMods.length} mod{instanceMods.length === 1 ? "" : "s"}
                  </span>
                  <span className="meta-chip">
                    {javaPath ? "Java OK" : "Java?"}
                  </span>
                </div>
              </div>
              <div className="plate-foot">
                <div className="play-glance">
                  <SkinAvatar account={activeAccount} sizeClass="skin" />
                  <div className="info">
                    <strong>{activeAccount?.name ?? "Geen account"}</strong>
                    <span>
                      {activeAccount
                        ? activeAccount.offline
                          ? "Offline"
                          : "Microsoft"
                        : "Meld je aan om te spelen"}
                    </span>
                  </div>
                </div>
                <span className={`plate-status ${launchReady ? "ready" : "warn"}`}>
                  {launchReady ? "Klaar" : "Wacht"}
                </span>
              </div>
            </aside>
          </section>
        )}

        {tab === "install" && (
          <section className="panel" key="install">
            <h2 className="section-head">Installeren</h2>
            <p className="section-sub">Kies een Minecraft-versie en loader.</p>

            <label>
              Minecraft
              <select value={installMc} onChange={(e) => setInstallMc(e.target.value)}>
                {releases.map((v) => (
                  <option key={v.id} value={v.id}>
                    {v.id}
                  </option>
                ))}
              </select>
            </label>

            <div className="chips" role="group" aria-label="Loader">
              {(["vanilla", "fabric", "forge"] as const).map((l) => (
                <button
                  key={l}
                  type="button"
                  className={loader === l ? "chip on" : "chip"}
                  onClick={() => setLoader(l)}
                >
                  {l}
                </button>
              ))}
            </div>

            {loader === "fabric" && (
              <label>
                Fabric-loader
                <select value={fabricPick} onChange={(e) => setFabricPick(e.target.value)}>
                  {fabricLoaders.map((l) => (
                    <option key={l.version} value={l.version}>
                      {l.version}
                      {l.stable ? " (stable)" : ""}
                    </option>
                  ))}
                </select>
              </label>
            )}

            {loader === "forge" && (
              <label>
                Forge
                <select value={forgePick} onChange={(e) => setForgePick(e.target.value)}>
                  {forgeList.map((f) => (
                    <option key={f.raw} value={f.raw}>
                      {f.raw}
                    </option>
                  ))}
                </select>
              </label>
            )}

            <button type="button" className="cta" disabled={busy} onClick={onInstall}>
              {busy ? "Bezig met installeren…" : "Installeren"}
            </button>

            {progress && (
              <div className="progress">
                <div className="progress-track">
                  <div className="bar" style={{ width: `${progressPct}%` }} />
                </div>
                <span>
                  {progress.stage}: {progress.message}
                </span>
              </div>
            )}
          </section>
        )}

        {tab === "mods" && (
          <section className="panel" key="mods">
            <h2 className="section-head">Mods</h2>
            <p className="section-sub">
              Zoek op Modrinth voor instance{" "}
              <strong style={{ color: "var(--text)" }}>{selected || "—"}</strong>
            </p>

            <div className="row">
              <input
                value={modQuery}
                onChange={(e) => setModQuery(e.target.value)}
                placeholder="Zoek op Modrinth…"
                onKeyDown={(e) => e.key === "Enter" && searchMods()}
              />
              <button
                type="button"
                className="cta secondary"
                disabled={busy}
                onClick={searchMods}
              >
                Zoeken
              </button>
            </div>

            <ul className="mod-list">
              {modHits.length === 0 && (
                <li>
                  <div className="mod-body">
                    <p style={{ margin: 0 }}>Nog geen resultaten — typ een zoekterm.</p>
                  </div>
                </li>
              )}
              {modHits.map((hit) => (
                <li key={hit.project_id}>
                  {hit.icon_url ? (
                    <img className="mod-icon" src={hit.icon_url} alt="" />
                  ) : (
                    <div className="mod-icon" />
                  )}
                  <div className="mod-body">
                    <strong>{hit.title}</strong>
                    <p>{hit.description}</p>
                  </div>
                  <button
                    type="button"
                    className="btn-sm"
                    disabled={busy || !selected}
                    onClick={() => installModFromHit(hit)}
                  >
                    Installeren
                  </button>
                </li>
              ))}
            </ul>

            {instanceMods.length > 0 && (
              <div className="list-block">
                <h3>Geïnstalleerd</h3>
                <ul className="plain mod-installed">
                  {instanceMods.map((m) => (
                    <li key={m}>
                      <span>{m}</span>
                      <div className="mod-actions">
                        <button
                          type="button"
                          className="btn-sm danger"
                          onClick={() => deleteMod(m)}
                        >
                          Verwijderen
                        </button>
                      </div>
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </section>
        )}

        {tab === "account" && (
          <section className="panel" key="account">
            <h2 className="section-head">Account</h2>
            <p className="section-sub">
              Microsoft-login of een offline-profiel. Microsoft kan kort “Prism
              Launcher” tonen — dat is alleen hun publieke login-app, niet Cubera-code.
            </p>

            <button type="button" className="cta" disabled={busy} onClick={microsoftLogin}>
              {busy && deviceCode ? "Wachten op Microsoft…" : "Inloggen met Microsoft"}
            </button>
            {deviceCode && (
              <div className="ms-login-box">
                <p className="hint">Ga naar microsoft.com/link en voer deze code in:</p>
                <p className="ms-code">{deviceCode}</p>
                <div className="row">
                  <button
                    type="button"
                    className="cta secondary"
                    onClick={() => deviceUri && openUrl(deviceUri)}
                  >
                    Open loginpagina
                  </button>
                  <button
                    type="button"
                    className="cta secondary"
                    onClick={() => navigator.clipboard.writeText(deviceCode)}
                  >
                    Kopieer code
                  </button>
                </div>
                {deviceMsg && <p className="hint">{deviceMsg}</p>}
              </div>
            )}
            {!deviceCode && deviceMsg && <p className="hint">{deviceMsg}</p>}

            <div className="divider">of offline</div>

            <div className="row">
              <input
                value={offlineName}
                onChange={(e) => setOfflineName(e.target.value)}
                placeholder="Offline gebruikersnaam"
                maxLength={16}
              />
              <button type="button" className="cta secondary" onClick={offlineLogin}>
                Toevoegen
              </button>
            </div>

            <ul className="plain">
              {settings?.accounts.length === 0 && (
                <li>Nog geen accounts.</li>
              )}
              {settings?.accounts.map((a) => {
                const isActive = a.uuid === settings.active_account;
                return (
                  <li key={a.uuid} className="account-row">
                    <div className="left">
                      <SkinAvatar account={a} />
                      <div className="meta">
                        <strong>
                          {a.name}
                          {isActive ? " · actief" : ""}
                        </strong>
                        <span>{a.offline ? "Offline" : "Microsoft"}</span>
                      </div>
                    </div>
                    <div className="actions">
                      {!isActive && (
                        <button
                          type="button"
                          className="btn-sm"
                          onClick={() => setActiveAccount(a.uuid)}
                        >
                          Activeren
                        </button>
                      )}
                      <button
                        type="button"
                        className="btn-sm danger"
                        onClick={() => requestRemoveAccount(a.uuid)}
                      >
                        Verwijderen
                      </button>
                    </div>
                  </li>
                );
              })}
            </ul>
          </section>
        )}

        {tab === "settings" && settings && (
          <section className="panel" key="settings">
            <h2 className="section-head">Instellingen</h2>
            <p className="section-sub">Geheugen, Java en data-map.</p>

            <div className="settings-grid">
              <label>
                Geheugen (MB)
                <input
                  type="number"
                  min={1024}
                  step={512}
                  value={settings.memory_mb}
                  onChange={(e) => saveMemory(Number(e.target.value))}
                />
              </label>
              <label>
                Java-pad
                <input
                  type="text"
                  value={settings.java_path ?? ""}
                  placeholder="Automatisch zoeken"
                  onChange={(e) =>
                    setSettings({ ...settings, java_path: e.target.value || null })
                  }
                  onBlur={(e) => saveJavaPath(e.target.value)}
                />
              </label>

              <label>
                Breedte
                <input
                  type="number"
                  min={640}
                  value={resW}
                  onChange={(e) => setResW(Number(e.target.value))}
                  onBlur={saveExtendedSettings}
                />
              </label>
              <label>
                Hoogte
                <input
                  type="number"
                  min={480}
                  value={resH}
                  onChange={(e) => setResH(Number(e.target.value))}
                  onBlur={saveExtendedSettings}
                />
              </label>
              <label className="check-row full">
                <input
                  type="checkbox"
                  checked={fullscreen}
                  onChange={(e) => {
                    setFullscreen(e.target.checked);
                    if (settings) {
                      persistSettings({
                        ...settings,
                        width: resW,
                        height: resH,
                        fullscreen: e.target.checked,
                        jvm_args: jvmArgs,
                      }).catch((err) => showStatus(String(err), true));
                    }
                  }}
                />
                Volledig scherm
              </label>
              <label className="full">
                JVM-argumenten
                <input
                  type="text"
                  value={jvmArgs}
                  placeholder="-XX:+UseG1GC …"
                  onChange={(e) => setJvmArgs(e.target.value)}
                  onBlur={saveExtendedSettings}
                />
              </label>

              <div className="full">
                <p className="hint">
                  Java: {javaPath ?? "niet gevonden — brew install --cask temurin"}
                </p>
                <DataDir />
              </div>
            </div>
          </section>
        )}

        {status && (
          <footer className={statusError ? "status error" : "status"}>{status}</footer>
        )}
      </main>

      {confirmAction && (
        <div className="modal-backdrop" role="presentation">
          <div className="modal" role="dialog" aria-modal="true" aria-labelledby="confirm-title">
            <h3 id="confirm-title">{confirmAction.title}</h3>
            <p>{confirmAction.body}</p>
            <div className="row">
              <button
                type="button"
                className="cta secondary"
                onClick={() => setConfirmAction(null)}
              >
                Annuleren
              </button>
              <button
                type="button"
                className="cta danger-cta"
                onClick={async () => {
                  const action = confirmAction;
                  setConfirmAction(null);
                  try {
                    await action.onConfirm();
                  } catch (e) {
                    showStatus(String(e), true);
                  }
                }}
              >
                Verwijderen
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function DataDir() {
  const [dir, setDir] = useState("");
  useEffect(() => {
    invoke<string>("get_data_dir").then(setDir);
  }, []);
  return <p className="hint" style={{ fontSize: "0.75rem", wordBreak: "break-all" }}>Data: {dir}</p>;
}
