import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../platform/media_scan.dart';
import '../rust/api/db.dart' as core;

/// Gallery import screen (PRD Part 2 K4: Google Photos 1-klik).
///
/// Import photos from device gallery (MediaStore) or Google Photos.
/// This is a key differentiator from Telephoto (K4: "1-klik").
class ImportScreen extends StatefulWidget {
  const ImportScreen({super.key});

  @override
  State<ImportScreen> createState() => _ImportScreenState();
}

class _ImportScreenState extends State<ImportScreen> {
  bool _scanning = false;
  bool _done = false;
  int _foundCount = 0;

  Future<void> _importGallery() async {
    if (_scanning) return;
    setState(() => _scanning = true);

    try {
      final json = await MediaScan.scanGalleryJson();
      final added = core.importScanResults(json: json);

      // Generate thumbnails for new items
      final missing = core.listMediaWithoutThumb(limit: 500);
      if (missing.isNotEmpty) {
        final mapJson = await MediaScan.generateThumbnails(ids: missing);
        core.saveThumbnailPaths(json: mapJson);
      }

      if (mounted) {
        setState(() {
          _scanning = false;
          _done = true;
          _foundCount = added;
        });
      }
    } catch (e) {
      if (mounted) {
        setState(() => _scanning = false);
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Import failed: $e')),
        );
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;

    return Scaffold(
      appBar: AppBar(title: const Text('Import photos')),
      body: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            // Header
            Center(
              child: Icon(
                _done ? LucideIcons.circleCheck : LucideIcons.images,
                size: 72,
                color: _done ? Colors.green : cs.primary,
              ),
            ),
            const SizedBox(height: 24),

            Center(
              child: Text(
                _done ? 'Import complete!' : 'Import your photos',
                style: Theme.of(context).textTheme.headlineSmall?.copyWith(
                      fontWeight: FontWeight.w600,
                    ),
              ),
            ),
            const SizedBox(height: 8),
            Center(
              child: Text(
                _done
                    ? '$_foundCount photos and videos imported from your gallery.'
                    : 'Scan your device gallery to find all photos and videos. '
                        'They will be organized in your private Telegram vault.',
                style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                      color: cs.onSurfaceVariant,
                    ),
                textAlign: TextAlign.center,
              ),
            ),
            const Spacer(),

            // Import button
            if (!_done)
              SizedBox(
                width: double.infinity,
                child: FilledButton.icon(
                  onPressed: _scanning ? null : _importGallery,
                  icon: _scanning
                      ? const SizedBox(
                          width: 18,
                          height: 18,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        )
                      : const Icon(LucideIcons.scan),
                  label: Text(_scanning ? 'Scanning...' : 'Scan gallery'),
                  style: FilledButton.styleFrom(
                    minimumSize: const Size(0, 48),
                  ),
                ),
              ),

            if (_done)
              SizedBox(
                width: double.infinity,
                child: FilledButton(
                  onPressed: () => Navigator.pop(context),
                  style: FilledButton.styleFrom(
                    minimumSize: const Size(0, 48),
                  ),
                  child: const Text('Done'),
                ),
              ),

            const SizedBox(height: 16),

            // Info
            if (!_done)
              Container(
                padding: const EdgeInsets.all(12),
                decoration: BoxDecoration(
                  color: cs.surfaceContainerHighest,
                  borderRadius: BorderRadius.circular(8),
                ),
                child: Row(
                  children: [
                    Icon(LucideIcons.info, size: 18, color: cs.onSurfaceVariant),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        'This reads photo metadata only. Original files stay on your device.',
                        style: Theme.of(context).textTheme.bodySmall?.copyWith(
                              color: cs.onSurfaceVariant,
                            ),
                      ),
                    ),
                  ],
                ),
              ),

            const SizedBox(height: 16),
          ],
        ),
      ),
    );
  }
}
