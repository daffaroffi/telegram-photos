import { useCallback, useEffect, useRef, useState } from "react";
import {
  cancelBackup,
  executeFreeUpSpace,
  listAllMedia,
  onBackupProgress,
  reclaimableSpace,
  restoreMedia,
  runBackup,
  tgGetVault,
  vaultLock,
  vaultSetup,
  vaultUnlock,
} from "../api";
import type { AppSettings, BackupProgressEvent, ReclaimableSpace, VaultInfo, VaultStatus } from "../types";
import { formatBytes } from "../api";
import { vaultStatus } from "../api";

export default function BackupScreen({
  settings,
  onSettingsChange,
  onVaultChange,
}: {
  settings: AppSettings;
  onSettingsChange: (s: AppSettings) => void;
  onVaultChange: () => void;
}) {
  const [running, setRunning] = useState(false);
  const [lastEvent, setLastEvent] = useState<BackupProgressEvent | null>(null);
  const [doneCount, setDoneCount] = useState(0);
  const [vault, setVault] = useState<VaultInfo | null>(null);
  const [vaultSt, setVaultSt] = useState<VaultStatus | null>(null);
  const [passphrase, setPassphrase] = useState("");
  const [pass2, setPass2] = useState("");
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState("");
  const [reclaim, setReclaim] = useState<ReclaimableSpace | null>(null);
  const [stats, setStats] = useState({ total: 0, backed: 0, pending: 0, failed: 0 });
  const unsub = useRef<(() => void) | null>(null);

  const flash = (m: string) => {
    setMsg(m);
    window.setTimeout(() => setMsg(""), 3000);
  };

  const refresh = useCallback(async () => {
    const [v, vs, r, all] = await Promise.all([
      tgGetVault().catch(() => null),
      vaultStatus().catch(() => null),
      reclaimableSpace().catch(() => null),
      listAllMedia().catch(() => []),
    ]);
    setVault(v);
    setVaultSt(vs);
    setReclaim(r);
    setStats({
      total: all.length,
      backed: all.filter((i) => i.syncStatus === "BACKED_UP" || i.syncStatus === "CLOUD_ONLY").length,
      pending: all.filter((i) => ["NOT_BACKED_UP", "QUEUED", "FAILED", "UPLOADING"].includes(i.syncStatus)).length,
      failed: all.filter((i) => i.syncStatus === "FAILED").length,
    });
  }, []);

  useEffect(() => {
    refresh();
    onBackupProgress((e) => {
      setLastEvent(e);
      if (e.status === "BACKED_UP") setDoneCount((c) => c + 1);
    }).then((u) => (unsub.current = u));
    return () => {
      unsub.current?.();
    };
  }, [refresh]);

  async function start() {
    setBusy(true);
    setDoneCount(0);
    try {
      setRunning(true);
      const n = await runBackup();
      setRunning(false);
      flash(`Backup selesai: ${n} item terupload`);
      await refresh();
    } catch (e) {
      setRunning(false);
      flash(`Backup gagal: ${e}`);
    } finally {
      setBusy(false);
    }
  }

  async function stop() {
    await cancelBackup();
    setRunning(false);
    flash("Membatalkan…");
  }

  async function doVaultSetup() {
    if (passphrase.length < 8) {
      flash("Passphrase minimal 8 karakter.");
      return;
    }
    if (passphrase !== pass2) {
      flash("Passphrase tidak cocok.");
      return;
    }
    setBusy(true);
    try {
      await vaultSetup(passphrase);
      flash("Vault diaktifkan. Enkripsi zero-knowledge aktif.");
      setPassphrase("");
      setPass2("");
      onVaultChange();
      await refresh();
    } catch (e) {
      flash(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function doVaultUnlock() {
    setBusy(true);
    try {
      await vaultUnlock(passphrase);
      flash("Vault terbuka.");
      setPassphrase("");
      onVaultChange();
      await refresh();
    } catch (e) {
      flash(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function doVaultLock() {
    await vaultLock();
    onVaultChange();
    await refresh();
  }

  async function doFreeUp() {
    if (!reclaim || reclaim.count === 0) return;
    if (!window.confirm(`Hapus ${reclaim.count} file lokal (${formatBytes(reclaim.totalSizeBytes)})? Salinan aman di Telegram.`)) return;
    setBusy(true);
    try {
      const r = await executeFreeUpSpace();
      flash(`Ruang dibebaskan: ${r.freedCount} item, ${formatBytes(r.freedBytes)}`);
      await refresh();
    } catch (e) {
      flash(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function doRestoreAll() {
    const cloudOnly = (await listAllMedia().catch(() => [])).filter(
      (i) => i.syncStatus === "CLOUD_ONLY" && i.tgMessageId,
    );
    if (!cloudOnly.length) {
      flash("Tidak ada item cloud-only.");
      return;
    }
    setBusy(true);
    let ok = 0;
    let fail = 0;
    for (const item of cloudOnly.slice(0, 20)) {
      try {
        await restoreMedia(item.id, "/storage/emulated/0/Download");
        ok++;
      } catch {
        fail++;
      }
    }
    flash(`Dipulihkan: ${ok} berhasil, ${fail} gagal (ke folder Download).`);
    setBusy(false);
  }

  const pct = lastEvent?.percent ?? 0;

  return (
    <div className="screen">
      <section className="card">
        <h2>Pencadangan</h2>
        <div className="stat-row">
          <div><strong>{stats.total}</strong><span>Total</span></div>
          <div><strong>{stats.backed}</strong><span>Tercadangkan</span></div>
          <div><strong>{stats.pending}</strong><span>Antrean</span></div>
          <div><strong>{stats.failed}</strong><span>Gagal</span></div>
        </div>
        {!running ? (
          <button className="primary big" disabled={busy} onClick={start}>
            {busy ? "Menghubungi Telegram…" : "▲ Mulai backup sekarang"}
          </button>
        ) : (
          <div className="progress-box">
            <div className="bar"><div className="fill" style={{ width: `${pct}%` }} /></div>
            <p className="muted small">
              {lastEvent ? `${lastEvent.fileName} — ${pct}%` : "Menyiapkan…"}
              {doneCount > 0 ? ` · ${doneCount} selesai` : ""}
            </p>
            <button className="ghost" onClick={stop}>Batal</button>
          </div>
        )}
        <label className="toggle">
          <input
            type="checkbox"
            checked={settings.autoBackupEnabled}
            onChange={(e) => {
              const s = { ...settings, autoBackupEnabled: e.target.checked };
              onSettingsChange(s);
            }}
          />
          <span>Auto-backup (15 menit, di background)</span>
        </label>
        <label className="toggle">
          <input
            type="checkbox"
            checked={settings.backupOverWifiOnly}
            onChange={(e) => {
              const s = { ...settings, backupOverWifiOnly: e.target.checked };
              onSettingsChange(s);
            }}
          />
          <span>Hanya via Wi-Fi</span>
        </label>
        <label className="toggle">
          <input
            type="checkbox"
            checked={settings.backupWhileChargingOnly}
            onChange={(e) => {
              const s = { ...settings, backupWhileChargingOnly: e.target.checked };
              onSettingsChange(s);
            }}
          />
          <span>Hanya saat mengisi daya</span>
        </label>
      </section>

      <section className="card">
        <h2>Vault Telegram</h2>
        {vault ? (
          <div className="vault-info">
            <p><strong>{vault.channelTitle}</strong> {vault.isPrivate ? "🔒 privat" : "⚠ publik"}</p>
            <p className="muted small">
              {vault.totalBackedUpFiles} file · {formatBytes(vault.totalStorageUsedBytes)}
            </p>
          </div>
        ) : (
          <p className="muted">Vault belum disiapkan.</p>
        )}
      </section>

      <section className="card">
        <h2>Enkripsi Vault (Zero-Knowledge)</h2>
        {!settings.clientEncryptionEnabled ? (
          <>
            <p className="muted small">
              Aktifkan enkripsi XChaCha20 sebelum upload. File dienkripsi di
              perangkat dengan passphrase — Telegram tidak bisa membacanya.
            </p>
            <input type="password" placeholder="Passphrase baru (min 8 karakter)" value={passphrase} onChange={(e) => setPassphrase(e.target.value)} />
            <input type="password" placeholder="Ulangi passphrase" value={pass2} onChange={(e) => setPass2(e.target.value)} />
            <button className="primary" disabled={busy} onClick={doVaultSetup}>Aktifkan enkripsi</button>
          </>
        ) : vaultSt?.unlocked ? (
          <>
            <p className="muted small">🔓 Vault terbuka — upload akan dienkripsi.</p>
            <button className="ghost" onClick={doVaultLock}>Kunci vault</button>
          </>
        ) : (
          <>
            <p className="muted small">🔒 Vault terkunci. Buka untuk mengaktifkan enkripsi upload.</p>
            <input type="password" placeholder="Passphrase" value={passphrase} onChange={(e) => setPassphrase(e.target.value)} />
            <button className="primary" disabled={busy} onClick={doVaultUnlock}>Buka vault</button>
          </>
        )}
      </section>

      <section className="card">
        <h2>Bebaskan Ruang</h2>
        {reclaim && reclaim.count > 0 ? (
          <>
            <p className="muted small">
              {reclaim.count} file lokal ({formatBytes(reclaim.totalSizeBytes)}) sudah aman di
              Telegram dan bisa dihapus dari perangkat.
            </p>
            <button className="ghost" disabled={busy} onClick={doFreeUp}>Hapus salinan lokal</button>
          </>
        ) : (
          <p className="muted small">Tidak ada file yang bisa dibebaskan.</p>
        )}
        <button className="link" disabled={busy} onClick={doRestoreAll}>
          Pulihkan item cloud-only ke Download
        </button>
      </section>

      {msg && <div className="toast">{msg}</div>}
    </div>
  );
}
