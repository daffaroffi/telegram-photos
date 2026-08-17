import { useEffect, useState } from "react";
import {
  getSettings,
  saveSettings,
  tgEnsureVault,
  tgQrLogin,
  tgQrPoll,
  tgRequestCode,
  tgSignIn,
  tgCheckPassword,
} from "../api";

type Step = "api" | "phone" | "code" | "password" | "qr";

export default function Onboarding({ onDone }: { onDone: () => void }) {
  const [step, setStep] = useState<Step>("api");
  const [apiId, setApiId] = useState("");
  const [apiHash, setApiHash] = useState("");
  const [phone, setPhone] = useState("");
  const [code, setCode] = useState("");
  const [password, setPassword] = useState("");
  const [qrUrl, setQrUrl] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [creatingVault, setCreatingVault] = useState(false);

  useEffect(() => {
    (async () => {
      const s = await getSettings().catch(() => null);
      if (s) {
        setApiId(s.telegramApiId ?? "");
        setApiHash(s.telegramApiHash ?? "");
        if (s.telegramApiId && s.telegramApiHash) setStep("phone");
      }
    })();
  }, []);

  async function saveCredentials() {
    setError("");
    if (!apiId.trim() || !apiHash.trim()) {
      setError("Isi API ID dan API Hash dari my.telegram.org.");
      return;
    }
    setBusy(true);
    try {
      const s = await getSettings();
      s.telegramApiId = apiId.trim();
      s.telegramApiHash = apiHash.trim();
      await saveSettings(s);
      setStep("phone");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function requestCode() {
    setError("");
    setBusy(true);
    try {
      await tgRequestCode(phone, Number(apiId), apiHash);
      setStep("code");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function submitCode() {
    setError("");
    setBusy(true);
    try {
      const r = await tgSignIn(code);
      if (r.status === "password_required") {
        setStep("password");
      } else if (r.status === "authorized") {
        await finalize();
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function submitPassword() {
    setError("");
    setBusy(true);
    try {
      const r = await tgCheckPassword(password);
      if (r.status === "authorized") await finalize();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function startQr() {
    setError("");
    setBusy(true);
    try {
      const url = await tgQrLogin(Number(apiId), apiHash);
      setQrUrl(url);
      setStep("qr");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  // QR polling
  useEffect(() => {
    if (step !== "qr") return;
    const timer = setInterval(async () => {
      try {
        const r = await tgQrPoll();
        if (r.status === "authorized") {
          clearInterval(timer);
          await finalize();
        }
      } catch {
        /* keep polling */
      }
    }, 1500);
    return () => clearInterval(timer);
  }, [step]);

  async function finalize() {
    setCreatingVault(true);
    try {
      // Create/find the private storage channel (PRD 4.2).
      await tgEnsureVault();
      onDone();
    } catch (e) {
      setError(`Masuk berhasil, tapi gagal menyiapkan vault: ${e}`);
      setCreatingVault(false);
    }
  }

  function qrImageUrl(url: string): string {
    // Render the tg://login token as a QR code via the public QR server.
    return `https://api.qrserver.com/v1/create-qr-code/?size=240x240&data=${encodeURIComponent(url)}`;
  }

  return (
    <div className="onboarding">
      <div className="onboard-card">
        <h1>Telegram Photos</h1>
        <p className="muted">
          Backup foto & video ke channel privat Telegram. Login dengan akun
          Telegram kamu untuk membuat vault penyimpanan.
        </p>

        {step === "api" && (
          <>
            <p className="muted small">
              Buat kredensial API di{" "}
              <a href="https://my.telegram.org/apps" target="_blank" rel="noreferrer">
                my.telegram.org/apps
              </a>{" "}
              (gratis), lalu tempel di sini.
            </p>
            <label>API ID</label>
            <input value={apiId} onChange={(e) => setApiId(e.target.value)} inputMode="numeric" placeholder="12345678" />
            <label>API Hash</label>
            <input value={apiHash} onChange={(e) => setApiHash(e.target.value)} placeholder="abcdef0123456789…" />
            <button className="primary" disabled={busy} onClick={saveCredentials}>
              {busy ? "Menyimpan…" : "Lanjut"}
            </button>
          </>
        )}

        {step === "phone" && (
          <>
            <label>Nomor telepon (format internasional)</label>
            <input value={phone} onChange={(e) => setPhone(e.target.value)} placeholder="+6281234567890" inputMode="tel" />
            <button className="primary" disabled={busy} onClick={requestCode}>
              {busy ? "Meminta kode…" : "Kirim kode"}
            </button>
            <button className="ghost" disabled={busy} onClick={startQr}>
              Masuk dengan kode QR
            </button>
            <button className="link" onClick={() => setStep("api")}>← Ubah kredensial API</button>
          </>
        )}

        {step === "code" && (
          <>
            <p className="muted">Masukkan kode verifikasi yang dikirim Telegram ke {phone}.</p>
            <input value={code} onChange={(e) => setCode(e.target.value)} inputMode="numeric" placeholder="12345" autoFocus />
            <button className="primary" disabled={busy || code.length === 0} onClick={submitCode}>
              {busy ? "Memverifikasi…" : "Verifikasi"}
            </button>
            <button className="link" onClick={() => setStep("phone")}>← Ganti nomor</button>
          </>
        )}

        {step === "password" && (
          <>
            <p className="muted">Akun ini memakai kata sandi 2FA.</p>
            <input type="password" value={password} onChange={(e) => setPassword(e.target.value)} placeholder="Kata sandi 2FA" autoFocus />
            <button className="primary" disabled={busy} onClick={submitPassword}>
              {busy ? "Memverifikasi…" : "Masuk"}
            </button>
          </>
        )}

        {step === "qr" && (
          <>
            <p className="muted">Scan kode ini dari Telegram (Setelan → Perangkat → Pindai Kode QR).</p>
            <div className="qr-box">
              <img src={qrImageUrl(qrUrl)} alt="QR login" width={220} height={220} />
            </div>
            <button className="ghost" disabled={busy} onClick={() => setStep("phone")}>
              Gunakan nomor telepon
            </button>
          </>
        )}

        {creatingVault && <p className="muted">Menyiapkan channel vault privat…</p>}
        {error && <p className="error">{error}</p>}
      </div>
    </div>
  );
}
