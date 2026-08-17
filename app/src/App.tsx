import { useCallback, useEffect, useState } from "react";
import { getSettings, tgCheckConnection, tgGetMe } from "./api";
import type { AppSettings, TelegramUser } from "./types";
import Onboarding from "./components/Onboarding";
import Gallery from "./components/Gallery";
import BackupScreen from "./components/BackupScreen";
import GoogleImport from "./components/GoogleImport";
import SettingsScreen from "./components/SettingsScreen";
type Tab = "gallery" | "backup" | "google" | "settings";

export default function App() {
  const [ready, setReady] = useState(false);
  const [connected, setConnected] = useState(false);
  const [me, setMe] = useState<TelegramUser | null>(null);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [tab, setTab] = useState<Tab>("gallery");

  const refreshConnection = useCallback(async () => {
    const ok = await tgCheckConnection().catch(() => false);
    setConnected(ok);
    if (ok) {
      const user = await tgGetMe().catch(() => null);
      setMe(user);
    } else {
      setMe(null);
    }
  }, []);

  const refreshSettings = useCallback(async () => {
    const s = await getSettings().catch(() => null);
    setSettings(s);
  }, []);

  useEffect(() => {
    (async () => {
      await Promise.all([refreshConnection(), refreshSettings()]);
      setReady(true);
    })();
  }, [refreshConnection, refreshSettings]);

  useEffect(() => {
    const theme = settings?.theme ?? "system";
    const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
    const dark = theme === "dark" || (theme === "system" && prefersDark);
    document.documentElement.dataset.theme = dark ? "dark" : "light";
  }, [settings]);

  if (!ready || !settings) {
    return <div className="boot">Memuat…</div>;
  }

  if (!connected) {
    return <Onboarding onDone={refreshConnection} />;
  }

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          <span className="brand-dot" />
          Telegram Photos
        </div>
        {me && (
          <div className="account" title={`${me.firstName} ${me.lastName ?? ""}`}>
            {me.firstName.charAt(0).toUpperCase()}
            {me.lastName ? me.lastName.charAt(0).toUpperCase() : ""}
          </div>
        )}
      </header>

      <main className="content">
        {tab === "gallery" && (
          <Gallery settings={settings} onSettingsChange={(s) => setSettings(s)} />
        )}
        {tab === "backup" && (
          <BackupScreen
            settings={settings}
            onSettingsChange={(s) => setSettings(s)}
            onVaultChange={() => {}}
          />
        )}
        {tab === "google" && <GoogleImport settings={settings} onSettingsChange={setSettings} />}
        {tab === "settings" && (
          <SettingsScreen
            settings={settings}
            onSettingsChange={setSettings}
            onLogout={async () => {
              await refreshConnection();
            }}
          />
        )}
      </main>

      <nav className="tabbar">
        <TabButton active={tab === "gallery"} onClick={() => setTab("gallery")} icon="▦" label="Galeri" />
        <TabButton active={tab === "backup"} onClick={() => setTab("backup")} icon="▲" label="Backup" />
        <TabButton active={tab === "google"} onClick={() => setTab("google")} icon="☁" label="Google" />
        <TabButton active={tab === "settings"} onClick={() => setTab("settings")} icon="⚙" label="Atur" />
      </nav>
    </div>
  );
}

function TabButton({
  active,
  onClick,
  icon,
  label,
}: {
  active: boolean;
  onClick: () => void;
  icon: string;
  label: string;
}) {
  return (
    <button className={`tab ${active ? "active" : ""}`} onClick={onClick}>
      <span className="tab-icon">{icon}</span>
      <span className="tab-label">{label}</span>
    </button>
  );
}
