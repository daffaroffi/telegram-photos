import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../../main.dart' show telegramHandle;
import '../platform/media_scan.dart';
import '../../src/rust/api/db.dart' as core;
import '../../src/rust/api/mirror.dart';
import '../../src/rust/api/telegram.dart' as tg;
import '../widgets/backup_banner.dart';
import '../widgets/status_badge.dart';
import 'upload_screen.dart';
import 'captions_screen.dart';

/// Tab Photos (PRD Part 2 S3.1): combined local+cloud timeline in one
/// virtualized grid, status badge per photo, backup banner on progress.
/// Supports multi-select for bulk upload.
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

  // Multi-select state.
  bool _selectionMode = false;
  final Set<String> _selectedIds = {};

  @override
  void initState() {
    super.initState();
    _loadPage();
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      if (core.countMedia() == 0) {
        _scanGallery();
      } else {
        await _ensureThumbnails();
        if (mounted) setState(() {});
      }
    });
  }

  void _toggleSelection(String id) {
    HapticFeedback.selectionClick();
    setState(() {
      if (_selectedIds.contains(id)) {
        _selectedIds.remove(id);
        if (_selectedIds.isEmpty) _selectionMode = false;
      } else {
        _selectedIds.add(id);
      }
    });
  }

  void _enterSelectionMode(String id) {
    HapticFeedback.mediumImpact();
    setState(() {
      _selectionMode = true;
      _selectedIds.add(id);
    });
  }

  Future<void> _editCaptionForSelected() async {
    if (_selectedIds.length != 1) return;
    final id = _selectedIds.first;
    final item = _items.firstWhere(
      (m) => m.id == id,
      orElse: () => _items.first,
    );
    await showModalBottomSheet(
      context: context,
      isScrollControlled: true,
      showDragHandle: false,
      builder: (_) => CaptionsScreen(item: item),
    );
    if (mounted) _exitSelectionMode();
  }

  void _exitSelectionMode() {
    setState(() {
      _selectionMode = false;
      _selectedIds.clear();
    });
  }

  Future<void> _bulkUpload() async {
    if (_selectedIds.isEmpty) return;
    final messenger = ScaffoldMessenger.of(context);
    final items = _items.where((m) => _selectedIds.contains(m.id)).toList();
    int success = 0;
    int failed = 0;

    messenger.showSnackBar(
      SnackBar(content: Text('Uploading ${items.length} photos...')),
    );

    for (final item in items) {
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
        success++;
      } catch (e) {
        failed++;
      }
    }

    _exitSelectionMode();
    if (mounted) {
      messenger.showSnackBar(
        SnackBar(
          content: Text(
            'Upload complete: $success succeeded, $failed failed',
          ),
        ),
      );
      setState(() {});
    }
  }

  Future<void> _loadPage({bool refresh = false}) async {
    if (_loading) return;
    setState(() => _loading = true);

    final before = refresh || _items.isEmpty ? null : _items.last.dateTaken;
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

    return Scaffold(
      appBar: _selectionMode
          ? AppBar(
              leading: IconButton(
                icon: const Icon(LucideIcons.x),
                onPressed: _exitSelectionMode,
                tooltip: 'Cancel selection',
              ),
              title: Text('${_selectedIds.length} selected'),
              actions: [
                IconButton(
                  icon: const Icon(LucideIcons.checkCheck),
                  tooltip: 'Select all',
                  onPressed: () {
                    setState(() {
                      _selectedIds.addAll(_items.map((m) => m.id));
                    });
                  },
                ),
                if (_selectedIds.length == 1)
                  IconButton(
                    icon: const Icon(LucideIcons.penLine),
                    tooltip: 'Edit caption',
                    onPressed: _editCaptionForSelected,
                  ),
                IconButton(
                  icon: const Icon(LucideIcons.upload),
                  tooltip: 'Upload selected',
                  onPressed: _selectedIds.isEmpty ? null : _bulkUpload,
                ),
              ],
            )
          : AppBar(
              title: const Text('Photos'),
              actions: [
                if (_items.isNotEmpty)
                  IconButton(
                    tooltip: 'Scan gallery',
                    icon: const Icon(LucideIcons.scan),
                    onPressed: _scanGallery,
                  ),
                IconButton(
                  tooltip: 'Refresh',
                  icon: const Icon(LucideIcons.refreshCw),
                  onPressed: () => _loadPage(refresh: true),
                ),
              ],
            ),
      body: Column(
        children: [
          if (!_selectionMode) ...[
            BackupBanner(
              summary: summary,
              onTap: () => _openProgressHub(context),
            ),
          ],
          Expanded(child: _buildBody()),
        ],
      ),
    );
  }

  Widget _buildBody() {
    if (_firstLoad) {
      return _LoadingSkeleton(columns: _columns);
    }
    if (_items.isEmpty) {
      return _EmptyState(onRescan: _scanGallery);
    }

    return RefreshIndicator(
      onRefresh: () => _loadPage(refresh: true),
      child: CustomScrollView(
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
                (context, i) => _GridTile(
                  item: _items[i],
                  selected: _selectedIds.contains(_items[i].id),
                  selectionMode: _selectionMode,
                  onTap: () {
                    if (_selectionMode) {
                      _toggleSelection(_items[i].id);
                    } else {
                      _openPreview(context, _items[i]);
                    }
                  },
                  onLongPress: () {
                    if (!_selectionMode) {
                      _enterSelectionMode(_items[i].id);
                    }
                  },
                ),
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

  void _openProgressHub(BuildContext context) {
    Navigator.of(context).push(
      MaterialPageRoute(builder: (_) => const UploadScreen()),
    );
  }

  void _openPreview(BuildContext context, MediaItem item) {
    showModalBottomSheet(
      context: context,
      isScrollControlled: true,
      useSafeArea: true,
      builder: (_) => _PreviewBottomSheet(item: item),
    );
  }

  Future<void> _scanGallery() async {
    final messenger = ScaffoldMessenger.of(context);
    final errorColor = Theme.of(context).colorScheme.error;
    try {
      final json = await MediaScan.scanGalleryJson();
      final added = core.importScanResults(json: json);
      await _ensureThumbnails();
      if (!mounted) return;
      setState(() => _items.clear());
      await _loadPage(refresh: true);
      messenger.showSnackBar(
        SnackBar(content: Text('$added photos & videos found')),
      );
    } catch (e) {
      messenger.showSnackBar(
        SnackBar(
          content: Text('Scan failed: $e'),
          backgroundColor: errorColor,
          action: SnackBarAction(label: 'Retry', onPressed: _scanGallery),
        ),
      );
    }
  }

  Future<void> _ensureThumbnails() async {
    final missing = core.listMediaWithoutThumb(limit: 500);
    if (missing.isEmpty) return;
    final mapJson = await MediaScan.generateThumbnails(ids: missing);
    core.saveThumbnailPaths(json: mapJson);
  }
}

// ─── Grid Tile ────────────────────────────────────────────────────────────────

class _GridTile extends StatelessWidget {
  const _GridTile({
    required this.item,
    required this.selected,
    required this.selectionMode,
    required this.onTap,
    required this.onLongPress,
  });

  final MediaItem item;
  final bool selected;
  final bool selectionMode;
  final VoidCallback onTap;
  final VoidCallback onLongPress;

  @override
  Widget build(BuildContext context) {
    final badge = photoStatusFromSync(item.syncStatus);

    return GestureDetector(
      onTap: onTap,
      onLongPress: onLongPress,
      child: Semantics(
        label: '${item.fileName}${selected ? ', selected' : ''}',
        button: true,
        child: Stack(
          fit: StackFit.expand,
          children: [
            _Thumbnail(item: item),
            if (selectionMode)
              Positioned.fill(
                child: Container(
                  decoration: BoxDecoration(
                    color: selected
                        ? Theme.of(context)
                            .colorScheme
                            .primary
                            .withValues(alpha: 0.3)
                        : Colors.black.withValues(alpha: 0.05),
                  ),
                ),
              ),
            if (selectionMode)
              Positioned(
                top: 4,
                left: 4,
                child: Icon(
                  selected ? LucideIcons.circleCheck : LucideIcons.circle,
                  color: selected
                      ? Theme.of(context).colorScheme.primary
                      : Colors.white,
                  size: 22,
                  shadows: const [Shadow(blurRadius: 3, color: Colors.black)],
                ),
              ),
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
      ),
    );
  }
}

// ─── Thumbnail ────────────────────────────────────────────────────────────────

class _Thumbnail extends StatelessWidget {
  const _Thumbnail({required this.item});

  final MediaItem item;

  @override
  Widget build(BuildContext context) {
    final color =
        Colors.primaries[item.id.hashCode.abs() % Colors.primaries.length]
            .shade300;

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
          item.mediaType == 'video'
              ? LucideIcons.video
              : LucideIcons.image,
          color: Colors.white.withValues(alpha: 0.85),
          size: 28,
        ),
      ),
    );
  }
}

// ─── Duration Chip ────────────────────────────────────────────────────────────

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

// ─── Preview Bottom Sheet ─────────────────────────────────────────────────────

class _PreviewBottomSheet extends StatefulWidget {
  const _PreviewBottomSheet({required this.item});

  final MediaItem item;

  @override
  State<_PreviewBottomSheet> createState() => _PreviewBottomSheetState();
}

class _PreviewBottomSheetState extends State<_PreviewBottomSheet> {
  bool _uploading = false;
  String? _error;
  bool _uploadSuccess = false;

  Future<void> _uploadToVault() async {
    if (_uploading) return;
    HapticFeedback.lightImpact();
    setState(() {
      _uploading = true;
      _error = null;
    });
    try {
      final tempPath = await MediaScan.readFileToTemp(widget.item.id);
      final ext = tempPath.split('.').last.toLowerCase();
      final isVideo = ['mp4', 'mov', 'avi', 'mkv', 'webm'].contains(ext);
      final mimeType = isVideo ? 'video/mp4' : 'image/jpeg';
      await tg.uploadPhoto(
        handle: telegramHandle,
        filePath: tempPath,
        fileName: widget.item.fileName,
        mimeType: mimeType,
        isVideo: isVideo,
      );
      core.setMediaStatus(id: widget.item.id, status: 1);
      HapticFeedback.mediumImpact();
      if (mounted) {
        setState(() {
          _uploading = false;
          _uploadSuccess = true;
        });
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Uploaded successfully')),
        );
      }
    } catch (e) {
      HapticFeedback.heavyImpact();
      if (mounted) {
        setState(() {
          _error = e.toString();
          _uploading = false;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final isNotBackedUp = widget.item.syncStatus == 'NOT_BACKED_UP';
    final bottomPadding = MediaQuery.of(context).viewInsets.bottom;

    return Container(
      constraints: BoxConstraints(
        maxHeight: MediaQuery.of(context).size.height * 0.85,
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          // Handle bar
          Center(
            child: Container(
              margin: const EdgeInsets.only(top: 12),
              width: 40,
              height: 4,
              decoration: BoxDecoration(
                color: Theme.of(context).colorScheme.outlineVariant,
                borderRadius: BorderRadius.circular(2),
              ),
            ),
          ),
          // Photo preview
          Flexible(
            child: Padding(
              padding: const EdgeInsets.all(16),
              child: ClipRRect(
                borderRadius: BorderRadius.circular(12),
                child: AspectRatio(
                  aspectRatio: 1,
                  child: _Thumbnail(item: widget.item),
                ),
              ),
            ),
          ),
          // File info
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16),
            child: Row(
              children: [
                Icon(
                  widget.item.mediaType == 'video'
                      ? LucideIcons.video
                      : LucideIcons.image,
                  size: 16,
                  color: Theme.of(context).colorScheme.onSurfaceVariant,
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    widget.item.fileName,
                    style: Theme.of(context).textTheme.bodyMedium,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                Text(
                  _formatSize(widget.item.fileSizeBytes),
                  style: Theme.of(context).textTheme.bodySmall?.copyWith(
                        color: Theme.of(context).colorScheme.onSurfaceVariant,
                      ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 16),
          // Action buttons
          Padding(
            padding: EdgeInsets.fromLTRB(16, 0, 16, 16 + bottomPadding),
            child: Row(
              children: [
                if (isNotBackedUp && !_uploadSuccess)
                  Expanded(
                    child: FilledButton.icon(
                      onPressed: _uploading ? null : _uploadToVault,
                      icon: _uploading
                          ? const SizedBox(
                              width: 18,
                              height: 18,
                              child: CircularProgressIndicator(
                                strokeWidth: 2,
                              ),
                            )
                          : const Icon(LucideIcons.upload),
                      label: Text(_uploading ? 'Uploading...' : 'Upload to vault'),
                      style: FilledButton.styleFrom(
                        minimumSize: const Size(0, 48), // 48dp touch target
                      ),
                    ),
                  ),
                if (_uploadSuccess)
                  Expanded(
                    child: FilledButton.icon(
                      onPressed: null,
                      icon: const Icon(LucideIcons.circleCheck),
                      label: const Text('Backed up'),
                      style: FilledButton.styleFrom(
                        minimumSize: const Size(0, 48),
                      ),
                    ),
                  ),
                if (!isNotBackedUp && !_uploadSuccess)
                  Expanded(
                    child: FilledButton.tonal(
                      onPressed: null,
                      style: FilledButton.styleFrom(
                        minimumSize: const Size(0, 48),
                      ),
                      child: const Text('Already backed up'),
                    ),
                  ),
              ],
            ),
          ),
          // Error bar
          if (_error != null)
            Container(
              width: double.infinity,
              color: Theme.of(context).colorScheme.errorContainer,
              padding: const EdgeInsets.all(12),
              child: Row(
                children: [
                  Icon(
                    LucideIcons.circleAlert,
                    color: Theme.of(context).colorScheme.onErrorContainer,
                    size: 18,
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      _error!,
                      style: TextStyle(
                        color: Theme.of(context).colorScheme.onErrorContainer,
                      ),
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                  TextButton(
                    onPressed: _uploadToVault,
                    child: const Text('Retry'),
                  ),
                ],
              ),
            ),
        ],
      ),
    );
  }

  String _formatSize(int bytes) {
    if (bytes < 1024 * 1024) return '${(bytes / 1024).toStringAsFixed(0)} KB';
    return '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB';
  }
}

// ─── Loading Skeleton ─────────────────────────────────────────────────────────

class _LoadingSkeleton extends StatelessWidget {
  const _LoadingSkeleton({required this.columns});

  final int columns;

  @override
  Widget build(BuildContext context) {
    return GridView.builder(
      padding: const EdgeInsets.all(2),
      gridDelegate: SliverGridDelegateWithFixedCrossAxisCount(
        crossAxisCount: columns,
        mainAxisSpacing: 2,
        crossAxisSpacing: 2,
      ),
      itemCount: columns * 6,
      itemBuilder: (_, _) => Container(
        decoration: BoxDecoration(
          color: Theme.of(context)
              .colorScheme
              .surfaceContainerHighest
              .withValues(alpha: 0.3),
          borderRadius: BorderRadius.circular(4),
        ),
      ),
    );
  }
}

// ─── Empty State ──────────────────────────────────────────────────────────────

class _EmptyState extends StatelessWidget {
  const _EmptyState({required this.onRescan});

  final VoidCallback onRescan;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(32),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              LucideIcons.image,
              size: 72,
              color: Theme.of(context).colorScheme.outline,
            ),
            const SizedBox(height: 16),
            Text(
              'No photos yet',
              style: Theme.of(context).textTheme.headlineSmall,
            ),
            const SizedBox(height: 8),
            Text(
              'Scan your gallery to start backing up photos to your private Telegram vault.',
              style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                    color: Theme.of(context).colorScheme.onSurfaceVariant,
                  ),
              textAlign: TextAlign.center,
            ),
            const SizedBox(height: 24),
            FilledButton.icon(
              onPressed: onRescan,
              icon: const Icon(LucideIcons.scan),
              label: const Text('Scan gallery'),
              style: FilledButton.styleFrom(
                minimumSize: const Size(0, 48), // 48dp touch target
              ),
            ),
          ],
        ),
      ),
    );
  }
}
