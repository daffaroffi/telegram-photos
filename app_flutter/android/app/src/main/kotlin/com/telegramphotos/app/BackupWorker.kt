package com.telegramphotos.app

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.os.Build
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.ExistingWorkPolicy
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.util.concurrent.TimeUnit

/**
 * Background worker that scans MediaStore and uploads new photos to the
 * Telegram vault channel. Runs periodically (every 15 min) or on-demand.
 *
 * Flow:
 * 1. Scan MediaStore for new/changed items
 * 2. Filter items NOT_BACKED_UP
 * 3. Upload each via MTProto (handled by Rust core through Flutter isolate)
 * 4. Update sync status in DB
 * 5. Show progress/completion notification
 */
class BackupWorker(
    appContext: Context,
    params: WorkerParameters
) : CoroutineWorker(appContext, params) {

    companion object {
        private const val TAG = "BackupWorker"
        private const val CHANNEL_ID = "telegram_photos_backup"
        private const val NOTIFICATION_ID_PROGRESS = 1001
        private const val NOTIFICATION_ID_COMPLETE = 1002
        private const val NOTIFICATION_ID_FAILED = 1003
        private const val UNIQUE_WORK_NAME = "telegram_photos_backup"

        fun enqueuePeriodic(context: Context) {
            val request = PeriodicWorkRequestBuilder<BackupWorker>(
                15, TimeUnit.MINUTES
            ).build()
            WorkManager.getInstance(context).enqueueUniquePeriodicWork(
                UNIQUE_WORK_NAME,
                ExistingPeriodicWorkPolicy.KEEP,
                request
            )
            Log.d(TAG, "Periodic backup enqueued (every 15 min)")
        }

        fun enqueueOneTime(context: Context) {
            val request = OneTimeWorkRequestBuilder<BackupWorker>().build()
            WorkManager.getInstance(context).enqueueUniqueWork(
                "${UNIQUE_WORK_NAME}_manual",
                ExistingWorkPolicy.KEEP,
                request
            )
            Log.d(TAG, "One-time backup enqueued")
        }

        fun cancelAll(context: Context) {
            WorkManager.getInstance(context).cancelUniqueWork(UNIQUE_WORK_NAME)
            WorkManager.getInstance(context).cancelUniqueWork("${UNIQUE_WORK_NAME}_manual")
            Log.d(TAG, "All backup work cancelled")
        }

        fun createNotificationChannel(context: Context) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                "Photo Backup",
                NotificationManager.IMPORTANCE_LOW
            ).apply {
                description = "Shows progress while backing up photos to Telegram"
                setShowBadge(false)
            }
            val manager = context.getSystemService(NotificationManager::class.java)
            manager.createNotificationChannel(channel)
        }
    }

    override suspend fun doWork(): Result {
        Log.d(TAG, "BackupWorker started")
        createNotificationChannel(applicationContext)

        return withContext(Dispatchers.IO) {
            try {
                // Step 1: Read pending items from a shared file written by Flutter
                val pendingFile = java.io.File(applicationContext.filesDir, "backup_pending.json")
                if (!pendingFile.exists()) {
                    Log.d(TAG, "No pending backup items")
                    return@withContext Result.success()
                }

                val pendingJson = pendingFile.readText()
                val items = parsePendingItems(pendingJson)
                if (items.isEmpty()) {
                    Log.d(TAG, "Pending list is empty")
                    pendingFile.delete()
                    return@withContext Result.success()
                }

                // Step 2: Show progress notification
                showProgressNotification(0, items.size)

                // Step 3: Write upload commands for Flutter to pick up
                val commandFile = java.io.File(applicationContext.filesDir, "backup_commands.json")
                commandFile.writeText(pendingJson)

                // Step 4: Signal Flutter via a broadcast or shared state
                // The actual upload happens in the Flutter isolate which has access
                // to the Rust MTProto engine. We just queue the work here.
                Log.d(TAG, "Queued ${items.size} items for upload")

                // Step 5: Wait for completion (poll with timeout)
                val resultFile = java.io.File(applicationContext.filesDir, "backup_result.json")
                val deadline = System.currentTimeMillis() + 5 * 60 * 1000L // 5 min timeout
                while (!resultFile.exists() && System.currentTimeMillis() < deadline) {
                    Thread.sleep(2000)
                }

                if (resultFile.exists()) {
                    val resultJson = resultFile.readText()
                    resultFile.delete()
                    pendingFile.delete()
                    val jsonObj = org.json.JSONObject(resultJson)
                    val success = jsonObj.optInt("successCount", 0)
                    val failed = jsonObj.optInt("failCount", 0)
                    showCompletionNotification(success, failed)
                    Log.d(TAG, "Backup complete: $success ok, $failed failed")
                    Result.success()
                } else {
                    Log.w(TAG, "Backup timed out")
                    showFailedNotification("Backup timed out after 5 minutes")
                    Result.retry()
                }
            } catch (e: Exception) {
                Log.e(TAG, "BackupWorker failed", e)
                showFailedNotification(e.message ?: "Unknown error")
                Result.failure()
            }
        }
    }

    private fun parsePendingItems(json: String): List<UploadCommand> {
        return try {
            val array = org.json.JSONArray(json)
            (0 until array.length()).map { i ->
                val obj = array.getJSONObject(i)
                UploadCommand(
                    id = obj.optString("id", ""),
                    contentUri = obj.optString("contentUri", ""),
                    fileName = obj.optString("fileName", ""),
                    mimeType = obj.optString("mimeType", "image/jpeg"),
                    isVideo = obj.optBoolean("isVideo", false)
                )
            }
        } catch (e: Exception) {
            emptyList()
        }
    }

    private fun showProgressNotification(current: Int, total: Int) {
        val manager = applicationContext.getSystemService(NotificationManager::class.java)
        val notification = NotificationCompat.Builder(applicationContext, CHANNEL_ID)
            .setSmallIcon(android.R.drawable.ic_menu_upload)
            .setContentTitle("Backing up photos")
            .setContentText("$current / $total uploaded")
            .setProgress(total, current, false)
            .setOngoing(true)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .build()
        manager.notify(NOTIFICATION_ID_PROGRESS, notification)
    }

    private fun showCompletionNotification(success: Int, failed: Int) {
        val manager = applicationContext.getSystemService(NotificationManager::class.java)
        manager.cancel(NOTIFICATION_ID_PROGRESS)

        val text = if (failed > 0) {
            "$success uploaded, $failed failed"
        } else {
            "$success photos backed up to Telegram"
        }

        val notification = NotificationCompat.Builder(applicationContext, CHANNEL_ID)
            .setSmallIcon(android.R.drawable.ic_menu_send)
            .setContentTitle("Backup complete")
            .setContentText(text)
            .setAutoCancel(true)
            .build()
        manager.notify(NOTIFICATION_ID_COMPLETE, notification)
    }

    private fun showFailedNotification(error: String) {
        val manager = applicationContext.getSystemService(NotificationManager::class.java)
        manager.cancel(NOTIFICATION_ID_PROGRESS)

        val notification = NotificationCompat.Builder(applicationContext, CHANNEL_ID)
            .setSmallIcon(android.R.drawable.ic_dialog_alert)
            .setContentTitle("Backup failed")
            .setContentText(error)
            .setAutoCancel(true)
            .build()
        manager.notify(NOTIFICATION_ID_FAILED, notification)
    }
}

data class UploadCommand(
    val id: String,
    val contentUri: String,
    val fileName: String,
    val mimeType: String,
    val isVideo: Boolean
)
