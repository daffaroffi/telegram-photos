// Types mirroring the Rust backend (camelCase serialization).

export interface TelegramUser {
  id: number;
  firstName: string;
  lastName?: string;
  username?: string;
  phone: string;
  isPremium: boolean;
}

export interface VaultInfo {
  channelId?: number;
  channelTitle: string;
  isPrivate: boolean;
  totalStorageUsedBytes: number;
  totalBackedUpFiles: number;
  lastSyncTimestamp: number;
}

export interface AuthCodeResult {
  status: "code_required" | "password_required" | "authorized" | "waiting" | string;
  codeLength?: number;
  resendAfterSeconds?: number;
  delivery?: string;
}

export interface MediaItem {
  id: string;
  localIdentifier?: string;
  fileName: string;
  filePath?: string;
  mimeType: string;
  mediaType: "image" | "video" | string;
  fileSizeBytes: number;
  sha256Hash: string;
  dateTaken: number;
  dateAdded: number;
  width?: number;
  height?: number;
  orientation?: number;
  durationMs?: number;
  cameraMake?: string;
  cameraModel?: string;
  focalLength?: number;
  aperture?: number;
  iso?: number;
  exposureTime?: string;
  latitude?: number;
  longitude?: number;
  geoCity?: string;
  geoCountry?: string;
  syncStatus:
    | "NOT_BACKED_UP"
    | "QUEUED"
    | "UPLOADING"
    | "BACKED_UP"
    | "CLOUD_ONLY"
    | "FAILED"
    | string;
  uploadProgress?: number;
  errorMessage?: string;
  tgChannelId?: number;
  tgMessageId?: number;
  tgFileId?: string;
  tgAccessHash?: number;
  importedFromGooglePhotos: boolean;
  googlePhotosMediaId?: string;
  googleCleanupStatus?: string;
  thumbnailPath?: string;
  previewPath?: string;
  blurHash?: string;
  isFavorite: boolean;
  isArchived: boolean;
  isTrashed: boolean;
  trashedTimestamp?: number;
  isEncrypted: boolean;
  albumIds: string[];
  deviceFolder?: string;
}

export interface Album {
  id: string;
  name: string;
  createdAt: number;
  coverMediaId?: string;
  isPinned: boolean;
  sourceType: string;
  itemCount: number;
}

export interface AppSettings {
  autoBackupEnabled: boolean;
  backupOverWifiOnly: boolean;
  backupWhileChargingOnly: boolean;
  uploadOriginalQuality: boolean;
  folderBackupSettings: Record<string, boolean>;
  clientEncryptionEnabled: boolean;
  vaultPassphraseSet: boolean;
  gridColumnCount: number;
  theme: "system" | "light" | "dark";
  telegramApiId?: string;
  telegramApiHash?: string;
  googleClientId?: string;
  googleClientSecret?: string;
}

export interface BackupProgressEvent {
  itemId: string;
  fileName: string;
  percent: number;
  status: string;
}

export interface ReclaimableSpace {
  count: number;
  totalSizeBytes: number;
}

export interface FreeUpSpaceResult {
  freedCount: number;
  freedBytes: number;
}

export interface GoogleDiscoveryInfo {
  totalCount: number;
  totalSizeBytes: number;
  albums: string[];
}

export interface VaultStatus {
  enabled: boolean;
  passphraseSet: boolean;
  unlocked: boolean;
}

export interface GoogleImportResult {
  choice: string;
  deletedCount: number;
  freedBytes: number;
  note?: string;
}
