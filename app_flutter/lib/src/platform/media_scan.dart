import 'package:flutter/services.dart';

/// Platform channel to the native MediaStore scanner (port of the Tauri
/// MediaPlugin). Returns a JSON array of gallery media entries.
class MediaScan {
  static const _channel = MethodChannel('com.telegramphotos.app/media');

  /// Scans MediaStore (images + videos) and returns the raw JSON string.
  /// Throws [PlatformException] when permission is missing or scan fails.
  static Future<String> scanGalleryJson({String folder = ''}) async {
    final result = await _channel.invokeMethod<String>(
      'scanMediaStore',
      {'folder': folder},
    );
    return result ?? '[]';
  }

  /// Generates small JPEG thumbnails for [ids] (MediaStore native thumbs,
  /// anti-OOM) and returns a JSON map { mediaId -> absolute path }.
  static Future<String> generateThumbnails({required List<String> ids}) async {
    if (ids.isEmpty) return '{}';
    final result = await _channel.invokeMethod<String>(
      'generateThumbnails',
      {'ids': ids},
    );
    return result ?? '{}';
  }
}
