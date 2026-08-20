import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../rust/api/db.dart' as core;
import '../rust/api/mirror.dart';
import '../rust/api/telegram.dart' as tg;
import '../../main.dart' show telegramHandle;
import '../platform/media_scan.dart';
import '../platform/backup_service.dart';

/// Upload / Progress Hub screen (PRD Part 2 S4.4, S6.1, S9.2).
///
/// Central task management: shows all running/paused/completed/failed tasks
/// with progress bars, pause/resume/retry/cancel actions.
class UploadScreen extends StatefulWidget {
  const UploadScreen({super.key});

  @override
  State<UploadScreen> createState() => _UploadScreenState();
}

class _UploadScreenState extends State<UploadScreen> {
  List<MediaItem> _pendingItems = [];
  List<MediaItem> _uploadedItems = [];
  List<MediaItem> _failedItems = [];
  bool _loading = true;
  bool _uploading = false;
  int _currentUploadIndex = 0;
  int _totalUploads = 0;
  int _successCount = 0;
  int _failedCount = 0;

  @override
  void initState() {
    super.initState();
    _loadItems();
  }

  Future<void> _loadItems() async {
    setState(() => _loading = true);

    final pending = core.listPendingBackup(limit: 1000);
    final all = core.listTimeline(beforeTimestamp: null, limit: 10000);

    final uploaded = all.where((m) => m.syncStatus == 'BACKED_UP').toList();
    final failed = <MediaItem>[]; // TODO: get from upload_errors table

    setState(() {
      _pendingItems = pending;
      _uploadedItems = uploaded;
      _failedItems = failed;
      _loading = false;
    });
  }

  Future<void> _startBulkUpload() async {
    if (_uploading || _pendingItems.isEmpty) return;

    setState(() {
      _uploading = true;
      _currentUploadIndex = 0;
      _totalUploads = _pendingItems.length;
      _successCount = 0;
      _failedCount = 0;
    });

    for (int i = 0; i < _pendingItems.length; i++) {
      if (!_uploading) break; // Allow cancellation

      final item = _pendingItems[i];
      setState(() => _currentUploadIndex = i + 1);

      try {
        final tempPath = await MediaScan.readFileToTemp(item.id);
        final ext = tempPath.split('.').last.toLowerCase();
        final isVideo = ['mp4', 'mov', 'avi', 'mkv', 'webm'].contains(ext);
        final mimeType = isVideo ? 'video/mp4' : 'image/jpeg';

        await tg.uploadPhoto(
          handle: telegramHandle,
          filePath: tempPath,
          fileName: item.fileName,
          mimeType: mimeType,
          isVideo: isVideo,
        );

        core.setMediaStatus(id: item.id, status: 1);
        _successCount++;
      } catch (e) {
        _failedCount++;
        // TODO: Log to upload_errors table
      }

      // Refresh UI periodically
      if (i % 5 == 0) {
        setState(() {});
      }
    }

    setState(() => _uploading = false);
    await _loadItems();

    if (mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            'Upload complete: $_successCount succeeded, $_failedCount failed',
          ),
        ),
      );
    }
  }

  void _cancelUpload() {
    setState(() => _uploading = false);
    BackupService.cancelBackup();
  }

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;
    final summary = core.uploadsSummary();

    return Scaffold(
      appBar: AppBar(
        title: const Text('Tasks'),
        actions: [
          if (_uploading)
            IconButton(
              icon: const Icon(LucideIcons.x),
              tooltip: 'Cancel all',
              onPressed: _cancelUpload,
            ),
        ],
      ),
      body: _loading
          ? const Center(child: CircularProgressIndicator())
          : RefreshIndicator(
              onRefresh: _loadItems,
              child: ListView(
                padding: const EdgeInsets.only(bottom: 32),
                children: [
                  // -- Active upload progress --
                  if (_uploading) _buildUploadProgress(cs),

                  // -- Pending items --
                  if (_pendingItems.isNotEmpty && !_uploading)
                    _buildSection(
                      context,
                      icon: LucideIcons.clock,
                      title: '${_pendingItems.length} waiting',
                      color: cs.primary,
                      child: Column(
                        children: [
                          ..._pendingItems.take(20).map(
                                (item) => _PendingTile(item: item),
                              ),
                          if (_pendingItems.length > 20)
                            Padding(
                              padding: const EdgeInsets.all(12),
                              child: Text(
                                '+${_pendingItems.length - 20} more',
                                style: TextStyle(color: cs.onSurfaceVariant),
                              ),
                            ),
                        ],
                      ),
                    ),

                  // -- Start upload button --
                  if (_pendingItems.isNotEmpty && !_uploading)
                    Padding(
                      padding: const EdgeInsets.all(16),
                      child: SizedBox(
                        width: double.infinity,
                        child: FilledButton.icon(
                          onPressed: _startBulkUpload,
                          icon: const Icon(LucideIcons.upload),
                          label: Text('Back up ${_pendingItems.length} photos'),
                          style: FilledButton.styleFrom(
                            minimumSize: const Size(0, 48),
                          ),
                        ),
                      ),
                    ),

                  // -- Uploaded items --
                  if (_uploadedItems.isNotEmpty)
                    _buildSection(
                      context,
                      icon: LucideIcons.cloud,
                      title: '${_uploadedItems.length} backed up',
                      color: Colors.green,
                      child: Column(
                        children: _uploadedItems.take(10).map(
                              (item) => _UploadedTile(item: item),
                            ).toList(),
                      ),
                    ),

                  // -- Failed items --
                  if (_failedItems.isNotEmpty)
                    _buildSection(
                      context,
                      icon: LucideIcons.circleAlert,
                      title: '${_failedItems.length} failed',
                      color: cs.error,
                      child: Column(
                        children: _failedItems.take(10).map(
                              (item) => _FailedTile(
                                item: item,
                                onRetry: () {
                                  // TODO: Implement retry
                                },
                              ),
                            ).toList(),
                      ),
                    ),

                  // -- Empty state --
                  if (_pendingItems.isEmpty &&
                      _uploadedItems.isEmpty &&
                      _failedItems.isEmpty)
                    _buildEmptyState(cs),
                ],
              ),
            ),
    );
  }

  Widget _buildUploadProgress(ColorScheme cs) {
    final progress =
        _totalUploads > 0 ? _currentUploadIndex / _totalUploads : 0.0;

    return Container(
      margin: const EdgeInsets.all(16),
      padding: const EdgeInsets.all(20),
      decoration: BoxDecoration(
        color: cs.primaryContainer,
        borderRadius: BorderRadius.circular(16),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              SizedBox(
                width: 20,
                height: 20,
                child: CircularProgressIndicator(
                  strokeWidth: 2,
                  value: progress,
                ),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Text(
                  'Uploading $_currentUploadIndex of $_totalUploads',
                  style: Theme.of(context).textTheme.titleMedium?.copyWith(
                        color: cs.onPrimaryContainer,
                      ),
                ),
              ),
              Text(
                '${(progress * 100).toStringAsFixed(0)}%',
                style: Theme.of(context).textTheme.titleMedium?.copyWith(
                      color: cs.onPrimaryContainer,
                      fontWeight: FontWeight.w600,
                    ),
              ),
            ],
          ),
          const SizedBox(height: 12),
          LinearProgressIndicator(
            value: progress,
            backgroundColor: cs.onPrimaryContainer.withValues(alpha: 0.15),
            color: cs.onPrimaryContainer,
          ),
          const SizedBox(height: 12),
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Text(
                '$_successCount succeeded, $_failedCount failed',
                style: TextStyle(color: cs.onPrimaryContainer),
              ),
              TextButton(
                onPressed: _cancelUpload,
                style: TextButton.styleFrom(
                  foregroundColor: cs.onPrimaryContainer,
                ),
                child: const Text('Cancel'),
              ),
            ],
          ),
        ],
      ),
    );
  }

  Widget _buildSection(
    BuildContext context, {
    required IconData icon,
    required String title,
    required Color color,
    required Widget child,
  }) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
          child: Row(
            children: [
              Icon(icon, size: 18, color: color),
              const SizedBox(width: 8),
              Text(
                title,
                style: Theme.of(context).textTheme.titleSmall?.copyWith(
                      color: color,
                    ),
              ),
            ],
          ),
        ),
        child,
      ],
    );
  }

  Widget _buildEmptyState(ColorScheme cs) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(32),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(LucideIcons.cloudOff, size: 64, color: cs.outline),
            const SizedBox(height: 16),
            Text(
              'All caught up',
              style: Theme.of(context).textTheme.headlineSmall,
            ),
            const SizedBox(height: 8),
            Text(
              'No pending uploads. Your photos are safe.',
              style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                    color: cs.onSurfaceVariant,
                  ),
            ),
          ],
        ),
      ),
    );
  }
}

// -- Pending Tile --

class _PendingTile extends StatelessWidget {
  const _PendingTile({required this.item});

  final MediaItem item;

  @override
  Widget build(BuildContext context) {
    return ListTile(
      leading: Icon(
        item.mediaType == 'video' ? LucideIcons.video : LucideIcons.image,
        size: 20,
      ),
      title: Text(
        item.fileName,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
        style: const TextStyle(fontSize: 14),
      ),
      trailing: Container(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
        decoration: BoxDecoration(
          color: Theme.of(context).colorScheme.surfaceContainerHighest,
          borderRadius: BorderRadius.circular(8),
        ),
        child: const Text('Pending', style: TextStyle(fontSize: 11)),
      ),
      contentPadding: const EdgeInsets.symmetric(horizontal: 16),
      dense: true,
    );
  }
}

// -- Uploaded Tile --

class _UploadedTile extends StatelessWidget {
  const _UploadedTile({required this.item});

  final MediaItem item;

  @override
  Widget build(BuildContext context) {
    return ListTile(
      leading: Icon(
        LucideIcons.cloud,
        size: 20,
        color: Colors.green,
      ),
      title: Text(
        item.fileName,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
        style: const TextStyle(fontSize: 14),
      ),
      trailing: Icon(LucideIcons.circleCheck, size: 18, color: Colors.green),
      contentPadding: const EdgeInsets.symmetric(horizontal: 16),
      dense: true,
    );
  }
}

// -- Failed Tile --

class _FailedTile extends StatelessWidget {
  const _FailedTile({required this.item, required this.onRetry});

  final MediaItem item;
  final VoidCallback onRetry;

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;
    return ListTile(
      leading: Icon(
        LucideIcons.circleAlert,
        size: 20,
        color: cs.error,
      ),
      title: Text(
        item.fileName,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
        style: const TextStyle(fontSize: 14),
      ),
      trailing: TextButton(
        onPressed: onRetry,
        child: const Text('Retry'),
      ),
      contentPadding: const EdgeInsets.symmetric(horizontal: 16),
      dense: true,
    );
  }
}
