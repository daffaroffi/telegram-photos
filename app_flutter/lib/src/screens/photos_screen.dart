import 'dart:io';

import 'package:flutter/material.dart';

import '../../src/platform/media_scan.dart';
import '../../src/rust/api/db.dart' as core;
import '../../src/rust/api/mirror.dart';
import '../widgets/backup_banner.dart';
import '../widgets/status_badge.dart';

/// Tab Photos (PRD Part 2 §3.1): combined local+cloud timeline in one
/// virtualized grid, status badge per photo, backup banner on progress.
class PhotosScreen extends StatefulWidget {
  const PhotosScreen({super.key});

  @override
  State<PhotosScreen> createState() => _PhotosScreenState();
}

class _PhotosScreenState extends State<PhotosScreen> {
  static const _pageSize = 60;

  final List<MediaItem> _items = [];
  bool _loading = false;
  bool _hasMore = true;
  bool _firstLoad = true;

  @override
  void initState() {
    super.initState();
    _loadPage();
    // PRD Part 2 §4.1: after granting photo access, auto-scan on first run
    // so the grid fills without the user hunting for a button.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (core.countMedia() == 0) _scanGallery();
    });
  }

  Future<void> _loadPage({bool refresh = false}) async {
    if (_loading) return;
    setState(() => _loading = true);

    final before = refresh || _items.isEmpty
        ? null
        : _items.last.dateTaken;
    final page = core.listTimeline(beforeTimestamp: before, limit: _pageSize);

    setState(() {
      if (refresh || _items.isEmpty) {
        _items
          ..clear()
          ..addAll(page);
      } else {
        _items.addAll(page);
      }
      _hasMore = page.length == _pageSize;
      _loading = false;
      _firstLoad = false;
    });
  }

  int get _columns {
    final c = core.getSettings().gridColumnCount;
    return c > 0 && c <= 8 ? c : 3;
  }

  @override
  Widget build(BuildContext context) {
    final summary = core.uploadsSummary();
    final settings = core.getSettings();

    return Scaffold(
      appBar: AppBar(
        title: const Text('Photos'),
        actions: [
          IconButton(
            tooltip: 'Refresh',
            icon: const Icon(Icons.refresh),
            onPressed: () => _loadPage(refresh: true),
          ),
        ],
      ),
      body: Column(
        children: [
          BackupBanner(
            summary: summary,
            onTap: () => _openProgressHub(context, summary),
          ),
          Expanded(child: _buildBody(settings)),
        ],
      ),
      floatingActionButton: _items.isEmpty
          ? null
          : FloatingActionButton.extended(
              onPressed: _scanGallery,
              icon: const Icon(Icons.add_photo_alternate_outlined),
              label: const Text('Scan gallery'),
            ),
    );
  }

  Widget _buildBody(AppSettings settings) {
    if (_firstLoad) {
      return const Center(child: CircularProgressIndicator());
    }
    if (_items.isEmpty) {
      return _EmptyState(onRescan: _scanGallery);
    }

    return RefreshIndicator(
      onRefresh: () => _loadPage(refresh: true),
      child: CustomScrollView(
        // Keyset pagination: load more when scrolled near the end.
        controller: _scrollController,
        slivers: [
          SliverPadding(
            padding: const EdgeInsets.all(2),
            sliver: SliverGrid(
              gridDelegate: SliverGridDelegateWithFixedCrossAxisCount(
                crossAxisCount: _columns,
                mainAxisSpacing: 2,
                crossAxisSpacing: 2,
              ),
              delegate: SliverChildBuilderDelegate(
                (context, i) => _GridTile(item: _items[i]),
                childCount: _items.length,
              ),
            ),
          ),
          if (_hasMore)
            SliverToBoxAdapter(
              child: Padding(
                padding: const EdgeInsets.all(12),
                child: Center(
                  child: _loading
                      ? const CircularProgressIndicator()
                      : const Text(''),
                ),
              ),
            ),
        ],
      ),
    );
  }

  late final ScrollController _scrollController = ScrollController()
    ..addListener(() {
      if (_scrollController.position.pixels >
          _scrollController.position.maxScrollExtent - 600) {
        _loadPage();
      }
    });

  void _openProgressHub(BuildContext context, UploadsSummary summary) {
    showModalBottomSheet(
      context: context,
      showDragHandle: true,
      builder: (_) => _ProgressHubSheet(summary: summary),
    );
  }

  /// Scans the device gallery through the native MediaStore channel, imports
  /// the results into the Rust core, then refreshes the timeline.
  Future<void> _scanGallery() async {
    final messenger = ScaffoldMessenger.of(context);
    final errorColor = Theme.of(context).colorScheme.error;
    try {
      final json = await MediaScan.scanGalleryJson();
      final added = core.importScanResults(json: json);
      if (!mounted) return;
      setState(() => _items.clear());
      await _loadPage(refresh: true);
      messenger.showSnackBar(
        SnackBar(content: Text('$added photos & videos found')),
      );
    } catch (e) {
      messenger.showSnackBar(
        SnackBar(
          content: Text('Scan failed: $e — check photo permission'),
          backgroundColor: errorColor,
        ),
      );
    }
  }
}

class _GridTile extends StatelessWidget {
  const _GridTile({required this.item});

  final MediaItem item;

  @override
  Widget build(BuildContext context) {
    final badge = photoStatusFromSync(item.syncStatus);

    return GestureDetector(
      onTap: () => _openPreview(context),
      child: Stack(
        fit: StackFit.expand,
        children: [
          _Thumbnail(item: item),
          Positioned(
            right: 4,
            bottom: 4,
            child: StatusBadge(status: badge),
          ),
          if (item.mediaType == 'video' && item.durationMs != null)
            Positioned(
              left: 4,
              bottom: 4,
              child: _DurationChip(durationMs: item.durationMs!),
            ),
        ],
      ),
    );
  }

  void _openPreview(BuildContext context) {
    showDialog<void>(
      context: context,
      builder: (_) => Dialog.fullscreen(
        child: _PreviewDialog(item: item),
      ),
    );
  }
}

/// Placeholder thumbnail until the thumbnail pipeline (PRD Part 2 §7.1) lands:
/// shows a deterministic color derived from the item id + media icon.
class _Thumbnail extends StatelessWidget {
  const _Thumbnail({required this.item});

  final MediaItem item;

  @override
  Widget build(BuildContext context) {
    final color = Colors.primaries[
        item.id.hashCode.abs() % Colors.primaries.length].shade300;

    if (item.thumbnailPath != null) {
      return Image.file(
        File(item.thumbnailPath!),
        fit: BoxFit.cover,
        errorBuilder: (_, _, _) => _fallback(color),
      );
    }
    return _fallback(color);
  }

  Widget _fallback(Color color) {
    return Container(
      color: color.withValues(alpha: 0.45),
      child: Center(
        child: Icon(
          item.mediaType == 'video' ? Icons.movie_outlined : Icons.image_outlined,
          color: Colors.white.withValues(alpha: 0.85),
          size: 28,
        ),
      ),
    );
  }
}

class _DurationChip extends StatelessWidget {
  const _DurationChip({required this.durationMs});

  final int durationMs;

  @override
  Widget build(BuildContext context) {
    final s = (durationMs / 1000).round();
    final text = '${s ~/ 60}:${(s % 60).toString().padLeft(2, '0')}';
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 2),
      decoration: BoxDecoration(
        color: Colors.black.withValues(alpha: 0.6),
        borderRadius: BorderRadius.circular(4),
      ),
      child: Text(
        text,
        style: const TextStyle(color: Colors.white, fontSize: 10),
      ),
    );
  }
}

/// Minimal unified preview (PRD Part 2 §4.3) — fullscreen, swipe between
/// photos is a P1 follow-up once the thumbnail pipeline lands.
class _PreviewDialog extends StatelessWidget {
  const _PreviewDialog({required this.item});

  final MediaItem item;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: Colors.black,
      appBar: AppBar(
        backgroundColor: Colors.black,
        foregroundColor: Colors.white,
        title: Text(
          item.fileName,
          style: const TextStyle(fontSize: 14),
        ),
      ),
      body: Center(
        child: _Thumbnail(item: item),
      ),
    );
  }
}

class _EmptyState extends StatelessWidget {
  const _EmptyState({required this.onRescan});

  final VoidCallback onRescan;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(Icons.photo_library_outlined,
              size: 64, color: Theme.of(context).colorScheme.outline),
          const SizedBox(height: 12),
          Text('No photos yet', style: Theme.of(context).textTheme.titleMedium),
          const SizedBox(height: 4),
          Text(
            'Scan your gallery to start backing up.',
            style: Theme.of(context).textTheme.bodyMedium,
          ),
          const SizedBox(height: 16),
          FilledButton.icon(
            onPressed: onRescan,
            icon: const Icon(Icons.refresh),
            label: const Text('Scan gallery'),
          ),
        ],
      ),
    );
  }
}

/// Task Progress Hub (T1) — bottom sheet showing upload queue & failures.
class _ProgressHubSheet extends StatelessWidget {
  const _ProgressHubSheet({required this.summary});

  final UploadsSummary summary;

  @override
  Widget build(BuildContext context) {
    final failed = core.listUploadsByStatus(status: 'FAILED');

    return SafeArea(
      child: Padding(
        padding: const EdgeInsets.fromLTRB(20, 0, 20, 20),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text('Backup progress', style: Theme.of(context).textTheme.titleLarge),
            const SizedBox(height: 12),
            _StatRow(
              label: 'Backed up',
              count: summary.backedUpCount,
              bytes: summary.backedUpBytes,
            ),
            _StatRow(
              label: 'Uploading / queued',
              count: summary.uploadingCount + summary.queuedCount,
              bytes: summary.uploadingBytes + summary.queuedBytes,
            ),
            _StatRow(
              label: 'Failed',
              count: summary.failedCount,
              bytes: summary.failedBytes,
            ),
            if (failed.isNotEmpty) ...[
              const SizedBox(height: 8),
              const Text('Failed items (tap to retry):',
                  style: TextStyle(fontWeight: FontWeight.w600)),
              const SizedBox(height: 4),
              ...failed.take(5).map((u) => ListTile(
                    dense: true,
                    contentPadding: EdgeInsets.zero,
                    leading: const Icon(Icons.error_outline, color: Colors.red),
                    title: Text(
                      u.mediaId,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                    subtitle: Text(u.lastError ?? ''),
                    trailing: TextButton(
                      onPressed: () => core.retryUpload(uploadId: u.id),
                      child: const Text('Retry'),
                    ),
                  )),
            ],
            const SizedBox(height: 8),
            Text(
              'Uploads continue in the background.',
              style: Theme.of(context).textTheme.bodySmall,
            ),
          ],
        ),
      ),
    );
  }
}

class _StatRow extends StatelessWidget {
  const _StatRow({required this.label, required this.count, required this.bytes});

  final String label;
  final int count;
  final int bytes;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 4),
      child: Row(
        children: [
          Expanded(child: Text(label)),
          Text(
            '$count · ${_fmt(bytes)}',
            style: Theme.of(context).textTheme.bodyMedium,
          ),
        ],
      ),
    );
  }

  String _fmt(int b) {
    if (b < 1024 * 1024) return '${(b / 1024).toStringAsFixed(0)} KB';
    return '${(b / (1024 * 1024)).toStringAsFixed(1)} MB';
  }
}
