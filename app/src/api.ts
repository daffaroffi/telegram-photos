import { invoke } from "@tauri-apps/api/core";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  AppSettings,
  AuthCodeResult,
  BackupProgressEvent,
  FreeUpSpaceResult,
  GoogleDiscoveryInfo,
  GoogleImportResult,
  MediaItem,
  ReclaimableSpace,
  TelegramUser,
  VaultInfo,
  VaultStatus,
} from "./types";

export { convertFileSrc };

// ── Telegram auth ────────────────────────────────────────────────────────────
export const tgConnect = (apiId: number, apiHash: string) =>
  invoke<boolean>("cmd_connect", { apiId, apiHash });
export const tgCheckConnection = () => invoke<boolean>("cmd_check_connection");
export const tgGetMe = () => invoke<TelegramUser | null>("cmd_get_me");
export const tgRequestCode = (phone: string, apiId: number, apiHash: string) =>
  invoke<AuthCodeResult>("cmd_auth_request_code", { phone, apiId, apiHash });
export const tgSignIn = (code: string) =>
  invoke<AuthCodeResult>("cmd_auth_sign_in", { code });
export const tgCheckPassword = (password: string) =>
  invoke<AuthCodeResult>("cmd_auth_check_password", { password });
export const tgQrLogin = (apiId: number, apiHash: string) =>
  invoke<string>("cmd_auth_qr_login", { apiId, apiHash });
export const tgQrPoll = () => invoke<AuthCodeResult>("cmd_auth_qr_poll");
export const tgLogout = () => invoke<boolean>("cmd_logout");
export const tgGetVault = () => invoke<VaultInfo>("cmd_get_vault");
export const tgEnsureVault = () => invoke<VaultInfo>("cmd_get_or_create_vault");

// ── Backup engine ────────────────────────────────────────────────────────────
export const runBackup = () => invoke<number>("cmd_run_backup");
export const cancelBackup = () => invoke<boolean>("cmd_cancel_backup");
export const backupStatus = () => invoke<boolean>("cmd_backup_status");
export const reclaimableSpace = () =>
  invoke<ReclaimableSpace>("cmd_calculate_free_up_space");
export const executeFreeUpSpace = () =>
  invoke<FreeUpSpaceResult>("cmd_execute_free_up_space");
export const restoreMedia = (mediaId: string, destDir: string) =>
  invoke<string>("cmd_restore_media", { mediaId, destDir });

// ── Media queries ────────────────────────────────────────────────────────────
export const listTimeline = (beforeTimestamp?: number, limit?: number) =>
  invoke<MediaItem[]>("cmd_list_timeline", { beforeTimestamp, limit });
export const listAllMedia = () => invoke<MediaItem[]>("cmd_list_all_media");
export const getMedia = (id: string) => invoke<MediaItem | null>("cmd_get_media", { id });
export const countMedia = () => invoke<number>("cmd_count_media");
export const searchMedia = (query: string) =>
  invoke<MediaItem[]>("cmd_search_media", { query });

// ── Ingestion ────────────────────────────────────────────────────────────────
export const addLocalFiles = (paths: string[], deviceFolder: string) =>
  invoke<number>("cmd_add_local_files", { paths, deviceFolder });
export const scanFolder = (folder: string) =>
  invoke<number>("cmd_scan_folder", { folder });
export const scanGalleryAndroid = (folder?: string) =>
  invoke<number>("cmd_scan_gallery_android", { folder: folder ?? null });

// ── Batch operations ─────────────────────────────────────────────────────────
export const batchFavorite = (ids: string[], favorite: boolean) =>
  invoke<number>("cmd_batch_toggle_favorite", { ids, favorite });
export const batchTrash = (ids: string[]) => invoke<number>("cmd_batch_trash", { ids });
export const batchQueueBackup = (ids: string[]) =>
  invoke<number>("cmd_batch_queue_backup", { ids });
export const purgeTrash = () => invoke<number>("cmd_purge_trash");
export const deleteFromTelegram = (mediaId: string) =>
  invoke<boolean>("cmd_delete_media_from_telegram", { mediaId });

// ── Settings ─────────────────────────────────────────────────────────────────
export const getSettings = () => invoke<AppSettings>("cmd_get_settings");
export const saveSettings = (settings: AppSettings) =>
  invoke<boolean>("cmd_save_settings", { settings });

// ── Vault (encryption) ───────────────────────────────────────────────────────
export const vaultSetup = (passphrase: string) =>
  invoke<boolean>("cmd_vault_setup", { passphrase });
export const vaultUnlock = (passphrase: string) =>
  invoke<boolean>("cmd_vault_unlock", { passphrase });
export const vaultLock = () => invoke<boolean>("cmd_vault_lock");
export const vaultStatus = () => invoke<VaultStatus>("cmd_vault_status");

// ── Google Photos ────────────────────────────────────────────────────────────
export const googleStartOAuth = () => invoke<string>("cmd_google_start_oauth");
export const googleWaitOAuth = () => invoke<string>("cmd_google_wait_oauth");
export const googleDisconnect = () => invoke<boolean>("cmd_google_disconnect");
export const googleStatus = () => invoke<boolean>("cmd_google_status");
export const googleDiscover = () =>
  invoke<GoogleDiscoveryInfo>("cmd_google_discover");
export const googleStartImport = (includeAlbums: boolean) =>
  invoke<string>("cmd_google_start_import", { includeAlbums });
export const googleCancelImport = () => invoke<boolean>("cmd_google_cancel_import");
export const googlePostImport = (sessionId: string, choice: string) =>
  invoke<GoogleImportResult>("cmd_google_post_import", { sessionId, choice });

// ── Events ───────────────────────────────────────────────────────────────────
export function onBackupProgress(
  cb: (e: BackupProgressEvent) => void,
): Promise<() => void> {
  return listen<BackupProgressEvent>("backup-progress", (event) => cb(event.payload));
}

// ── Formatting helpers ───────────────────────────────────────────────────────
export function formatBytes(bytes: number): string {
  if (bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / Math.pow(1024, i)).toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}

export function formatDate(ts: number): string {
  if (!ts) return "—";
  return new Date(ts).toLocaleDateString("id-ID", {
    day: "numeric",
    month: "short",
    year: "numeric",
  });
}

export function formatMonth(ts: number): string {
  if (!ts) return "Tanpa tanggal";
  return new Date(ts).toLocaleDateString("id-ID", {
    month: "long",
    year: "numeric",
  });
}
