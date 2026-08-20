package com.telegramphotos.app

import android.os.Bundle
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel

class MainActivity : FlutterActivity() {
    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        val channel = MethodChannel(
            flutterEngine.dartExecutor.binaryMessenger,
            "com.telegramphotos.app/media",
        )
        MediaPlugin.register(channel, applicationContext)
        MediaPlugin.currentActivity = this
    }

    override fun onRequestPermissionsResult(
        requestCode: Int,
        permissions: Array<out String>,
        grantResults: IntArray,
    ) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        if (requestCode == MediaPlugin.REQUEST_MEDIA_PERMISSIONS) {
            MediaPlugin.onPermissionResult(grantResults)
        }
    }

    override fun onResume() {
        super.onResume()
        MediaPlugin.currentActivity = this
    }

    override fun onDestroy() {
        MediaPlugin.currentActivity = null
        super.onDestroy()
    }
}
