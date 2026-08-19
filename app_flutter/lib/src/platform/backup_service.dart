import 'package:flutter/services.dart';

/// Bridges to the native WorkManager backup service.
class BackupService {
  static const _channel = MethodChannel('com.telegramphotos.app/media');

  /// Start background backup for the given items.
  /// Each item map should have: id, contentUri, fileName, mimeType, isVideo.
  static Future<void> startBackup(List<Map<String, dynamic>> items) async {
    await _channel.invokeMethod('startBackup', {'items': items});
  }

  /// Cancel all pending backup work.
  static Future<void> cancelBackup() async {
    await _channel.invokeMethod('cancelBackup');
  }
}
