import { useState } from "react";
import { saveSettings, tgLogout } from "../api";
import type { AppSettings } from "../types";

export default function SettingsScreen({
  settings,
  onSettingsChange,
  onLogout,
}: {
  settings: AppSettings;
  onSettingsChange: (s: AppSettings) => void;
  onLogout: () => void;
}) {
  const [saving, setSaving] = useState(false);
  const [msg, setMsg] = useState("");
  const [apiId, setApiId] = useState(settings.telegramApiId ?? "");
  const [apiHash, setApiHash] = useState(settings.telegramApiHash ?? "");

  const flash = (m: string) => {
    setMsg(m);
    window.setTimeout(() => setMsg(""), 2500);
  };

  const update = (patch: Partial<AppSettings>) => {
    onSettingsChange({ ...settings, ...patch });
  };

  async function persist() {
    setSaving(true);
    try {
      const s = {
        ...settings,
        telegramApiId: apiId.trim() || undefined,
        telegramApiHash: apiHash.trim() || undefined,
      };
      await saveSettings(s);
      onSettingsChange(s);
      flash("Pengaturan disimpan.");
    } catch (e) {
      flash(`Gagal menyimpan: ${e}`);
    } finally {
      setSaving(false);
    }
  }

  async function doLogout() {
    if (!window.confirm("Keluar dari akun Telegram? Sesi login akan dihapus.")) return;
    await tgLogout();
    onLogout();
  }

  return (
    <div className="screen">
      <section className="card">
        <h2>Telegram API</h2>
        <label>API ID</label>
        <input value={apiId} onChange={(e) => setApiId(e.target.value)} inputMode="numeric" />
        <label>API Hash</label>
        <input value={apiHash} onChange={(e) => setApiHash(e.target.value)} />
        <button className="chip" disabled={saving} onClick={persist}>Simpan kredensial</button>
        <button className="chip danger" onClick={doLogout}>Keluar dari Telegram</button>
      </section>

      <section className="card">
        <h2>Folder yang di-backup</h2>
        <p className="muted small">Matikan folder yang tidak ingin dicadangkan otomatis.</p>
        {Object.entries(settings.folderBackupSettings).map(([folder, enabled]) => (
          <label className="toggle" key={folder}>
            <input
              type="checkbox"
              checked={enabled}
              onChange={(e) =>
                update({
                  folderBackupSettings: { ...settings.folderBackupSettings, [folder]: e.target.checked },
                })
              }
            />
            <span>{folder}</span>
          </label>
        ))}
      </section>

      <section className="card">
        <h2>Tampilan</h2>
        <label>Jumlah kolom grid</label>
        <select
          value={settings.gridColumnCount}
          onChange={(e) => update({ gridColumnCount: Number(e.target.value) })}
        >
          <option value={1}>1 kolom</option>
          <option value={3}>3 kolom</option>
          <option value={5}>5 kolom</option>
          <option value={8}>8 kolom</option>
        </select>
        <label>Tema</label>
        <select
          value={settings.theme}
          onChange={(e) => update({ theme: e.target.value as AppSettings["theme"] })}
        >
          <option value="system">Sistem</option>
          <option value="light">Terang</option>
          <option value="dark">Gelap</option>
        </select>
        <label className="toggle">
          <input
            type="checkbox"
            checked={settings.uploadOriginalQuality}
            onChange={(e) => update({ uploadOriginalQuality: e.target.checked })}
          />
          <span>Upload kualitas asli</span>
        </label>
      </section>

      <section className="card">
        <h2>Tentang</h2>
        <p className="muted small">
          Telegram Photos — backup galeri ke channel privat Telegram.
          <br />
          MTProto (Grammers) · XChaCha20-Poly1305 · MediaStore · WorkManager
        </p>
      </section>

      <button className="primary" disabled={saving} onClick={persist}>Simpan semua</button>
      {msg && <div className="toast">{msg}</div>}
    </div>
  );
}
