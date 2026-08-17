import { useEffect } from "react";
import { convertFileSrc, formatBytes, formatDate } from "../api";
import type { MediaItem } from "../types";

export default function Lightbox({
  items,
  index,
  onClose,
  onNavigate,
}: {
  items: MediaItem[];
  index: number;
  onClose: () => void;
  onNavigate: (i: number) => void;
}) {
  const item = items[index];
  const src = item.previewPath || item.thumbnailPath;
  const isVideo = item.mediaType === "video";

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
      if (e.key === "ArrowRight") onNavigate(Math.min(index + 1, items.length - 1));
      if (e.key === "ArrowLeft") onNavigate(Math.max(index - 1, 0));
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [index, items.length, onClose, onNavigate]);

  if (!item) return null;

  return (
    <div className="lightbox" onClick={onClose}>
      <button className="lightbox-close" onClick={onClose}>✕</button>
      {index > 0 && (
        <button className="lightbox-nav prev" onClick={(e) => { e.stopPropagation(); onNavigate(index - 1); }}>‹</button>
      )}
      {index < items.length - 1 && (
        <button className="lightbox-nav next" onClick={(e) => { e.stopPropagation(); onNavigate(index + 1); }}>›</button>
      )}
      <div className="lightbox-media" onClick={(e) => e.stopPropagation()}>
        {src && !isVideo ? (
          <img src={convertFileSrc(src)} alt={item.fileName} />
        ) : isVideo && item.filePath ? (
          <video src={convertFileSrc(item.filePath)} controls autoPlay />
        ) : (
          <p className="muted">File tidak tersedia secara lokal. Pulihkan dari Telegram di tab Backup.</p>
        )}
        <div className="lightbox-meta">
          <strong>{item.fileName}</strong>
          <span>{formatBytes(item.fileSizeBytes)} · {formatDate(item.dateTaken)}</span>
          {item.geoCity && <span>📍 {item.geoCity}{item.geoCountry ? `, ${item.geoCountry}` : ""}</span>}
          {item.cameraModel && <span>📷 {item.cameraModel}{item.iso ? ` · ISO ${item.iso}` : ""}</span>}
          {item.errorMessage && <span className="error">⚠ {item.errorMessage}</span>}
        </div>
      </div>
    </div>
  );
}
