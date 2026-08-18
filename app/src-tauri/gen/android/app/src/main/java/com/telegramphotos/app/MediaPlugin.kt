package com.telegramphotos.app

import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.ContentResolver
import android.content.ContentUris
import android.content.Context
import android.content.pm.PackageManager
import android.database.ContentObserver
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.net.Uri
import android.os.BatteryManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.provider.MediaStore
import android.util.Log
import java.io.File
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import org.json.JSONArray
import org.json.JSONObject

/**
 * Native bridge for the Rust backend (PRD sections 4.3, 4.4).
 *
 * Exposes static methods called over JNI:
 *  - scanMediaStore(folder)      -> JSON array of gallery media
 *  - checkConstraints(wifi, chg) -> real network/charging state
 *  - registerContentObserver()   -> real-time new-media notifications
 *  - reportProgress(...)         -> update backup progress notification
 *  - notifyBackupDone(count)     -> completion summary notification
 */
object MediaPlugin {
    private const val TAG = "MediaPlugin"
    private const val CHANNEL_ID = "backup_progress"
    private const val NOTIF_ID = 1001

    @Volatile
    private var appContext: Context? = null

    private var observer: ContentObserver? = null
    private var lastMediaCount = 0L

    /** Called from MainActivity.onCreate so static methods have a Context. */
    @JvmStatic
    fun initialize(context: Context) {
        appContext = context.applicationContext
        ensureNotificationChannel()
    }

    // ─────────────────────────────────────────────────────────────────────────
    // MediaStore scan (PRD 4.3)
    // ─────────────────────────────────────────────────────────────────────────

    @JvmStatic
    fun scanMediaStore(folder: String): String {
        val ctx = appContext ?: return "[]"
        val resolver: ContentResolver = ctx.contentResolver
        val entries = JSONArray()

        val collections = listOf(
            MediaStore.Images.Media.EXTERNAL_CONTENT_URI to "image",
            MediaStore.Video.Media.EXTERNAL_CONTENT_URI to "video",
        )

        for ((uri, mediaType) in collections) {
            // Build the projection dynamically: some columns only exist on
            // newer API levels, and querying a missing column throws
            // SQLiteException (which made the whole scan fail on API < 29).
            val projection = buildList {
                add(MediaStore.MediaColumns._ID)
                add(MediaStore.MediaColumns.DISPLAY_NAME)
                add(MediaStore.MediaColumns.MIME_TYPE)
                add(MediaStore.MediaColumns.SIZE)
                add(MediaStore.MediaColumns.DATE_TAKEN)
                add(MediaStore.MediaColumns.WIDTH)
                add(MediaStore.MediaColumns.HEIGHT)
                add(MediaStore.MediaColumns.DURATION)
                add(MediaStore.MediaColumns.BUCKET_DISPLAY_NAME)
                add(MediaStore.MediaColumns.DATE_MODIFIED)
                if (Build.VERSION.SDK_INT >= 29) {
                    add(MediaStore.MediaColumns.RELATIVE_PATH)
                    add(MediaStore.Images.Media.LATITUDE)
                    add(MediaStore.Images.Media.LONGITUDE)
                }
                if (Build.VERSION.SDK_INT >= 33) {
                    add(MediaStore.MediaColumns.IS_FAVORITE)
                }
            }.toTypedArray()
            // Filter: folder (opsional) + skip files that are still being
            // written (is_pending=1 — invisible to apps and half-written).
            val selectionParts = mutableListOf<String>()
            val selectionArgs = mutableListOf<String>()
            if (folder.isNotBlank()) {
                selectionParts.add("${MediaStore.MediaColumns.BUCKET_DISPLAY_NAME} = ?")
                selectionArgs.add(folder)
            }
            if (Build.VERSION.SDK_INT >= 29) {
                selectionParts.add("${MediaStore.MediaColumns.IS_PENDING} = 0")
            }
            val selection = if (selectionParts.isNotEmpty()) selectionParts.joinToString(" AND ") else null
            val selectionArgsArr = if (selectionArgs.isNotEmpty()) selectionArgs.toTypedArray() else null

            try {
                resolver.query(uri, projection, selection, selectionArgsArr, null)?.use { cursor ->
                    val idCol = cursor.getColumnIndexOrThrow(MediaStore.MediaColumns._ID)
                    val nameCol = cursor.getColumnIndexOrThrow(MediaStore.MediaColumns.DISPLAY_NAME)
                    val mimeCol = cursor.getColumnIndexOrThrow(MediaStore.MediaColumns.MIME_TYPE)
                    val sizeCol = cursor.getColumnIndexOrThrow(MediaStore.MediaColumns.SIZE)
                    val takenCol = cursor.getColumnIndexOrThrow(MediaStore.MediaColumns.DATE_TAKEN)
                    val widthCol = cursor.getColumnIndex(MediaStore.MediaColumns.WIDTH)
                    val heightCol = cursor.getColumnIndex(MediaStore.MediaColumns.HEIGHT)
                    val durCol = cursor.getColumnIndex(MediaStore.MediaColumns.DURATION)
                    val latCol = cursor.getColumnIndex(MediaStore.Images.Media.LATITUDE)
                    val lonCol = cursor.getColumnIndex(MediaStore.Images.Media.LONGITUDE)
                    val bucketCol = cursor.getColumnIndexOrThrow(MediaStore.MediaColumns.BUCKET_DISPLAY_NAME)
                    val modCol = cursor.getColumnIndex(MediaStore.MediaColumns.DATE_MODIFIED)
                    val relCol = cursor.getColumnIndex(MediaStore.MediaColumns.RELATIVE_PATH)
                    val favCol = cursor.getColumnIndex(MediaStore.MediaColumns.IS_FAVORITE)

                    while (cursor.moveToNext()) {
                        val id = cursor.getLong(idCol)
                        val itemUri = ContentUris.withAppendedId(uri, id)
                        val obj = JSONObject()
                        obj.put("id", "$mediaType:$id")
                        obj.put("uri", itemUri.toString())
                        obj.put("path", if (relCol >= 0) cursor.getString(relCol) else JSONObject.NULL)
                        obj.put("fileName", cursor.getString(nameCol) ?: "")
                        obj.put("mimeType", cursor.getString(mimeCol) ?: "application/octet-stream")
                        obj.put("mediaType", mediaType)
                        obj.put("sizeBytes", cursor.getLong(sizeCol))
                        obj.put("dateTaken", if (cursor.isNull(takenCol)) cursor.getLong(modCol) * 1000 else cursor.getLong(takenCol))
                        obj.put("width", if (widthCol >= 0 && !cursor.isNull(widthCol)) cursor.getInt(widthCol) else JSONObject.NULL)
                        obj.put("height", if (heightCol >= 0 && !cursor.isNull(heightCol)) cursor.getInt(heightCol) else JSONObject.NULL)
                        obj.put("durationMs", if (durCol >= 0 && !cursor.isNull(durCol)) cursor.getLong(durCol) else JSONObject.NULL)
                        obj.put("latitude", if (latCol >= 0 && !cursor.isNull(latCol)) cursor.getDouble(latCol) else JSONObject.NULL)
                        obj.put("longitude", if (lonCol >= 0 && !cursor.isNull(lonCol)) cursor.getDouble(lonCol) else JSONObject.NULL)
                        obj.put("deviceFolder", cursor.getString(bucketCol) ?: "")
                        obj.put("isFavorite", favCol >= 0 && cursor.getInt(favCol) == 1)
                        // Native small thumbnail (avoids decoding full photos in
                        // Rust, which caused OOM crashes when scanning large
                        // galleries).
                        obj.put("thumbnailPath", makeThumbnail(ctx, itemUri, id, mediaType) ?: JSONObject.NULL)
                        entries.put(obj)
                    }
                }
            } catch (e: Exception) {
                Log.w(TAG, "scanMediaStore query failed: ${e.message}")
            }
        }
        return entries.toString()
    }

    /**
     * Creates a small (~256px) JPEG thumbnail in the cache dir using Android's
     * native decoder. Returns the absolute path, or null on failure.
     */
    private fun makeThumbnail(ctx: Context, uri: Uri, id: Long, mediaType: String): String? {
        return try {
            val dir = File(ctx.cacheDir, "scanthumbs")
            dir.mkdirs()
            val out = File(dir, "${mediaType}_${id}.jpg")
            if (out.exists() && out.length() > 0) return out.absolutePath
            val bmp = if (Build.VERSION.SDK_INT >= 29) {
                ctx.contentResolver.loadThumbnail(uri, android.util.Size(256, 256), null)
            } else if (mediaType == "image") {
                MediaStore.Images.Thumbnails.getThumbnail(
                    ctx.contentResolver, id, MediaStore.Images.Thumbnails.MINI_KIND, null
                )
            } else {
                MediaStore.Video.Thumbnails.getThumbnail(
                    ctx.contentResolver, id, MediaStore.Video.Thumbnails.MINI_KIND, null
                )
            }
            if (bmp == null) return null
            out.outputStream().use { fos ->
                bmp.compress(android.graphics.Bitmap.CompressFormat.JPEG, 80, fos)
            }
            out.absolutePath
        } catch (e: Exception) {
            Log.w(TAG, "makeThumbnail failed: ${e.message}")
            null
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // MediaStore materialization (content:// URI -> real file)
    // ─────────────────────────────────────────────────────────────────────────

    /**
     * Copies a MediaStore `content://` URI into app-private storage so the
     * Rust backend (which has no ContentResolver) can hash, thumbnail and
     * upload it. Returns the absolute file path, or an empty string on failure.
     */
    @JvmStatic
    fun materializeMedia(uri: String, destDir: String, fileName: String): String {
        val ctx = appContext ?: return ""
        return try {
            val src = Uri.parse(uri)
            val dir = File(destDir)
            dir.mkdirs()
            val out = File(dir, "${System.currentTimeMillis()}_${sanitize(fileName)}")
            ctx.contentResolver.openInputStream(src)?.use { input ->
                out.outputStream().use { output -> input.copyTo(output) }
            } ?: return ""
            if (out.length() == 0L) {
                out.delete()
                return ""
            }
            out.absolutePath
        } catch (e: Exception) {
            Log.w(TAG, "materializeMedia failed: ${e.message}")
            ""
        }
    }

    private fun sanitize(name: String): String =
        name.replace(Regex("[^A-Za-z0-9._-]"), "_")

    // ─────────────────────────────────────────────────────────────────────────
    // Backup constraints (PRD 4.4: Wi-Fi only / charging only)
    // ─────────────────────────────────────────────────────────────────────────

    @JvmStatic
    fun checkConstraints(wifiOnly: Boolean, chargingOnly: Boolean): Boolean {
        val ctx = appContext ?: return true
        if (wifiOnly) {
            val cm = ctx.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
            val caps = cm.getNetworkCapabilities(cm.activeNetwork) ?: return false
            if (!caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) &&
                !caps.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET)
            ) {
                return false
            }
        }
        if (chargingOnly) {
            val bm = ctx.getSystemService(Context.BATTERY_SERVICE) as BatteryManager
            if (bm.getIntProperty(BatteryManager.BATTERY_PROPERTY_STATUS) !=
                BatteryManager.BATTERY_STATUS_CHARGING
            ) {
                return false
            }
        }
        return true
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Real-time new-media detection (PRD 4.3)
    // ─────────────────────────────────────────────────────────────────────────

    @JvmStatic
    fun registerContentObserver(arg: String): String {
        val ctx = appContext ?: return ""
        if (observer != null) return ""
        val resolver = ctx.contentResolver
        lastMediaCount = countMedia(resolver)
        observer = object : ContentObserver(Handler(Looper.getMainLooper())) {
            override fun onChange(selfChange: Boolean) {
                val now = countMedia(resolver)
                if (now > lastMediaCount) {
                    lastMediaCount = now
                    // Signal Rust: emit a lightweight scan request. The backup
                    // engine picks up new items on its next cycle.
                    Log.i(TAG, "New media detected ($now items)")
                } else {
                    lastMediaCount = now
                }
            }
        }
        resolver.registerContentObserver(
            MediaStore.Images.Media.EXTERNAL_CONTENT_URI,
            true,
            observer!!
        )
        resolver.registerContentObserver(
            MediaStore.Video.Media.EXTERNAL_CONTENT_URI,
            true,
            observer!!
        )
        return ""
    }

    private fun countMedia(resolver: ContentResolver): Long {
        var count = 0L
        for (uri in listOf(
            MediaStore.Images.Media.EXTERNAL_CONTENT_URI,
            MediaStore.Video.Media.EXTERNAL_CONTENT_URI
        )) {
            try {
                resolver.query(uri, arrayOf("count(*) AS c"), null, null, null)?.use { c ->
                    if (c.moveToFirst()) count += c.getLong(0)
                }
            } catch (_: Exception) {
            }
        }
        return count
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Notifications (PRD 4.4: foreground feedback during backup)
    // ─────────────────────────────────────────────────────────────────────────

    private fun ensureNotificationChannel() {
        val ctx = appContext ?: return
        val nm = ctx.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        val channel = NotificationChannel(
            CHANNEL_ID,
            "Backup Telegram",
            NotificationManager.IMPORTANCE_LOW
        ).apply {
            description = "Progres pencadangan foto ke Telegram"
            setShowBadge(false)
        }
        nm.createNotificationChannel(channel)
    }

    @JvmStatic
    fun reportProgress(fileName: String, percent: Int, status: String) {
        val ctx = appContext ?: return
        val nm = ctx.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        val title = when (status) {
            "UPLOADING" -> "Mencadangkan ke Telegram"
            "BACKED_UP" -> "Tercadangkan"
            "FAILED" -> "Gagal mencadangkan"
            "VAULT_LOCKED" -> "Vault terkunci"
            else -> "Pencadangan"
        }
        val text = if (percent > 0) "$fileName — $percent%" else fileName
        val notif = NotificationCompat.Builder(ctx, CHANNEL_ID)
            .setSmallIcon(android.R.drawable.stat_sys_upload)
            .setContentTitle(title)
            .setContentText(text)
            .setOnlyAlertOnce(true)
            .setProgress(100, percent.coerceIn(0, 100), percent <= 0)
            .build()
        nm.notify(NOTIF_ID, notif)
    }

    @JvmStatic
    fun notifyBackupDone(count: Int) {
        val ctx = appContext ?: return
        val nm = ctx.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        val notif = NotificationCompat.Builder(ctx, CHANNEL_ID)
            .setSmallIcon(android.R.drawable.stat_sys_upload_done)
            .setContentTitle("Pencadangan selesai")
            .setContentText("$count item baru tersimpan di Telegram")
            .setAutoCancel(true)
            .build()
        nm.notify(NOTIF_ID, notif)
    }
}
