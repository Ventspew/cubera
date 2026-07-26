import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import "./App.css";

type Tab = "instances" | "install" | "mods" | "news" | "account" | "settings";

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
  ingame_branding?: boolean;
};

type AppInfo = {
  name: string;
  version: string;
  tagline: string;
};

type LaunchLog = {
  stdout: string;
  stderr: string;
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

type InstanceInfo = {
  id: string;
  version_id: string;
  name: string;
  notes: string;
  game_version?: string;
  loader?: string;
  memory_mb?: number;
  jvm_args?: string;
  java_path?: string;
  last_played?: string;
  play_count: number;
  created_at?: string;
  mod_count: number;
  running: boolean;
};

type NewsItem = {
  title: string;
  tag: string;
  date: string;
  text: string;
  image_url?: string;
  read_more_url?: string;
};

type ContentKind = "mods" | "resourcepacks" | "shaders";
type VersionFilter = "release" | "snapshot" | "all";
type LoaderKind = "vanilla" | "fabric" | "quilt" | "forge";

const TABS: { id: Tab; label: string; short: string }[] = [
  { id: "instances", label: "Instances", short: "Inst" },
  { id: "install", label: "Install", short: "Add" },
  { id: "mods", label: "Mods", short: "Mods" },
  { id: "news", label: "News", short: "News" },
  { id: "account", label: "Account", short: "Acc" },
  { id: "settings", label: "Settings", short: "Set" },
];

function CuberaLogo({ size = 32, className = "" }: { size?: number; className?: string }) {
  return (
    <svg
      className={`cubera-logo ${className}`.trim()}
      width={size}
      height={size}
      viewBox="0 0 64 64"
      fill="none"
      aria-hidden
    >
      <defs>
        <linearGradient id="logo-ore" x1="6" y1="4" x2="58" y2="60" gradientUnits="userSpaceOnUse">
          <stop stopColor="#F5D4A8" />
          <stop offset="0.35" stopColor="#E8A86A" />
          <stop offset="0.65" stopColor="#D4894A" />
          <stop offset="1" stopColor="#6B3818" />
        </linearGradient>
        <linearGradient id="logo-facet" x1="18" y1="10" x2="50" y2="54" gradientUnits="userSpaceOnUse">
          <stop stopColor="#2E2620" />
          <stop offset="0.55" stopColor="#1A1612" />
          <stop offset="1" stopColor="#0A0908" />
        </linearGradient>
        <radialGradient id="logo-glow" cx="32" cy="32" r="28" gradientUnits="userSpaceOnUse">
          <stop stopColor="#D4894A" stopOpacity="0.28" />
          <stop offset="1" stopColor="#D4894A" stopOpacity="0" />
        </radialGradient>
      </defs>
      <circle cx="32" cy="32" r="30" fill="url(#logo-glow)" className="logo-glow-ring" />
      <path
        d="M8 18 L32 4 L56 18 L56 42 L32 60 L8 42 Z"
        fill="url(#logo-facet)"
        stroke="rgba(232,168,106,0.45)"
        strokeWidth="1.35"
      />
      <path d="M32 4 L56 18 L32 30 L8 18 Z" fill="rgba(245,212,168,0.12)" />
      <path d="M8 18 L32 30 L32 60 L8 42 Z" fill="rgba(0,0,0,0.32)" />
      <path d="M56 18 L32 30 L32 60 L56 42 Z" fill="rgba(212,137,74,0.14)" />
      <path
        d="M40.5 22.5c-1.4-2.8-4.6-4.7-8.3-4.7-5.4 0-9.5 3.9-9.5 9.7v9c0 5.8 4.1 9.7 9.5 9.7 3.7 0 6.9-1.9 8.3-4.7"
        stroke="url(#logo-ore)"
        strokeWidth="3.6"
        strokeLinecap="round"
        className="logo-monogram"
      />
      <path
        d="M21 27 L29.5 32.5 L27.5 39.5 L35.5 43.5 L39 48"
        stroke="#F0C08A"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
        className="logo-vein"
      />
      <circle cx="39" cy="48" r="2" fill="#F0C08A" className="logo-vein-dot" />
    </svg>
  );
}

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
    case "instances":
      return (
        <svg {...common}>
          <path d="M2 2h4v4H2zM8 2h4v4H8zM2 8h4v4H2zM8 8h4v4H8z" stroke="currentColor" strokeWidth="1.3" />
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
    case "news":
      return (
        <svg {...common}>
          <path d="M2 2.5h10v9H2z" stroke="currentColor" strokeWidth="1.3" />
          <path d="M4 5h6M4 7.5h6M4 10h3.5" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
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

function instanceMeta(loaderOrId: string) {
  const lower = (loaderOrId || "").toLowerCase();
  if (lower.includes("fabric")) return { loader: "Fabric", kind: "Modded" };
  if (lower.includes("quilt")) return { loader: "Quilt", kind: "Modded" };
  if (lower.includes("neoforge")) return { loader: "NeoForge", kind: "Modded" };
  if (lower.includes("forge")) return { loader: "Forge", kind: "Modded" };
  if (!loaderOrId) return { loader: "—", kind: "No instance" };
  if (lower === "vanilla" || lower.includes("release") || /^\d+\.\d+/.test(lower)) {
    return { loader: "Vanilla", kind: "Release" };
  }
  return { loader: "Vanilla", kind: "Release" };
}

function stripHtml(html: string): string {
  return html
    .replace(/<br\s*\/?>/gi, "\n")
    .replace(/<\/p>/gi, "\n")
    .replace(/<[^>]+>/g, "")
    .replace(/&nbsp;/g, " ")
    .replace(/&amp;/g, "&")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

function formatPlayed(iso?: string): string {
  if (!iso) return "Never";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  });
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

function loaderHintFromInstance(inst: InstanceInfo | null | undefined): string | undefined {
  if (!inst) return undefined;
  const raw = (inst.loader || inst.version_id || inst.id).toLowerCase();
  if (raw.includes("fabric")) return "fabric";
  if (raw.includes("quilt")) return "quilt";
  if (raw.includes("neoforge")) return "neoforge";
  if (raw.includes("forge")) return "forge";
  return undefined;
}

export default function App() {
  const [tab, setTab] = useState<Tab>("instances");
  const [manifest, setManifest] = useState<VersionManifest | null>(null);
  const [instances, setInstances] = useState<InstanceInfo[]>([]);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [selected, setSelected] = useState<string>("");
  const [status, setStatus] = useState("");
  const [statusError, setStatusError] = useState(false);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<ProgressEvent | null>(null);
  const [javaPath, setJavaPath] = useState<string | null>(null);

  const [installMc, setInstallMc] = useState("");
  const [versionFilter, setVersionFilter] = useState<VersionFilter>("release");
  const [loader, setLoader] = useState<LoaderKind>("vanilla");
  const [fabricLoaders, setFabricLoaders] = useState<FabricLoader[]>([]);
  const [fabricPick, setFabricPick] = useState("");
  const [quiltLoaders, setQuiltLoaders] = useState<FabricLoader[]>([]);
  const [quiltPick, setQuiltPick] = useState("");
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

  const [contentKind, setContentKind] = useState<ContentKind>("mods");
  const [modQuery, setModQuery] = useState("");
  const [modHits, setModHits] = useState<ModHit[]>([]);
  const [instanceMods, setInstanceMods] = useState<string[]>([]);

  const [news, setNews] = useState<NewsItem[]>([]);
  const [newsLoading, setNewsLoading] = useState(false);

  const [editName, setEditName] = useState("");
  const [editNotes, setEditNotes] = useState("");
  const [editMemory, setEditMemory] = useState("");
  const [editJvm, setEditJvm] = useState("");
  const [selectedRunning, setSelectedRunning] = useState(false);

  const [resW, setResW] = useState(1280);
  const [resH, setResH] = useState(720);
  const [fullscreen, setFullscreen] = useState(false);
  const [jvmArgs, setJvmArgs] = useState("");
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [launchLog, setLaunchLog] = useState<LaunchLog | null>(null);
  const [logLoading, setLogLoading] = useState(false);

  const activeAccount = useMemo(() => {
    if (!settings) return null;
    return (
      settings.accounts.find((a) => a.uuid === settings.active_account) ??
      settings.accounts[settings.accounts.length - 1] ??
      null
    );
  }, [settings]);

  const selectedInstance = useMemo(
    () => instances.find((i) => i.id === selected) ?? null,
    [instances, selected],
  );

  const showStatus = useCallback((msg: string, isError = false) => {
    setStatus(msg);
    setStatusError(isError);
  }, []);

  const refreshInstances = useCallback(async () => {
    const list = await invoke<InstanceInfo[]>("list_instances");
    setInstances(list);
    return list;
  }, []);

  const refresh = useCallback(async () => {
    const [m, list, s, java] = await Promise.all([
      invoke<VersionManifest>("get_version_manifest"),
      invoke<InstanceInfo[]>("list_instances"),
      invoke<Settings>("get_settings"),
      invoke<{ path: string | null; found: boolean }>("get_java_info"),
    ]);
    setManifest(m);
    setInstances(list);
    setSettings(s);
    setJavaPath(java.path);
    if (typeof s.width === "number") setResW(s.width);
    if (typeof s.height === "number") setResH(s.height);
    if (typeof s.fullscreen === "boolean") setFullscreen(s.fullscreen);
    if (typeof s.jvm_args === "string") setJvmArgs(s.jvm_args);

    const ids = list.map((i) => i.id);
    const initial =
      s.last_version && ids.includes(s.last_version)
        ? s.last_version
        : list[0]?.id ?? "";
    setSelected((prev) => (prev && ids.includes(prev) ? prev : initial));
    setInstallMc((prev) => prev || m.latest.release);
  }, []);

  useEffect(() => {
    refresh().catch((e) => showStatus(String(e), true));
    invoke<AppInfo>("get_app_info").then(setAppInfo).catch(() => {});
    const unlisten = listen<ProgressEvent>("install-progress", (e) => {
      setProgress(e.payload);
      showStatus(e.payload.message);
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, [refresh, showStatus]);

  useEffect(() => {
    if (!selectedInstance) {
      setEditName("");
      setEditNotes("");
      setEditMemory("");
      setEditJvm("");
      setSelectedRunning(false);
      return;
    }
    setEditName(selectedInstance.name);
    setEditNotes(selectedInstance.notes ?? "");
    setEditMemory(
      selectedInstance.memory_mb != null ? String(selectedInstance.memory_mb) : "",
    );
    setEditJvm(selectedInstance.jvm_args ?? "");
    setSelectedRunning(selectedInstance.running);
  }, [selectedInstance?.id, selectedInstance?.name, selectedInstance?.notes, selectedInstance?.memory_mb, selectedInstance?.jvm_args, selectedInstance?.running]);

  useEffect(() => {
    if (!selected) {
      setInstanceMods([]);
      return;
    }
    invoke<string[]>("list_mods", { instanceId: selected })
      .then(setInstanceMods)
      .catch(() => setInstanceMods([]));
  }, [selected]);

  useEffect(() => {
    if (!selected) return;
    let cancelled = false;
    const tick = async () => {
      try {
        const running = await invoke<boolean>("is_instance_running", {
          instanceId: selected,
        });
        if (!cancelled) {
          setSelectedRunning(running);
          setInstances((prev) =>
            prev.map((i) => (i.id === selected ? { ...i, running } : i)),
          );
        }
      } catch {
        /* ignore poll errors */
      }
    };
    tick();
    const id = window.setInterval(tick, 3000);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, [selected]);

  useEffect(() => {
    if (tab !== "news") return;
    let cancelled = false;
    setNewsLoading(true);
    invoke<NewsItem[]>("fetch_news")
      .then((items) => {
        if (!cancelled) setNews(items);
      })
      .catch((e) => {
        if (!cancelled) showStatus(String(e), true);
      })
      .finally(() => {
        if (!cancelled) setNewsLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [tab, showStatus]);

  const filteredVersions = useMemo(() => {
    if (!manifest) return [];
    const all = manifest.versions;
    const filtered =
      versionFilter === "all"
        ? all
        : all.filter((v) => v.type === versionFilter);
    return filtered.slice(0, 80);
  }, [manifest, versionFilter]);

  useEffect(() => {
    if (filteredVersions.length === 0) return;
    if (!filteredVersions.some((v) => v.id === installMc)) {
      setInstallMc(filteredVersions[0].id);
    }
  }, [filteredVersions, installMc]);

  async function persistSettings(next: Settings) {
    await invoke("update_settings", { settings: next });
    setSettings(next);
  }

  async function onLaunch() {
    if (!selected) return;
    setBusy(true);
    showStatus("Starting Minecraft…");
    try {
      const msg = await invoke<string>("launch_instance", { versionId: selected });
      if (settings) {
        await persistSettings({ ...settings, last_version: selected });
      }
      setSelectedRunning(true);
      showStatus(msg);
      await refreshInstances();
    } catch (e) {
      showStatus(String(e), true);
    } finally {
      setBusy(false);
    }
  }

  async function onKill() {
    if (!selected) return;
    setBusy(true);
    try {
      await invoke("kill_instance", { instanceId: selected });
      setSelectedRunning(false);
      showStatus("Instance stopped");
      await refreshInstances();
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
      showStatus("Version not found in manifest", true);
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
      } else if (loader === "quilt") {
        id = await invoke<string>("install_quilt", {
          gameVersion: installMc,
          gameVersionUrl: info.url,
          loaderVersion: quiltPick,
        });
      } else {
        id = await invoke<string>("install_forge", {
          mcVersion: installMc,
          mcVersionUrl: info.url,
          forgeFull: forgePick,
        });
      }
      showStatus(`Installed: ${id}`);
      const list = await refreshInstances();
      await refresh();
      const pick = list.find((i) => i.id === id)?.id ?? id;
      setSelected(pick);
      setTab("instances");
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

  async function loadQuilt() {
    if (!installMc) return;
    const list = await invoke<FabricLoader[]>("list_quilt_loaders", {
      gameVersion: installMc,
    });
    setQuiltLoaders(list);
    const stable = list.find((l) => l.stable) ?? list[0];
    setQuiltPick(stable?.version ?? "");
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
    if (loader === "quilt" && installMc) {
      loadQuilt().catch((e) => showStatus(String(e), true));
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
        "Open the link, enter the code, and wait for Cubera to fetch your account…",
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
      showStatus(`Signed in as ${account.name}`);
      await refresh();
      setTab("instances");
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
      showStatus(`Offline account ${account.name}`);
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
      showStatus("Active account updated");
      return;
    }
    await persistSettings({ ...settings, active_account: uuid });
    showStatus("Active account updated");
  }

  function requestRemoveAccount(uuid: string) {
    if (!settings) return;
    const account = settings.accounts.find((a) => a.uuid === uuid);
    setConfirmAction({
      title: "Remove account",
      body: `Remove account “${account?.name ?? uuid}”? This cannot be undone.`,
      onConfirm: async () => {
        const ok = await invokeOk("remove_account", { uuid });
        if (ok) {
          await refresh();
          showStatus("Account removed");
          return;
        }
        const accounts = settings.accounts.filter((a) => a.uuid !== uuid);
        const active =
          settings.active_account === uuid
            ? (accounts[accounts.length - 1]?.uuid ?? null)
            : settings.active_account;
        await persistSettings({ ...settings, accounts, active_account: active });
        showStatus("Account removed");
      },
    });
  }

  async function openInstanceFolder() {
    if (!selected) return;
    const ok = await tryInvoke("open_instance_folder", { instanceId: selected });
    if (ok === null) {
      showStatus("Could not open instance folder", true);
    }
  }

  async function openSubfolder(folder: string) {
    if (!selected) return;
    const ok = await invokeOk("open_instance_subfolder", {
      instanceId: selected,
      folder,
    });
    if (!ok) showStatus(`Could not open ${folder}`, true);
  }

  async function saveInstanceFields() {
    if (!selectedInstance) return;
    setBusy(true);
    try {
      const memoryRaw = editMemory.trim();
      const memory_mb =
        memoryRaw === "" ? undefined : Number.parseInt(memoryRaw, 10);
      if (memoryRaw !== "" && (Number.isNaN(memory_mb!) || memory_mb! < 512)) {
        showStatus("Memory must be empty (use global) or at least 512 MB", true);
        return;
      }
      const meta = {
        id: selectedInstance.id,
        version_id: selectedInstance.version_id,
        name: editName.trim() || selectedInstance.id,
        notes: editNotes,
        game_version: selectedInstance.game_version,
        loader: selectedInstance.loader,
        memory_mb,
        jvm_args: editJvm.trim() || undefined,
        java_path: selectedInstance.java_path,
        last_played: selectedInstance.last_played,
        play_count: selectedInstance.play_count,
        created_at: selectedInstance.created_at,
      };
      await invoke("update_instance", { meta });
      showStatus("Instance saved");
      await refreshInstances();
    } catch (e) {
      showStatus(String(e), true);
    } finally {
      setBusy(false);
    }
  }

  function duplicateSelected() {
    if (!selectedInstance) return;
    const base = selectedInstance.name || selectedInstance.id;
    const newName = `${base} copy`;
    setConfirmAction({
      title: "Duplicate instance",
      body: `Create a copy named “${newName}”?`,
      onConfirm: async () => {
        const id = await invoke<string>("duplicate_instance", {
          instanceId: selectedInstance.id,
          newName,
        });
        showStatus(`Duplicated as ${id}`);
        await refreshInstances();
        setSelected(id);
      },
    });
  }

  function deleteInstance() {
    if (!selected) return;
    const id = selected;
    const name = selectedInstance?.name ?? id;
    setConfirmAction({
      title: "Delete instance",
      body: `Delete instance “${name}”? This cannot be undone.`,
      onConfirm: async () => {
        const ok = await invokeOk("delete_instance", { instanceId: id });
        if (!ok) {
          showStatus("Could not delete instance", true);
          return;
        }
        showStatus(`Instance deleted: ${name}`);
        setSelected("");
        await refreshInstances();
      },
    });
  }

  async function searchContent() {
    if (!selectedInstance && contentKind === "mods") {
      /* still allow search without instance, but install needs one */
    }
    setBusy(true);
    try {
      const gameVersion = selectedInstance?.game_version ?? null;
      const loaderHint = loaderHintFromInstance(selectedInstance);
      let res: { hits: ModHit[] };
      if (contentKind === "mods") {
        res = await invoke<{ hits: ModHit[] }>("search_mods", {
          query: modQuery,
          loader: loaderHint,
          gameVersion,
        });
      } else if (contentKind === "resourcepacks") {
        res = await invoke<{ hits: ModHit[] }>("search_resourcepacks", {
          query: modQuery,
          gameVersion,
        });
      } else {
        res = await invoke<{ hits: ModHit[] }>("search_shaders", {
          query: modQuery,
          gameVersion,
        });
      }
      setModHits(res.hits);
    } catch (e) {
      showStatus(String(e), true);
    } finally {
      setBusy(false);
    }
  }

  async function installContentFromHit(hit: ModHit) {
    if (!selected) {
      showStatus("Select an instance first", true);
      return;
    }
    setBusy(true);
    try {
      const loaderHint = loaderHintFromInstance(selectedInstance);
      const gameVersion = selectedInstance?.game_version ?? null;
      const versions = await invoke<ModVersion[]>("get_mod_versions", {
        projectId: hit.project_id,
        gameVersion,
        loader: contentKind === "mods" ? loaderHint : null,
      });
      const file = versions[0]?.files.find((f) => f.primary) ?? versions[0]?.files[0];
      if (!file) throw new Error("No downloadable file found");

      if (contentKind === "mods") {
        await invoke("install_mod", {
          instanceId: selected,
          fileUrl: file.url,
          filename: file.filename,
        });
        setInstanceMods(await invoke("list_mods", { instanceId: selected }));
      } else {
        const folder =
          contentKind === "resourcepacks" ? "resourcepacks" : "shaderpacks";
        await invoke("install_content", {
          instanceId: selected,
          folder,
          fileUrl: file.url,
          filename: file.filename,
        });
      }
      showStatus(`Installed: ${file.filename}`);
      await refreshInstances();
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
      title: "Remove mod",
      body: `Remove mod “${filename}” from this instance?`,
      onConfirm: async () => {
        const ok = await invokeOk("delete_mod", { instanceId, filename });
        if (!ok) {
          showStatus("Could not remove mod", true);
          return;
        }
        showStatus(`Mod removed: ${filename}`);
        setInstanceMods(await invoke("list_mods", { instanceId }));
        await refreshInstances();
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
    showStatus("Settings saved");
  }

  async function loadLaunchLog() {
    if (!selected) return;
    setLogLoading(true);
    try {
      const log = await invoke<LaunchLog>("get_launch_log", { instanceId: selected });
      setLaunchLog(log);
    } catch (e) {
      showStatus(String(e), true);
    } finally {
      setLogLoading(false);
    }
  }

  async function saveIngameBranding(enabled: boolean) {
    if (!settings) return;
    await persistSettings({ ...settings, ingame_branding: enabled });
    showStatus(enabled ? "In-game branding enabled" : "In-game branding disabled");
  }

  async function openDataFolder() {
    const ok = await tryInvoke("open_data_folder");
    if (ok === null) showStatus("Failed to open data folder", true);
  }

  const progressPct = progress?.total
    ? Math.min(100, (100 * progress.current) / progress.total)
    : 8;

  const plateMeta = instanceMeta(
    selectedInstance?.loader || selectedInstance?.version_id || selected,
  );
  const launchReady = Boolean(selected && activeAccount && !busy && !selectedRunning);
  const isRunning = selectedRunning || Boolean(selectedInstance?.running);

  return (
    <div className="shell">
      <aside className="rail">
        <div className="brand">
          <CuberaLogo size={32} className="brand-mark" />
          <span>Cubera</span>
          <span className="brand-text-mobile">CB</span>
        </div>
        <nav aria-label="Main navigation">
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
                {activeAccount ? activeAccount.name : "Not signed in"}
              </p>
              <p className={javaPath ? "java ok" : "java"}>
                {javaPath ? "Java ready" : "Java missing"}
              </p>
            </div>
          </button>
        </div>
      </aside>

      <main className="stage">
        {tab === "instances" && (
          <section className="instances-view" key="instances">
            {instances.length === 0 ? (
              <div className="instances-empty">
                <CuberaLogo size={64} className="hero-mark" />
                <h2 className="section-head">No instances yet</h2>
                <p className="section-sub">
                  Install Minecraft with vanilla, Fabric, Quilt, or Forge to get started.
                </p>
                <button
                  type="button"
                  className="cta launch"
                  onClick={() => setTab("install")}
                >
                  Install Minecraft
                </button>
              </div>
            ) : (
              <div className="instances-layout">
                <div className="instances-main">
                  <header className="instances-head">
                    <div>
                      <h2 className="section-head">Instances</h2>
                      <p className="section-sub">
                        {instances.length} instance{instances.length === 1 ? "" : "s"} ·{" "}
                        {appInfo?.tagline ?? "Minecraft launcher"}
                      </p>
                    </div>
                    <button
                      type="button"
                      className="cta secondary"
                      onClick={() => setTab("install")}
                    >
                      New install
                    </button>
                  </header>

                  <div className="instances-grid" role="list">
                    {instances.map((inst) => {
                      const meta = instanceMeta(inst.loader || inst.version_id || inst.id);
                      const active = inst.id === selected;
                      return (
                        <button
                          key={inst.id}
                          type="button"
                          role="listitem"
                          className={active ? "instance-card on" : "instance-card"}
                          onClick={() => setSelected(inst.id)}
                        >
                          <div className="instance-card-top">
                            <strong className="instance-card-name">{inst.name || inst.id}</strong>
                            {(inst.running || (active && isRunning)) && (
                              <span className="running-badge">Running</span>
                            )}
                          </div>
                          <div className="instance-card-meta">
                            <span className="meta-chip accent">{meta.loader}</span>
                            <span className="meta-chip">
                              {inst.game_version || inst.version_id || "—"}
                            </span>
                            <span className="meta-chip">
                              {inst.mod_count} mod{inst.mod_count === 1 ? "" : "s"}
                            </span>
                          </div>
                          <p className="instance-card-played">
                            Last played: {formatPlayed(inst.last_played)}
                          </p>
                        </button>
                      );
                    })}
                  </div>
                </div>

                {selectedInstance && (
                  <aside className="instance-detail" aria-label="Instance details">
                    <div className="instance-plate detail-plate">
                      <PlateGeometry />
                      <div>
                        <p className="plate-eyebrow">Selected instance</p>
                        <h2 className="plate-title">
                          {selectedInstance.name || selectedInstance.id}
                        </h2>
                        <div className="plate-meta">
                          <span className="meta-chip accent">{plateMeta.loader}</span>
                          <span className="meta-chip">
                            {selectedInstance.game_version || selectedInstance.version_id}
                          </span>
                          <span className="meta-chip">
                            {selectedInstance.mod_count} mod
                            {selectedInstance.mod_count === 1 ? "" : "s"}
                          </span>
                          {isRunning && <span className="meta-chip running">Running</span>}
                        </div>
                      </div>
                      <div className="plate-foot">
                        <div className="play-glance">
                          <SkinAvatar account={activeAccount} sizeClass="skin" />
                          <div className="info">
                            <strong>{activeAccount?.name ?? "No account"}</strong>
                            <span>
                              {activeAccount
                                ? activeAccount.offline
                                  ? "Offline"
                                  : "Microsoft"
                                : "Sign in to play"}
                            </span>
                          </div>
                        </div>
                        <span className={`plate-status ${launchReady || isRunning ? "ready" : "warn"}`}>
                          {isRunning ? "Live" : launchReady ? "Ready" : "Waiting"}
                        </span>
                      </div>
                    </div>

                    <div className="instance-actions">
                      {isRunning ? (
                        <button
                          type="button"
                          className="cta danger-cta"
                          disabled={busy}
                          onClick={onKill}
                        >
                          Kill
                        </button>
                      ) : (
                        <button
                          type="button"
                          className="cta launch"
                          disabled={!launchReady}
                          onClick={onLaunch}
                        >
                          {busy ? "Working…" : "Launch"}
                        </button>
                      )}
                      <button type="button" className="cta ghost" onClick={openInstanceFolder}>
                        Open folder
                      </button>
                    </div>

                    <div className="content-tabs subfolder-tabs" role="group" aria-label="Folders">
                      <button type="button" className="chip" onClick={() => openSubfolder("mods")}>
                        Mods
                      </button>
                      <button type="button" className="chip" onClick={() => openSubfolder("saves")}>
                        Saves
                      </button>
                      <button
                        type="button"
                        className="chip"
                        onClick={() => openSubfolder("screenshots")}
                      >
                        Screenshots
                      </button>
                    </div>

                    <div className="instance-edit">
                      <label>
                        Name
                        <input
                          value={editName}
                          onChange={(e) => setEditName(e.target.value)}
                        />
                      </label>
                      <label>
                        Notes
                        <textarea
                          className="instance-notes"
                          rows={3}
                          value={editNotes}
                          onChange={(e) => setEditNotes(e.target.value)}
                          placeholder="Optional notes…"
                        />
                      </label>
                      <label>
                        Memory (MB)
                        <input
                          type="number"
                          min={512}
                          step={512}
                          value={editMemory}
                          onChange={(e) => setEditMemory(e.target.value)}
                          placeholder={`Global (${settings?.memory_mb ?? 4096})`}
                        />
                      </label>
                      <label>
                        JVM arguments
                        <input
                          value={editJvm}
                          onChange={(e) => setEditJvm(e.target.value)}
                          placeholder="Optional override…"
                        />
                      </label>
                      <button
                        type="button"
                        className="cta secondary"
                        disabled={busy}
                        onClick={saveInstanceFields}
                      >
                        Save
                      </button>
                    </div>

                    <div className="instance-actions secondary-actions">
                      <button type="button" className="cta ghost" onClick={duplicateSelected}>
                        Duplicate
                      </button>
                      <button type="button" className="cta ghost" onClick={deleteInstance}>
                        Delete
                      </button>
                    </div>

                    {!activeAccount && (
                      <p className="hint">
                        Add an account first (Microsoft or offline) under Account.
                      </p>
                    )}
                  </aside>
                )}
              </div>
            )}
          </section>
        )}

        {tab === "install" && (
          <section className="panel" key="install">
            <h2 className="section-head">Install</h2>
            <p className="section-sub">Choose a Minecraft version and loader.</p>

            <div className="chips content-tabs" role="group" aria-label="Version type">
              {(["release", "snapshot", "all"] as const).map((f) => (
                <button
                  key={f}
                  type="button"
                  className={versionFilter === f ? "chip on" : "chip"}
                  onClick={() => setVersionFilter(f)}
                >
                  {f}
                </button>
              ))}
            </div>

            <label>
              Minecraft
              <select value={installMc} onChange={(e) => setInstallMc(e.target.value)}>
                {filteredVersions.map((v) => (
                  <option key={v.id} value={v.id}>
                    {v.id}
                    {v.type !== "release" ? ` (${v.type})` : ""}
                  </option>
                ))}
              </select>
            </label>

            <div className="chips" role="group" aria-label="Loader">
              {(["vanilla", "fabric", "quilt", "forge"] as const).map((l) => (
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
                Fabric loader
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

            {loader === "quilt" && (
              <label>
                Quilt loader
                <select value={quiltPick} onChange={(e) => setQuiltPick(e.target.value)}>
                  {quiltLoaders.map((l) => (
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
              {busy ? "Installing…" : "Install"}
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
            <h2 className="section-head">Content</h2>
            <p className="section-sub">
              Search Modrinth for instance{" "}
              <strong style={{ color: "var(--text)" }}>
                {selectedInstance?.name || selected || "—"}
              </strong>
              {selectedInstance?.game_version
                ? ` · ${selectedInstance.game_version}`
                : ""}
            </p>

            <div className="chips content-tabs" role="group" aria-label="Content type">
              {(["mods", "resourcepacks", "shaders"] as const).map((k) => (
                <button
                  key={k}
                  type="button"
                  className={contentKind === k ? "chip on" : "chip"}
                  onClick={() => {
                    setContentKind(k);
                    setModHits([]);
                  }}
                >
                  {k === "resourcepacks" ? "resource packs" : k}
                </button>
              ))}
            </div>

            <div className="row">
              <input
                value={modQuery}
                onChange={(e) => setModQuery(e.target.value)}
                placeholder="Search Modrinth…"
                onKeyDown={(e) => e.key === "Enter" && searchContent()}
              />
              <button
                type="button"
                className="cta secondary"
                disabled={busy}
                onClick={searchContent}
              >
                Search
              </button>
            </div>

            <ul className="mod-list">
              {modHits.length === 0 && (
                <li>
                  <div className="mod-body">
                    <p style={{ margin: 0 }}>No results yet — enter a search term.</p>
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
                    onClick={() => installContentFromHit(hit)}
                  >
                    Install
                  </button>
                </li>
              ))}
            </ul>

            {contentKind === "mods" && instanceMods.length > 0 && (
              <div className="list-block">
                <h3>Installed mods</h3>
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
                          Remove
                        </button>
                      </div>
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </section>
        )}

        {tab === "news" && (
          <section className="panel news-panel" key="news">
            <h2 className="section-head">News</h2>
            <p className="section-sub">Latest from Minecraft.</p>

            {newsLoading && <p className="hint">Loading news…</p>}
            {!newsLoading && news.length === 0 && (
              <p className="hint">No news available right now.</p>
            )}

            <div className="news-list">
              {news.map((item, idx) => (
                <article key={`${item.title}-${idx}`} className="news-card">
                  {item.image_url && (
                    <img className="news-image" src={item.image_url} alt="" />
                  )}
                  <div className="news-body">
                    <div className="news-meta">
                      <span className="meta-chip accent">{item.tag}</span>
                      {item.date && <span className="news-date">{item.date}</span>}
                    </div>
                    <h3>{item.title}</h3>
                    <p>
                      {(() => {
                        const plain = stripHtml(item.text);
                        return plain.length > 280
                          ? `${plain.slice(0, 280)}…`
                          : plain;
                      })()}
                    </p>
                    {item.read_more_url && (
                      <button
                        type="button"
                        className="cta ghost"
                        onClick={() => openUrl(item.read_more_url!)}
                      >
                        Read more
                      </button>
                    )}
                  </div>
                </article>
              ))}
            </div>
          </section>
        )}

        {tab === "account" && (
          <section className="panel" key="account">
            <h2 className="section-head">Account</h2>
            <p className="section-sub">
              Microsoft sign-in or an offline profile. Microsoft may briefly show “Prism
              Launcher” — that is only their public login app, not Cubera code.
            </p>

            <button type="button" className="cta" disabled={busy} onClick={microsoftLogin}>
              {busy && deviceCode ? "Waiting for Microsoft…" : "Sign in with Microsoft"}
            </button>
            {deviceCode && (
              <div className="ms-login-box">
                <p className="hint">Go to microsoft.com/link and enter this code:</p>
                <p className="ms-code">{deviceCode}</p>
                <div className="row">
                  <button
                    type="button"
                    className="cta secondary"
                    onClick={() => deviceUri && openUrl(deviceUri)}
                  >
                    Open sign-in page
                  </button>
                  <button
                    type="button"
                    className="cta secondary"
                    onClick={() => navigator.clipboard.writeText(deviceCode)}
                  >
                    Copy code
                  </button>
                </div>
                {deviceMsg && <p className="hint">{deviceMsg}</p>}
              </div>
            )}
            {!deviceCode && deviceMsg && <p className="hint">{deviceMsg}</p>}

            <div className="divider">or offline</div>

            <div className="row">
              <input
                value={offlineName}
                onChange={(e) => setOfflineName(e.target.value)}
                placeholder="Offline username"
                maxLength={16}
              />
              <button type="button" className="cta secondary" onClick={offlineLogin}>
                Add
              </button>
            </div>

            <ul className="plain">
              {settings?.accounts.length === 0 && (
                <li>No accounts yet.</li>
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
                          {isActive ? " · active" : ""}
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
                          Activate
                        </button>
                      )}
                      <button
                        type="button"
                        className="btn-sm danger"
                        onClick={() => requestRemoveAccount(a.uuid)}
                      >
                        Remove
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
            <h2 className="section-head">Settings</h2>
            <p className="section-sub">Memory, Java, resolution, and data folder.</p>

            <div className="settings-grid">
              <label>
                Memory (MB)
                <input
                  type="number"
                  min={1024}
                  step={512}
                  value={settings.memory_mb}
                  onChange={(e) => saveMemory(Number(e.target.value))}
                />
              </label>
              <label>
                Java path
                <input
                  type="text"
                  value={settings.java_path ?? ""}
                  placeholder="Auto-detect"
                  onChange={(e) =>
                    setSettings({ ...settings, java_path: e.target.value || null })
                  }
                  onBlur={(e) => saveJavaPath(e.target.value)}
                />
              </label>

              <label>
                Width
                <input
                  type="number"
                  min={640}
                  value={resW}
                  onChange={(e) => setResW(Number(e.target.value))}
                  onBlur={saveExtendedSettings}
                />
              </label>
              <label>
                Height
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
                Full screen
              </label>
              <label className="full">
                JVM arguments
                <input
                  type="text"
                  value={jvmArgs}
                  placeholder="-XX:+UseG1GC …"
                  onChange={(e) => setJvmArgs(e.target.value)}
                  onBlur={saveExtendedSettings}
                />
              </label>

              <label className="check-row full">
                <input
                  type="checkbox"
                  checked={settings.ingame_branding !== false}
                  onChange={(e) => saveIngameBranding(e.target.checked)}
                />
                In-game Cubera branding (resource pack with logo &amp; splashes)
              </label>

              <div className="full log-block">
                <div className="log-head">
                  <h3>Launch log</h3>
                  <button
                    type="button"
                    className="btn-sm"
                    disabled={!selected || logLoading}
                    onClick={loadLaunchLog}
                  >
                    {logLoading ? "Loading…" : "Refresh"}
                  </button>
                </div>
                {!selected && (
                  <p className="hint">Select an instance to view logs.</p>
                )}
                {selected && launchLog && (
                  <pre className="log-view">
                    {launchLog.stderr && (
                      <>
                        <span className="log-label">stderr</span>
                        {launchLog.stderr}
                        {"\n\n"}
                      </>
                    )}
                    {launchLog.stdout || "(No stdout — launch the game to generate logs)"}
                  </pre>
                )}
              </div>

              <div className="full row-actions">
                <button type="button" className="cta secondary" onClick={openDataFolder}>
                  Open data folder
                </button>
              </div>

              <div className="full">
                <p className="hint">
                  Java: {javaPath ?? "not found — brew install --cask temurin"}
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
                Cancel
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
                Confirm
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
  return (
    <p className="hint" style={{ fontSize: "0.75rem", wordBreak: "break-all" }}>
      Data: {dir}
    </p>
  );
}
