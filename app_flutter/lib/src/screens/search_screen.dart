import 'dart:async';

import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../../src/rust/api/db.dart' as core;
import '../../src/rust/api/mirror.dart';

/// Tab Search (PRD Part 2 S3.2): instant results by file name, caption and
/// #hashtag; quick chips for common filters.
class SearchScreen extends StatefulWidget {
  const SearchScreen({super.key});

  @override
  State<SearchScreen> createState() => _SearchScreenState();
}class _SearchScreenState extends State<SearchScreen> {
  final TextEditingController _query = TextEditingController();
  Timer? _debounce;
  String _mode = 'all'; // all | videos | screenshots | recent
  List<MediaItem> _results = [];
  bool _searched = false;

  @override
  void dispose() {
    _debounce?.cancel();
    _query.dispose();
    super.dispose();
  }

  void _onQueryChanged(String value) {
    _debounce?.cancel();
    _debounce = Timer(const Duration(milliseconds: 300), () => _runSearch(value));
  }

  void _runSearch(String value) {
    final q = value.trim();
    setState(() {
      _searched = true;
      if (q.isEmpty) {
        _results = [];
        return;
      }

      // Use SQL-side search instead of loading all rows into Dart memory.
      var hits = core.searchMedia(query: q, limit: 200);
      _applyMode(hits);
    });
  }

  void _applyMode(List<MediaItem> hits) {
    setState(() {
      switch (_mode) {
        case 'videos':
          _results = hits.where((m) => m.mediaType == 'video').toList();
        case 'screenshots':
          _results = hits
              .where((m) =>
                  m.fileName.toLowerCase().contains('screenshot') ||
                  m.fileName.toLowerCase().contains('screen_'))
              .toList();
        case 'recent':
          final cutoff = DateTime.now()
              .subtract(const Duration(days: 30))
              .millisecondsSinceEpoch;
          _results = hits.where((m) => m.dateTaken >= cutoff).toList();
        default:
          _results = hits;
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Semantics(
          label: 'Search photos',
          child: TextField(
            controller: _query,
            autofocus: false,
            decoration: InputDecoration(
              hintText: 'Search photos, captions, #tags...',
              prefixIcon: const Icon(LucideIcons.search, size: 20),
              border: InputBorder.none,
              hintStyle: TextStyle(
                color: Theme.of(context).colorScheme.onSurfaceVariant,
              ),
            ),              onChanged: _onQueryChanged,
          ),
        ),
        actions: [
          if (_query.text.isNotEmpty)
            IconButton(
              icon: const Icon(LucideIcons.xCircle),
              onPressed: () {
                _query.clear();
                _runSearch('');
              },
              tooltip: 'Clear search',
            ),
        ],
      ),
      body: Column(
        children: [
          _QuickChips(
            selected: _mode,
            onSelect: (m) {
              _mode = m;
              _runSearch(_query.text);
            },
          ),
          Expanded(child: _buildResults()),
        ],
      ),
    );
  }

  Widget _buildResults() {
    if (!_searched) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              LucideIcons.search,
              size: 48,
              color: Theme.of(context).colorScheme.outline,
            ),
            const SizedBox(height: 12),
            Text(
              'Search your photos, captions and #hashtags.',
              style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                    color: Theme.of(context).colorScheme.onSurfaceVariant,
                  ),
            ),
          ],
        ),
      );
    }
    if (_results.isEmpty) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              LucideIcons.circleAlert,
              size: 48,
              color: Theme.of(context).colorScheme.outline,
            ),
            const SizedBox(height: 12),
            Text(
              'No matches found',
              style: Theme.of(context).textTheme.titleMedium,
            ),
            const SizedBox(height: 4),
            Text(
              'Try a different search term or filter.',
              style: Theme.of(context).textTheme.bodySmall?.copyWith(
                    color: Theme.of(context).colorScheme.onSurfaceVariant,
                  ),
            ),
          ],
        ),
      );
    }

    return ListView.builder(
      itemCount: _results.length,
      itemBuilder: (context, i) {
        final m = _results[i];
        return ListTile(
          leading: Icon(
            m.mediaType == 'video' ? LucideIcons.video : LucideIcons.image,
            color: Theme.of(context).colorScheme.onSurfaceVariant,
          ),
          title: Text(m.fileName, maxLines: 1, overflow: TextOverflow.ellipsis),
          subtitle: Text(_formatDate(m.dateTaken)),
          trailing: Text(
            _formatSize(m.fileSizeBytes),
            style: Theme.of(context).textTheme.bodySmall?.copyWith(
                  color: Theme.of(context).colorScheme.onSurfaceVariant,
                ),
          ),
          contentPadding: const EdgeInsets.symmetric(horizontal: 16, vertical: 4),
        );
      },
    );
  }

  String _formatDate(int ms) {
    final dt = DateTime.fromMillisecondsSinceEpoch(ms);
    return '${dt.day} ${_months[dt.month - 1]} ${dt.year}';
  }

  static const _months = [
    'Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun',
    'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec',
  ];

  String _formatSize(int bytes) {
    if (bytes < 1024 * 1024) return '${(bytes / 1024).toStringAsFixed(0)} KB';
    return '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB';
  }
}

// ─── Quick Chips ──────────────────────────────────────────────────────────────

class _QuickChips extends StatelessWidget {
  const _QuickChips({required this.selected, required this.onSelect});

  final String selected;
  final ValueChanged<String> onSelect;

  @override
  Widget build(BuildContext context) {
    final chips = [
      ('all', 'All', LucideIcons.images),
      ('videos', 'Videos', LucideIcons.video),
      ('screenshots', 'Screenshots', LucideIcons.camera),
      ('recent', 'Last 30 days', LucideIcons.clock),
    ];

    return SizedBox(
      height: 56, // Increased for better touch target
      child: ListView.separated(
        scrollDirection: Axis.horizontal,
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
        itemCount: chips.length,
        separatorBuilder: (_, _) => const SizedBox(width: 8),
        itemBuilder: (context, i) {
          final (value, label, icon) = chips[i];
          return ChoiceChip(
            label: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Icon(icon, size: 16),
                const SizedBox(width: 6),
                Text(label),
              ],
            ),
            selected: selected == value,
            onSelected: (_) => onSelect(value),
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
          );
        },
      ),
    );
  }
}
