import { useEffect, useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { listen } from "@tauri-apps/api/event";
import {
  googleCancelImport,
  googleDiscover,
  googleDisconnect,
  googlePostImport,
  googleStartImport,
  googleStartOAuth,
  googleStatus,
  googleWaitOAuth,
  saveSettings,
} from "../api";
import type { AppSettings, GoogleDiscoveryInfo } from "../types";
import { formatBytes } from "../api";

interface ImportProgress {
  sessionId: string;
  current: number;
  total: number;
  success: number;
  failed: number;
  bytesMigrated: number;
  status: string;
}

export default function GoogleImport({
  settings,
  onSettingsChange,
}: {
  settings: AppSettings;
  onSettingsChange: (s: AppSettings) => void;
}) {
  const [clientId, setClientId] = useState(settings.googleClientId ?? "");
  const [clientSecret, setClientSecret] = useState(settings.googleClientSecret ?? "");
  const [connected, setConnected] = useState(false);
  const [discovery, setDiscovery] = useState<GoogleDiscoveryInfo | null>(null);
  const [progress, setProgress] = useState<ImportProgress | null>(null);
  const [sessionId, setSessionId] = useState("");
  const [cleanupResult, setCleanupResult] = useState("");
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState("");
  const [includeAlbums, setIncludeAlbums] = useState(true);
  const unsub = useRef<(() => void) | null>(null);

  const flash = (m: string) => {
    setMsg(m);
    window.setTimeout(() => setMsg(""), 4000);
  };

  useEffect(() => {
    googleStatus().then(setConnected).catch(() => setConnected(false));
    listen<ImportProgress>("google-import-progress", (e) => {
      setProgress(e.payload);
      setSessionId(e.payload.sessionId);
    }).then((u) => (unsub.current = u));
    return () => {
      unsub.current?.();
    };
  }, []);

  async function saveCredentials() {
    if (!clientId.trim() || !clientSecret.trim()) {
      flash("Isi Client ID dan Client Secret.");
      return;
    }
    const s = { ...settings, googleClientId: clientId.trim(), googleClientSecret: clientSecret.trim() };
    await saveSettings(s);
    onSettingsChange(s);
    flash("Kredensial Google disimpan.");
  }

  async function connect() {
    setBusy(true);
    try {
      const url = await googleStartOAuth();
      await openUrl(url);
      const result = await googleWaitOAuth();
      if (result === "connected") {
        setConnected(true);
        flash("Terhubung ke Google Photos!");
        const d = await googleDiscover().catch(() => null);
        setDiscovery(d);
      } else {
        flash(`OAuth: ${result}`);
      }
    } catch (e) {
      flash(`Gagal OAuth: ${e}`);
    } finally {
      setBusy(false);
    }
  }

  async function discover() {
    setBusy(true);
    try {
      const d = await googleDiscover();
      setDiscovery(d);
      flash(`Ditemukan ${d.totalCount} item di Google Photos.`);
    } catch (e) {
      flash(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function startImport() {
    setBusy(true);
    setCleanupResult("");
    setProgress(null);
    try {
      const sid = await googleStartImport(includeAlbums);
      setSessionId(sid);
    } catch (e) {
      flash(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function cancelImport() {
    await googleCancelImport();
    flash("Membatalkan import…");
  }

  async function postImport(choice: "DELETE_FROM_GOOGLE" | "KEEP_IN_GOOGLE") {
    setBusy(true);
    try {
      const r = await googlePostImport(sessionId, choice);
      setCleanupResult(
        r.choice === "DELETE_FROM_GOOGLE"
          ? `${r.deletedCount} item ditandai aman. ${r.freedBytes ? `${formatBytes(r.freedBytes)} bisa dibebaskan.` : ""} ${r.note ?? ""}`
          : "Item tetap di Google Photos.",
      );
    } catch (e) {
      flash(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function disconnect() {
    await googleDisconnect();
    setConnected(false);
    setDiscovery(null);
    setSessionId("");
    setCleanupResult("");
    flash("Google Photos diputuskan.");
  }

  const done = progress && progress.total > 0 && progress.current >= progress.total;

  return (
    <div className="screen">
      <section className="card">
        <h2>Migrasi Google Photos</h2>
        <p className="muted small">
          Impor semua foto & video dari Google Photos ke vault Telegram, lalu
          bebaskan kuota Google. Stream langsung cloud-ke-cloud (tanpa simpan
          ganda di perangkat).
        </p>
        <label>Google Client ID</label>
        <input value={clientId} onChange={(e) => setClientId(e.target.value)} placeholder="xxxx.apps.googleusercontent.com" />
        <label>Google Client Secret</label>
        <input value={clientSecret} onChange={(e) => setClientSecret(e.target.value)} placeholder="GOCSPX-…" />
        <div className="row-btns">
          <button className="chip" disabled={busy} onClick={saveCredentials}>Simpan kredensial</button>
          {!connected && (
            <button className="primary" disabled={busy} onClick={connect}>
              {busy ? "Menunggu login…" : "Hubungkan Google"}
            </button>
          )}
        </div>
        {connected && (
          <div className="row-btns">
            <button className="chip" disabled={busy} onClick={discover}>Hitung item</button>
            <button className="chip danger" onClick={disconnect}>Putuskan</button>
          </div>
        )}
        {connected && <p className="ok small">✓ Terhubung ke Google Photos</p>}
      </section>

      {discovery && (
        <section className="card">
          <h2>Hasil Pindai</h2>
          <p className="muted small">
            {discovery.totalCount} item · {formatBytes(discovery.totalSizeBytes)}
          </p>
          {discovery.albums.length > 0 && (
            <p className="muted small">Album: {discovery.albums.slice(0, 8).join(", ")}{discovery.albums.length > 8 ? "…" : ""}</p>
          )}
          <label className="toggle">
            <input type="checkbox" checked={includeAlbums} onChange={(e) => setIncludeAlbums(e.target.checked)} />
            <span>Impor juga struktur album</span>
          </label>
          {!progress && (
            <button className="primary big" disabled={busy} onClick={startImport}>
              {busy ? "Memulai…" : "☁ Mulai migrasi"}
            </button>
          )}
        </section>
      )}

      {progress && (
        <section className="card">
          <h2>Migrasi Berjalan</h2>
          <div className="bar">
            <div className="fill" style={{ width: `${progress.total ? (progress.current / progress.total) * 100 : 0}%` }} />
          </div>
          <p className="muted small">
            {progress.current}/{progress.total} item · {progress.success} berhasil · {progress.failed} gagal
            {progress.bytesMigrated ? ` · ${formatBytes(progress.bytesMigrated)}` : ""}
          </p>
          {!done && <button className="ghost" onClick={cancelImport}>Batalkan</button>}
        </section>
      )}

      {done && !cleanupResult && (
        <section className="card">
          <h2>Migrasi Selesai ✅</h2>
          <p className="muted small">
            Semua item sudah aman di Telegram. Mau apa yang dilakukan dengan
            salinan di Google Photos?
          </p>
          <div className="row-btns">
            <button className="chip" disabled={busy} onClick={() => postImport("KEEP_IN_GOOGLE")}>
              Biarkan di Google
            </button>
            <button className="chip danger" disabled={busy} onClick={() => postImport("DELETE_FROM_GOOGLE")}>
              Hapus dari Google
            </button>
          </div>
        </section>
      )}

      {cleanupResult && <p className="ok small">{cleanupResult}</p>}
      {msg && <div className="toast">{msg}</div>}
    </div>
  );
}
