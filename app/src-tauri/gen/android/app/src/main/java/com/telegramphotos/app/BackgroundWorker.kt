package com.telegramphotos.app

import android.content.Context
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters

/**
 * Periodic auto-backup worker (PRD section 4.4).
 *
 * Runs on a background thread via WorkManager and calls into the Rust backup
 * engine through JNI (`Java_com_telegramphotos_app_BackgroundWorker_runBackup`).
 * The Rust side opens the same database and Telegram session as the UI, so the
 * worker is a real background backup, not a simulated one.
 */
class BackgroundWorker(
    appContext: Context,
    workerParams: WorkerParameters,
) : CoroutineWorker(appContext, workerParams) {

    override suspend fun doWork(): Result {
        // Load the Rust library (no-op if the main activity already loaded it).
        loadRustLibrary()

        val dataDir = applicationContext.filesDir.absolutePath
        val code = try {
            runBackup(dataDir)
        } catch (e: Throwable) {
            android.util.Log.w("BackgroundWorker", "Backup worker failed", e)
            return Result.retry()
        }
        return if (code >= 0) Result.success() else Result.retry()
    }

    private fun loadRustLibrary() {
        if (libraryLoaded) return
        val candidates = listOf(
            System.getProperty("tauri.libName") ?: "",
            "telegram_photos_lib",
            "app",
            "tauri",
        ).filter { it.isNotBlank() }
        for (name in candidates) {
            try {
                System.loadLibrary(name)
                libraryLoaded = true
                return
            } catch (_: UnsatisfiedLinkError) {
                // try next candidate
            }
        }
    }

    private external fun runBackup(dataDir: String): Int

    companion object {
        @Volatile
        private var libraryLoaded = false
    }
}
