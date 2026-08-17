package com.telegramphotos.app

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.enableEdgeToEdge
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)

    // Give the JNI bridge a Context and create the notification channel.
    MediaPlugin.initialize(this)

    // Real-time gallery sync (PRD 4.3) + periodic auto-backup (PRD 4.4).
    MediaPlugin.registerContentObserver("")
    BackupScheduler.schedule(this)

    requestNeededPermissions()
  }

  private fun requestNeededPermissions() {
    val needed = mutableListOf<String>()

    if (Build.VERSION.SDK_INT >= 33) {
      needed.add(Manifest.permission.READ_MEDIA_IMAGES)
      needed.add(Manifest.permission.READ_MEDIA_VIDEO)
      needed.add(Manifest.permission.POST_NOTIFICATIONS)
    } else {
      needed.add(Manifest.permission.READ_EXTERNAL_STORAGE)
    }

    val missing = needed.filter {
      ContextCompat.checkSelfPermission(this, it) != PackageManager.PERMISSION_GRANTED
    }
    if (missing.isNotEmpty()) {
      ActivityCompat.requestPermissions(this, missing.toTypedArray(), 0x01)
    }
  }
}
