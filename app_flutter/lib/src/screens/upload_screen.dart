import 'package:flutter/material.dart';

import '../rust/api/db.dart' as core;
import '../rust/api/mirror.dart';

/// Upload / backup progress screen (PRD Part 2 §4.4).
///
/// Design principles applied:
/// - Subtractive: only show what matters — progress, failures, action
/// - Engineering constraints: empty, loading, error, partial states
/// - Clear hierarchy: total progress > individual items > actions
/// - Platform conventions: Material 3, proper touch targets
class UploadScreen extends StatefulWidget {
  const UploadScreen({super.key});

  @override
  State<UploadScreen> createState() => _UploadScreenState();
}

class _UploadScreenState extends State<UploadScreen> {
  List<Upload> _uploads = [];
  UploadsSummary _summary = UploadsSummary(
    queuedCount: 0,
    queuedBytes: 0,
    uploadingCount: 0,
    uploadingBytes: 0,
    failedCount: 0,
    failedBytes: 0,
    backedUpCount: 0,
    backedUpBytes: 0,
  );
  bool _loading = true;

  @override
  void initState() {
    super.initState();
    _refresh();
  }

  void _refresh() {
    setState(() {
      _uploads = _loadUploads();
      _summary = core.uploadsSummary();
      _loading = false;
    });
  }

  List<Upload> _loadUploads() {
    final queued = core.listUploadsByStatus(status: 'QUEUED');
    final uploading = core.listUploadsByStatus(status: 'UPLOADING');
    final failed = core.listUploadsByStatus(status: 'FAILED');
    return [...uploading, ...queued, ...failed];
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Backup Progress'),
        actions: [
          IconButton(
            icon: const Icon(Icons.refresh),
            onPressed: _refresh,
            tooltip: 'Refresh',
          ),
        ],
      ),
      body: _loading
          ? const Center(child: CircularProgressIndicator())
          : _buildBody(),
    );
  }

  Widget _buildBody() {
    if (_uploads.isEmpty && _summary.backedUpCount == 0) {
      return _EmptyState();
    }

    return CustomScrollView(
      slivers: [
        // Summary card
        SliverToBoxAdapter(
          child: _SummaryCard(summary: _summary),
        ),

        // Active uploads
        if (_uploads.isNotEmpty) ...[
          SliverToBoxAdapter(
            child: Padding(
              padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
              child: Text(
                'Active uploads',
                style: Theme.of(context).textTheme.titleSmall?.copyWith(
                      color: Theme.of(context).colorScheme.primary,
                    ),
              ),
            ),
          ),
          SliverList(
            delegate: SliverChildBuilderDelegate(
              (context, i) => _UploadTile(
                upload: _uploads[i],
                onRetry: () {
                  core.retryUpload(uploadId: _uploads[i].id);
                  _refresh();
                },
              ),
              childCount: _uploads.length,
            ),
          ),
        ],

        // Backed up count
        if (_summary.backedUpCount > 0)
          SliverToBoxAdapter(
            child: Padding(
              padding: const EdgeInsets.all(16),
              child: Row(
                children: [
                  Icon(
                    Icons.check_circle_outline,
                    size: 16,
                    color: Theme.of(context).colorScheme.primary,
                  ),
                  const SizedBox(width: 8),
                  Text(
                    '${_summary.backedUpCount} photos backed up',
                    style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                          color: Theme.of(context).colorScheme.primary,
                        ),
                  ),
                ],
              ),
            ),
          ),
      ],
    );
  }
}

/// Summary card showing total backup progress.
class _SummaryCard extends StatelessWidget {
  const _SummaryCard({required this.summary});
  final UploadsSummary summary;

  @override
  Widget build(BuildContext context) {
    final total = summary.queuedCount +
        summary.uploadingCount +
        summary.failedCount +
        summary.backedUpCount;
    final done = summary.backedUpCount;
    final progress = total > 0 ? done / total : 0.0;

    return Card(
      margin: const EdgeInsets.all(16),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [
                Text(
                  '$done / $total',
                  style: Theme.of(context).textTheme.headlineSmall,
                ),
                if (summary.uploadingCount > 0)
                  const SizedBox(
                    width: 20,
                    height: 20,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  ),
              ],
            ),
            const SizedBox(height: 12),
            ClipRRect(
              borderRadius: BorderRadius.circular(4),
              child: LinearProgressIndicator(
                value: progress,
                minHeight: 6,
              ),
            ),
            const SizedBox(height: 12),
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [
                _Stat(
                  label: 'Queued',
                  count: summary.queuedCount,
                  color: Theme.of(context).colorScheme.outline,
                ),
                _Stat(
                  label: 'Uploading',
                  count: summary.uploadingCount,
                  color: Theme.of(context).colorScheme.primary,
                ),
                _Stat(
                  label: 'Failed',
                  count: summary.failedCount,
                  color: Theme.of(context).colorScheme.error,
                ),
                _Stat(
                  label: 'Done',
                  count: summary.backedUpCount,
                  color: Theme.of(context).colorScheme.primary,
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

class _Stat extends StatelessWidget {
  const _Stat({required this.label, required this.count, required this.color});
  final String label;
  final int count;
  final Color color;

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        Text(
          '$count',
          style: Theme.of(context).textTheme.titleMedium?.copyWith(color: color),
        ),
        Text(
          label,
          style: Theme.of(context).textTheme.bodySmall,
        ),
      ],
    );
  }
}

/// Individual upload tile with progress and retry.
class _UploadTile extends StatelessWidget {
  const _UploadTile({required this.upload, required this.onRetry});
  final Upload upload;
  final VoidCallback onRetry;

  @override
  Widget build(BuildContext context) {
    final isFailed = upload.status == 'FAILED';
    final isUploading = upload.status == 'UPLOADING';
    final progress = upload.totalBytes > 0
        ? (upload.uploadedBytes / upload.totalBytes).clamp(0.0, 1.0)
        : 0.0;

    return ListTile(
      leading: isFailed
          ? const Icon(Icons.error_outline, color: Colors.red)
          : isUploading
              ? const SizedBox(
                  width: 24,
                  height: 24,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : const Icon(Icons.hourglass_empty, color: Colors.orange),
      title: Text(
        upload.mediaId,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
      ),
      subtitle: isFailed
          ? Text(
              upload.lastError ?? 'Unknown error',
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(color: Theme.of(context).colorScheme.error),
            )
          : isUploading
              ? LinearProgressIndicator(value: progress)
              : null,
      trailing: isFailed
          ? TextButton(
              onPressed: onRetry,
              child: const Text('Retry'),
            )
          : Text(
              '${(progress * 100).toInt()}%',
              style: Theme.of(context).textTheme.bodySmall,
            ),
    );
  }
}

/// Empty state when no uploads exist.
class _EmptyState extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(
            Icons.cloud_upload_outlined,
            size: 48,
            color: Theme.of(context).colorScheme.outline,
          ),
          const SizedBox(height: 12),
          Text(
            'No uploads yet',
            style: Theme.of(context).textTheme.titleMedium,
          ),
          const SizedBox(height: 4),
          Text(
            'Photos will appear here when backup starts.',
            style: Theme.of(context).textTheme.bodySmall,
          ),
        ],
      ),
    );
  }
}
