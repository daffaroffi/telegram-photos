import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  batchFavorite,
  batchQueueBackup,
  batchTrash,
  listAllMedia,
  purgeTrash,
  scanFolder,
  scanGalleryAndroid,
  searchMedia,
} from "../api";
import type { AppSettings, MediaItem } from "../types";
import { formatBytes, formatMonth } from "../api";
import MediaThumb from "./MediaThumb";
import Lightbox from "./Lightbox";

type View = "all" | "trash" | "search";

export default function Gallery({
  settings,
  onSettingsChange,
}: {
  settings: AppSettings;
  onSettingsChange: (s: AppSettings) => void;
}) {
  const [items, setItems] = useState<MediaItem[]>([]);
  const [view, setView] = useState<View>("all");
  const [query, setQuery] = useState("");
  const [searchResults, setSearchResults] = useState<MediaItem[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [lightbox, setLightbox] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState("");
  const [scanProgress, setScanProgress] = useState("");
  const scrubRef = useRef<HTMLDivElement>(null);

  const cols = settings.gridColumnCount;

  const load = useCallback(async () => {
    const all = await listAllMedia().catch(() => []);
    setItems(all);
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  useEffect(() => {
    if (view === "search" && query.trim()) {
      const t = setTimeout(async () => {
        const r = await searchMedia(query).catch(() => []);
        setSearchResults(r);
      }, 250);
      return () => clearTimeout(t);
    }
  }, [query, view]);

  const visible = useMemo(() => {
    if (view === "trash") return items.filter((i) => i.isTrashed);
    if (view === "search") return searchResults;
    return items.filter((i) => !i.isTrashed);
  }, [items, view, searchResults]);

  // Group by month for sticky headers.
  const groups = useMemo(() => {
    const map = new Map<string, MediaItem[]>();
    for (const item of [...visible].sort((a, b) => b.dateTaken - a.dateTaken)) {
      const key = formatMonth(item.dateTaken);
      if (!map.has(key)) map.set(key, []);
      map.get(key)!.push(item);
    }
    return [...map.entries()];
  }, [visible]);

  const months = useMemo(
    () => [...new Set(visible.map((i) => formatMonth(i.dateTaken)))],
    [visible],
  );

  const toggleSelect = useCallback((id: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const flash = (m: string) => {
    setMsg(m);
    window.setTimeout(() => setMsg(""), 2500);
  };

  async function doScanAndroid() {
    setBusy(true);
    setScanProgress("Memindai galeri perangkat…");
    try {
      const n = await scanGalleryAndroid(undefined);
      flash(`Galeri diperbarui: ${n} media baru`);
      await load();
    } catch (e) {
      flash(`Gagal memindai: ${e}`);
    } finally {
      setBusy(false);
      setScanProgress("");
    }
  }

  async function doPickFolder() {
    const dir = await open({ directory: true }).catch(() => null);
    if (!dir) return;
    setBusy(true);
    setScanProgress(`Memindai ${dir}…`);
    try {
      const n = await scanFolder(dir);
      flash(`Ditambahkan: ${n} media`);
      await load();
    } catch (e) {
      flash(`Gagal: ${e}`);
    } finally {
      setBusy(false);
      setScanProgress("");
    }
  }

  async function doBatch(fn: (ids: string[]) => Promise<number>, ok: string) {
    const ids = [...selected];
    if (!ids.length) return;
    setBusy(true);
    try {
      const n = await fn(ids);
      flash(`${ok}: ${n} item`);
      setSelected(new Set());
      await load();
    } catch (e) {
      flash(`Gagal: ${e}`);
    } finally {
      setBusy(false);
    }
  }

  async function doPurgeTrash() {
    if (!window.confirm("Hapus permanen item di Sampah yang berusia > 30 hari?")) return;
    setBusy(true);
    try {
      const n = await purgeTrash();
      flash(`${n} item dihapus permanen`);
      await load();
    } catch (e) {
      flash(`Gagal: ${e}`);
    } finally {
      setBusy(false);
    }
  }

  function scrollToMonth(m: string) {
    const idx = groups.findIndex(([k]) => k === m);
    if (idx < 0) return;
    const el = document.getElementById(`group-${idx}`);
    el?.scrollIntoView({ behavior: "smooth", block: "start" });
  }

  const selectionMode = selected.size > 0;
  const backedUpCount = items.filter(
    (i) => i.syncStatus === "BACKED_UP" || i.syncStatus === "CLOUD_ONLY",
  ).length;
  const totalBytes = items.reduce((s, i) => s + i.fileSizeBytes, 0);

  return (
    <div className="gallery">
      <div className="gallery-tools">
        <input
          className="search"
          placeholder="Cari kota, negara, kamera, nama file…"
          value={view === "search" ? query : ""}
          onChange={(e) => {
            setQuery(e.target.value);
            setView(e.target.value.trim() ? "search" : "all");
          }}
        />
        <div className="row-btns">
          <button className="chip" onClick={() => setView("all")}>Semua</button>
          <button className="chip" onClick={() => setView("trash")}>Sampah</button>
          <button
            className="chip"
            onClick={() => onSettingsChange({ ...settings, gridColumnCount: cols === 8 ? 1 : cols === 1 ? 3 : cols + 2 })}
            title="Ubah jumlah kolom"
          >
            {cols} kolom
          </button>
        </div>
        <div className="row-btns">
          <button className="chip" disabled={busy} onClick={doScanAndroid}>🔄 Pindai galeri</button>
          <button className="chip" disabled={busy} onClick={doPickFolder}>📁 Tambah folder</button>
          {view === "trash" && (
            <button className="chip danger" onClick={doPurgeTrash}>Hapus lama</button>
          )}
        </div>
        {scanProgress && <p className="muted small">{scanProgress}</p>}
      </div>

      {selectionMode && (
        <div className="selection-bar">
          <span>{selected.size} dipilih</span>
          <button onClick={() => doBatch((ids) => batchQueueBackup(ids), "Diantrekan")}>▲ Backup</button>
          <button onClick={() => doBatch((ids) => batchFavorite(ids, true), "Difavoritkan")}>♥</button>
          <button onClick={() => doBatch((ids) => batchTrash(ids), "Ke sampah")}>🗑</button>
          <button onClick={() => setSelected(new Set())}>Batal</button>
        </div>
      )}

      <div className="stats muted small">
        {items.length} media · {backedUpCount} tercadangkan · {formatBytes(totalBytes)}
      </div>

      <div className="timeline" ref={scrubRef}>
        {groups.length === 0 && (
          <div className="empty">
            <p>Belum ada media.</p>
            <p className="muted small">Pindai galeri perangkat atau tambahkan folder untuk mulai.</p>
          </div>
        )}
        {groups.map(([month, group], gi) => (
          <div key={month} id={`group-${gi}`} className="group">
            <div className="group-header">
              <span>{month}</span>
              <span className="muted small">{group.length} item</span>
            </div>
            <div className={`grid cols-${cols}`}>
              {group.map((item) => (
                <MediaThumb
                  key={item.id}
                  item={item}
                  selected={selected.has(item.id)}
                  onClick={() => {
                    if (selectionMode) toggleSelect(item.id);
                    else setLightbox(visible.indexOf(item));
                  }}
                  onLongPress={() => toggleSelect(item.id)}
                />
              ))}
            </div>
          </div>
        ))}
      </div>

      {months.length > 1 && (
        <div className="scrubber">
          {months.map((m) => (
            <button key={m} onClick={() => scrollToMonth(m)} title={m}>
              {m.slice(0, 3)}
            </button>
          ))}
        </div>
      )}

      {lightbox !== null && (
        <Lightbox
          items={visible}
          index={lightbox}
          onClose={() => setLightbox(null)}
          onNavigate={(i) => setLightbox(i)}
        />
      )}

      {msg && <div className="toast">{msg}</div>}
    </div>
  );
}
