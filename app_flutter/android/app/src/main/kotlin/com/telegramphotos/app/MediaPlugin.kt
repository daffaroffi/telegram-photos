package com.telegramphotos.app

import android.content.ContentResolver
import android.content.Context
import android.os.Build
import android.provider.MediaStore
import android.util.Log
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import org.json.JSONArray
import org.json.JSONObject

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
            else -> result.notImplemented()
        }
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
}
