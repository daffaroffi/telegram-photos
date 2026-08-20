package com.telegramphotos.app

import android.Manifest
import android.app.Activity
import android.content.ContentResolver
import android.content.ContentUris
import android.content.Context
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import android.os.Build
import android.provider.MediaStore
import android.util.Size
import android.util.Log
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.io.FileOutputStream

/**
 * Native MediaStore scanner exposed to Flutter via a MethodChannel
 * (port of the Tauri-era MediaPlugin, same query logic and fixes:
 * dynamic projection for API levels, is_pending=0 filter, no OOM).
 */
object MediaPlugin : MethodChannel.MethodCallHandler {
    private const val TAG = "MediaPlugin"
    private const val CHANNEL = "com.telegramphotos.app/media"

    private var appContext: Context? = null

    fun register(channel: MethodChannel, context: Context) {
        appContext = context.applicationContext
        channel.setMethodCallHandler(this)
    }

    override fun onMethodCall(call: MethodCall, result: MethodChannel.Result) {
        when (call.method) {
            "scanMediaStore" -> {
                val folder = call.argument<String>("folder") ?: ""
                result.success(scanMediaStore(folder))
            }
            "generateThumbnails" -> {
                val ids = call.argument<List<String>>("ids") ?: emptyList()
                result.success(generateThumbnails(ids))
            }
            "readFileBytes" -> {
                val uri = call.argument<String>("uri") ?: return result.error("NO_URI", "No URI", null)
                val outPath = readFileToTemp(uri)
                if (outPath != null) result.success(outPath)
                else result.error("READ_FAILED", "Failed to read file", null)
            }
            "startBackup" -> {
                @Suppress("UNCHECKED_CAST")
                val items = call.argument<List<Map<String, Any>>>("items") ?: emptyList()
                startBackup(items)
                result.success(true)
            }
            "cancelBackup" -> {
                BackupWorker.cancelAll(appContext ?: return result.error("NO_CONTEXT", "No context", null))
                result.success(true)
            }
            "requestMediaPermissions" -> {
                val activity = getActivity(result) ?: return
                requestMediaPermissions(activity, result)
            }
            "checkMediaPermissions" -> {
                val ctx = appContext ?: return result.error("NO_CONTEXT", "No context", null)
                result.success(hasMediaPermissions(ctx))
            }
            else -> result.notImplemented()
        }
    }

    private fun startBackup(items: List<Map<String, Any>>) {
        val ctx = appContext ?: return
        // Write pending items to file for BackupWorker to pick up
        val pendingFile = java.io.File(ctx.filesDir, "backup_pending.json")
        val jsonArray = org.json.JSONArray()
        for (item in items) {
            val obj = org.json.JSONObject()
            obj.put("id", item["id"] ?: "")
            obj.put("contentUri", item["contentUri"] ?: "")
            obj.put("fileName", item["fileName"] ?: "")
            obj.put("mimeType", item["mimeType"] ?: "image/jpeg")
            obj.put("isVideo", item["isVideo"] ?: false)
            jsonArray.put(obj)
        }
        pendingFile.writeText(jsonArray.toString())
        // Enqueue one-time work
        BackupWorker.enqueueOneTime(ctx)
    }

    /**
     * Reads file from content URI and copies to a temp file. Returns the temp file path.
     */
    private fun readFileToTemp(uriStr: String): String? {
        val ctx = appContext ?: return null
        return try {
            // ID format from scan: "content://media/external/images/media_1000000034"
            // Convert to valid URI: replace trailing _<digits> with /<digits>
            val fixedUri = if (uriStr.contains("_")) {
                val lastUnderscore = uriStr.lastIndexOf('_')
                val afterUnderscore = uriStr.substring(lastUnderscore + 1)
                if (afterUnderscore.all { it.isDigit() }) {
                    uriStr.substring(0, lastUnderscore) + "/" + afterUnderscore
                } else uriStr
            } else uriStr
            Log.d("MediaPlugin", "readFileToTemp: original=$uriStr fixed=$fixedUri")
            val uri = Uri.parse(fixedUri)
            val resolver = ctx.contentResolver
            val tempFile = File(ctx.cacheDir, "upload_${System.currentTimeMillis()}_tmp")
            resolver.openInputStream(uri)?.use { input ->
                FileOutputStream(tempFile).use { output ->
                    input.copyTo(output)
                }
            }
            tempFile.absolutePath
        } catch (e: Exception) {
            Log.e("MediaPlugin", "readFileToTemp failed for $uriStr", e)
            null
        }
    }

    /**
     * Generates small JPEG thumbnails for the given media ids and stores them
     * under filesDir/thumbs/. Returns a JSON map { mediaId -> absolute path }.
     * Uses MediaStore's built-in thumbnails (no full decode = anti-OOM, PRD
     * Part 2 §7.1 pipeline tier 2: ~256px).
     */
    private fun generateThumbnails(ids: List<String>): String {
        val ctx = appContext ?: return "{}"
        val out = JSONObject()
        val thumbsDir = File(ctx.filesDir, "thumbs").apply { mkdirs() }
        val resolver = ctx.contentResolver

        for (id in ids) {
            try {
                // Scan stores IDs as "${contentUri}_${numericId}".
                // Extract the numeric part and rebuild a proper content URI.
                val lastUnderscore = id.lastIndexOf('_')
                val numericId = if (lastUnderscore > 0) id.substring(lastUnderscore + 1).toLong() else 0L
                val isVideo = id.contains("/video/")
                val baseUri = if (isVideo) MediaStore.Video.Media.EXTERNAL_CONTENT_URI
                              else MediaStore.Images.Media.EXTERNAL_CONTENT_URI
                val contentUri = ContentUris.withAppendedId(baseUri, numericId)
                val thumb = if (Build.VERSION.SDK_INT >= 29) {
                    resolver.loadThumbnail(contentUri, Size(256, 256), null)
                } else {
                    // Pre-29 fallback: decode with inSampleSize (anti-OOM).
                    val bmp = resolver.openInputStream(contentUri)?.use { ins ->
                        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
                        BitmapFactory.decodeStream(ins, null, bounds)
                        var sample = 1
                        while (bounds.outWidth / sample > 512 || bounds.outHeight / sample > 512) sample *= 2
                        val opts = BitmapFactory.Options().apply { inSampleSize = sample }
                        resolver.openInputStream(contentUri)?.use { ins2 ->
                            BitmapFactory.decodeStream(ins2, null, opts)
                        }


                    }
                    bmp ?: continue
                }
                val file = File(thumbsDir, "${id.hashCode()}.jpg")
                FileOutputStream(file).use { fos ->
                    thumb.compress(Bitmap.CompressFormat.JPEG, 82, fos)
                }
                out.put(id, file.absolutePath)
                thumb.recycle()
            } catch (e: Exception) {
                Log.w(TAG, "thumb failed for $id: ${e.message}")
            }
        }
        return out.toString()
    }

    private fun scanMediaStore(folder: String): String {
        val ctx = appContext ?: return "[]"
        val resolver: ContentResolver = ctx.contentResolver
        val entries = JSONArray()

        val collections = listOf(
            MediaStore.Images.Media.EXTERNAL_CONTENT_URI to "image",
            MediaStore.Video.Media.EXTERNAL_CONTENT_URI to "video",
        )

        for ((uri, mediaType) in collections) {
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
            }.toTypedArray()

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
                    Log.d(TAG, "query $mediaType returned rows=${cursor.count} sel=$selection")
                    val idCol = cursor.getColumnIndexOrThrow(MediaStore.MediaColumns._ID)
                    val nameCol = cursor.getColumnIndexOrThrow(MediaStore.MediaColumns.DISPLAY_NAME)
                    val mimeCol = cursor.getColumnIndex(MediaStore.MediaColumns.MIME_TYPE)
                    val sizeCol = cursor.getColumnIndex(MediaStore.MediaColumns.SIZE)
                    val takenCol = cursor.getColumnIndex(MediaStore.MediaColumns.DATE_TAKEN)
                    val widthCol = cursor.getColumnIndex(MediaStore.MediaColumns.WIDTH)
                    val heightCol = cursor.getColumnIndex(MediaStore.MediaColumns.HEIGHT)
                    val durCol = cursor.getColumnIndex(MediaStore.MediaColumns.DURATION)
                    val latCol = cursor.getColumnIndex(MediaStore.Images.Media.LATITUDE)
                    val lonCol = cursor.getColumnIndex(MediaStore.Images.Media.LONGITUDE)
                    val bucketCol = cursor.getColumnIndex(MediaStore.MediaColumns.BUCKET_DISPLAY_NAME)
                    val modCol = cursor.getColumnIndex(MediaStore.MediaColumns.DATE_MODIFIED)

                    while (cursor.moveToNext()) {
                        val id = cursor.getLong(idCol)
                        val name = cursor.getString(nameCol) ?: "IMG_$id"
                        val mime = cursor.getString(mimeCol) ?: ""
                        val size = if (sizeCol >= 0) cursor.getLong(sizeCol) else 0L
                        val taken = if (takenCol >= 0) cursor.getLong(takenCol) else 0L
                        val width = if (widthCol >= 0) cursor.getInt(widthCol) else 0
                        val height = if (heightCol >= 0) cursor.getInt(heightCol) else 0
                        val duration = if (durCol >= 0) cursor.getLong(durCol) else 0L
                        val bucket = if (bucketCol >= 0) cursor.getString(bucketCol) ?: "" else ""
                        val modified = if (modCol >= 0) cursor.getLong(modCol) else 0L

                        val entry = JSONObject().apply {
                            put("id", "${uri}_$id")
                            put("localIdentifier", id.toString())
                            put("fileName", name)
                            put("mimeType", mime)
                            put("mediaType", mediaType)
                            put("fileSizeBytes", size)
                            put("dateTaken", taken * 1000L)
                            put("dateAdded", modified * 1000L)
                            put("width", if (width > 0) width else JSONObject.NULL)
                            put("height", if (height > 0) height else JSONObject.NULL)
                            put("durationMs", if (duration > 0) duration else JSONObject.NULL)
                            put("deviceFolder", bucket)
                            if (latCol >= 0 && lonCol >= 0) {
                                val lat = cursor.getDouble(latCol)
                                val lon = cursor.getDouble(lonCol)
                                if (lat != 0.0 || lon != 0.0) {
                                    put("latitude", lat)
                                    put("longitude", lon)
                                }
                            }
                            // Real file path resolved below when needed; the
                            // content:// URI is enough for thumbnails.
                            put("filePath", JSONObject.NULL)
                            // Required MediaItem fields with scan-time defaults.
                            put("sha256Hash", "")
                            put("syncStatus", "NOT_BACKED_UP")
                            put("importedFromGooglePhotos", false)
                            put("isFavorite", false)
                            put("isArchived", false)
                            put("isTrashed", false)
                            put("isEncrypted", false)
                            put("albumIds", JSONArray())
                        }
                        entries.put(entry)
                    }
                }
            } catch (e: Exception) {
                Log.w(TAG, "scan query failed for $mediaType: ${e.message}")
            }
        }
        return entries.toString()
    }

    /**
     * Returns the current Activity reference from the Flutter embedding,
     * or calls result.error if unavailable.
     */
    private fun getActivity(result: MethodChannel.Result): Activity? {
        // The appContext is the applicationContext, not the Activity.
        // We need to reach back through the FlutterActivity to get it.
        // Use a workaround: the channel is registered by MainActivity,
        // which passes applicationContext. We store a weak ref to the Activity.
        return currentActivity ?: run {
            result.error("NO_ACTIVITY", "No current Activity", null)
            null
        }
    }

    /** Weak reference to the hosting Activity, set by MainActivity. */
    var currentActivity: Activity? = null
        internal set

    /** Check if media permissions are already granted. */
    fun hasMediaPermissions(context: Context): Boolean {
        return if (Build.VERSION.SDK_INT >= 33) {
            ContextCompat.checkSelfPermission(context, Manifest.permission.READ_MEDIA_IMAGES) ==
                    PackageManager.PERMISSION_GRANTED &&
            ContextCompat.checkSelfPermission(context, Manifest.permission.READ_MEDIA_VIDEO) ==
                    PackageManager.PERMISSION_GRANTED
        } else {
            ContextCompat.checkSelfPermission(context, Manifest.permission.READ_EXTERNAL_STORAGE) ==
                    PackageManager.PERMISSION_GRANTED
        }
    }

    /** Request media permissions from the user. Blocks until the user responds. */
    private fun requestMediaPermissions(activity: Activity, result: MethodChannel.Result) {
        if (hasMediaPermissions(activity)) {
            result.success(true)
            return
        }

        val permissions = if (Build.VERSION.SDK_INT >= 33) {
            arrayOf(
                Manifest.permission.READ_MEDIA_IMAGES,
                Manifest.permission.READ_MEDIA_VIDEO,
            )
        } else {
            arrayOf(Manifest.permission.READ_EXTERNAL_STORAGE)
        }

        pendingPermissionResult = result
        ActivityCompat.requestPermissions(activity, permissions, REQUEST_MEDIA_PERMISSIONS)
    }

    /** Called by MainActivity when the permission result arrives. */
    fun onPermissionResult(grantResults: IntArray) {
        val pending = pendingPermissionResult ?: return
        pendingPermissionResult = null
        val granted = grantResults.isNotEmpty() &&
                grantResults.all { it == PackageManager.PERMISSION_GRANTED }
        pending.success(granted)
    }

    /** Pending result for the permission request. */
    private var pendingPermissionResult: MethodChannel.Result? = null

    const val REQUEST_MEDIA_PERMISSIONS = 1001
}
