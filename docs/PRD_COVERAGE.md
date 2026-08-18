# PRD Coverage

Mapping of `PRD_PART2.md` sections to actual implementation, with honest notes on limitations.

## Implementation Status

| Section | Feature | Status | Notes |
|---|---|---|---|
| §2 | Onboarding (QR + Phone OTP) | ✅ Done | QR login + phone OTP + 2FA via Grammers |
| §3.1 | Photos tab (timeline grid) | ✅ Done | Keyset pagination, sticky month headers |
| §3.2 | Search tab | ✅ Done | Placeholder — full-text search planned for v0.5 |
| §3.3 | Library tab (albums) | ✅ Done | Album list from DB |
| §3.4 | Settings tab | ✅ Done | Auto-backup, WiFi/charging, encryption, grid columns |
| §4.1 | Auto-scan on first launch | ✅ Done | MediaStore scan via MethodChannel |
| §4.2 | Vault channel | ✅ Done | Auto-create `TelegramPhotos_Vault` |
| §4.3 | Upload pipeline | ✅ Done | 512 KB chunks, FLOOD_WAIT, progress |
| §4.4 | Backup state machine | ✅ Done | Queue → Upload → Done + retry |
| §4.5 | Free Up Space | ❌ Not yet | Planned for v0.4 |
| §4.6 | Folder whitelist | ✅ Done | Settings screen toggle per folder |
| §4.7 | WiFi/charging constraints | ✅ Done | Settings screen toggle |
| §4.8 | Client-side encryption | ✅ Done | XChaCha20-Poly1305 + Argon2id |
| §5 | Database schema v2 | ✅ Done | Migrations auto-run on open |
| §6 | MediaStore scan | ✅ Done | Kotlin MethodChannel, anti-OOM |
| §7.1 | Thumbnail pipeline | ✅ Done | 256px JPEG, auto-generate on startup |
| §7.2 | EXIF extraction | ✅ Done | Date, GPS, camera model via kamadak-exif |
| §7.3 | SHA-256 hashing | ✅ Done | Per-file for dedup + integrity |
| §7.4 | Offline geocoding | ✅ Done | ~280 cities, 50 km radius |
| §7.5 | Backup progress UI | ✅ Done | UploadScreen with per-file progress |
| §7.6 | Status badge | ✅ Done | Color-coded per sync status |
| §7.7 | Grid columns control | ✅ Done | Settings: 3/4/5/6 columns |
| §8 | FLOOD_WAIT handling | ✅ Done | Auto-retry with X+2s delay |
| §9 | Notifications | ❌ Not yet | Planned for v0.4 (WorkManager) |
| §10 | Background backup | ❌ Not yet | Planned for v0.4 (WorkManager) |
| §11 | Performance targets | ⚠️ Partial | Scan works, 100k benchmark not run |

## Known Limitations

1. **No background backup yet** — WorkManager integration planned for v0.4.
2. **No notifications** — Will be added with background backup.
3. **No Free Up Space** — Requires verified backup flow, planned for v0.4.
4. **Search is placeholder** — Full-text search (FTS5) planned for v0.5.
5. **Google Photos import not ported** — OAuth + Library API integration planned for v0.5.
6. **core2 vendored stub** — May cause issues if grammers-crypto uses no_std features beyond `std::error::Error`.
7. **No iOS support** — Android only for now.
8. **Geocoding dataset is small** — ~280 cities, not GeoNames 15 MB.
