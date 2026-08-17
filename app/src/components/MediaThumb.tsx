import { convertFileSrc } from "../api";
import type { MediaItem } from "../types";

export default function MediaThumb({
  item,
  selected,
  onClick,
  onLongPress,
}: {
  item: MediaItem;
  selected?: boolean;
  onClick?: () => void;
  onLongPress?: () => void;
}) {
  const src = item.thumbnailPath ? convertFileSrc(item.thumbnailPath) : undefined;
  const isVideo = item.mediaType === "video";
  const status = item.syncStatus;

  function handlePointer(e: React.PointerEvent) {
    if (e.pointerType === "touch" && e.type === "pointerdown") {
      // Long-press = multi-select on touch.
      const el = e.currentTarget as HTMLElement;
      const t = window.setTimeout(() => {
        onLongPress?.();
      }, 450);
      const clear = () => {
        window.clearTimeout(t);
        el.removeEventListener("pointerup", clear);
        el.removeEventListener("pointercancel", clear);
        el.removeEventListener("pointermove", clear);
      };
      el.addEventListener("pointerup", clear, { once: true });
      el.addEventListener("pointercancel", clear, { once: true });
      el.addEventListener("pointermove", clear, { once: true });
    }
  }

  return (
    <div
      className={`thumb ${selected ? "selected" : ""} ${status === "BACKED_UP" || status === "CLOUD_ONLY" ? "backed" : ""}`}
      onClick={onClick}
      onPointerDown={handlePointer}
    >
      {src ? (
        <img src={src} alt={item.fileName} loading="lazy" draggable={false} />
      ) : (
        <div className="thumb-placeholder">
          {item.blurHash ? "◍" : "▢"}
        </div>
      )}
      {isVideo && <span className="video-badge">▶</span>}
      {item.isFavorite && <span className="fav-badge">♥</span>}
      {item.isEncrypted && <span className="enc-badge">🔒</span>}
      {selected && <span className="sel-badge">✓</span>}
      {status === "FAILED" && <span className="fail-badge">!</span>}
    </div>
  );
}
