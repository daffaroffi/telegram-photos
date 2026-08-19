import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../../src/rust/api/mirror.dart';

/// Backup banner (PRD Part 2 S3.1, pattern G4 from Google Photos):
/// shown only while there is active progress; tap opens the Progress Hub.
class BackupBanner extends StatelessWidget {
  const BackupBanner({super.key, required this.summary, this.onTap});

  final UploadsSummary summary;
  final VoidCallback? onTap;

  bool get _hasProgress =>
      summary.queuedCount > 0 || summary.uploadingCount > 0;

  @override
  Widget build(BuildContext context) {
    if (!_hasProgress) return const SizedBox.shrink();

    final active = summary.uploadingCount;
    final queued = summary.queuedCount;
    final total = active + queued;
    final totalBytes = summary.queuedBytes + summary.uploadingBytes;
    final title = active > 0
        ? 'Uploading $active photo${active == 1 ? '' : 's'}...'
        : '$queued photo${queued == 1 ? '' : 's'} waiting...';
    final subtitle =
        '$total file${total == 1 ? '' : 's'} . ${_formatBytes(totalBytes)}';

    return Material(
      color: Theme.of(context).colorScheme.primaryContainer,
      child: InkWell(
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
          child: Row(
            children: [
              SizedBox(
                width: 20,
                height: 20,
                child: CircularProgressIndicator(
                  strokeWidth: 2.5,
                  color: Theme.of(context).colorScheme.onPrimaryContainer,
                ),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      title,
                      style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                            color: Theme.of(context)
                                .colorScheme
                                .onPrimaryContainer,
                          ),
                    ),
                    Text(
                      subtitle,
                      style: Theme.of(context).textTheme.bodySmall?.copyWith(
                            color: Theme.of(context)
                                .colorScheme
                                .onPrimaryContainer
                                .withValues(alpha: 0.7),
                          ),
                    ),
                  ],
                ),
              ),
              Icon(
                LucideIcons.chevronRight,
                color: Theme.of(context).colorScheme.onPrimaryContainer,
              ),
            ],
          ),
        ),
      ),
    );
  }

  String _formatBytes(int bytes) {
    if (bytes < 1024) return '$bytes B';
    if (bytes < 1024 * 1024) return '${(bytes / 1024).toStringAsFixed(0)} KB';
    if (bytes < 1024 * 1024 * 1024) {
      return '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB';
    }
    return '${(bytes / (1024 * 1024 * 1024)).toStringAsFixed(2)} GB';
  }
}
