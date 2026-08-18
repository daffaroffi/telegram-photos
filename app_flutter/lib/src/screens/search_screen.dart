import 'package:flutter/material.dart';

import '../../src/rust/api/db.dart' as core;
import '../../src/rust/api/mirror.dart';

/// Tab Search (PRD Part 2 §3.2): instant results by file name, caption and
/// #hashtag; quick chips for common filters.
class SearchScreen extends StatefulWidget {
  const SearchScreen({super.key});

  @override
  State<SearchScreen> createState() => _SearchScreenState();
}

class _SearchScreenState extends State<SearchScreen> {
  final TextEditingController _query = TextEditingController();
  String _mode = 'all'; // all | videos | screenshots | recent
  List<MediaItem> _results = [];
  bool _searched = false;

  @override
  void dispose() {
    _query.dispose();
    super.dispose();
  }

  void _runSearch(String value) {
    final q = value.trim().toLowerCase();
    setState(() {
      _searched = true;
      if (q.isEmpty) {
        _results = [];
        return;
      }

      final all = core.listTimeline(beforeTimestamp: null, limit: 5000);
      var hits = <MediaItem>[];

      // #hashtag search goes through the caption_tags index.
      if (q.startsWith('#')) {
        final ids = core
            .searchByHashtag(tag: q.substring(1))
            .toSet();
        hits = all.where((m) => ids.contains(m.id)).toList();
      } else {
        hits = all
            .where((m) =>
                m.fileName.toLowerCase().contains(q) ||
                (core.getCaption(mediaId: m.id) ?? '')
                    .toLowerCase()
                    .contains(q))
            .toList();
      }

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
          final cutoff =
              DateTime.now().subtract(const Duration(days: 30)).millisecondsSinceEpoch;
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
        title: TextField(
          controller: _query,
          autofocus: false,
          decoration: const InputDecoration(
            hintText: 'Search photos, captions, #tags…',
            border: InputBorder.none,
          ),
          onChanged: _runSearch,
        ),
        actions: [
          if (_query.text.isNotEmpty)
            IconButton(
              icon: const Icon(Icons.clear),
              onPressed: () {
                _query.clear();
                _runSearch('');
              },
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
      return const Center(
        child: Text('Search your photos, captions and #hashtags.'),
      );
    }
    if (_results.isEmpty) {
      return const Center(child: Text('No matches.'));
    }

    return ListView.builder(
      itemCount: _results.length,
      itemBuilder: (context, i) {
        final m = _results[i];
        return ListTile(
          leading: Icon(
            m.mediaType == 'video' ? Icons.movie_outlined : Icons.image_outlined,
          ),
          title: Text(m.fileName, maxLines: 1, overflow: TextOverflow.ellipsis),
          subtitle: Text(_formatDate(m.dateTaken)),
          trailing: Text(_formatSize(m.fileSizeBytes)),
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

class _QuickChips extends StatelessWidget {
  const _QuickChips({required this.selected, required this.onSelect});

  final String selected;
  final ValueChanged<String> onSelect;

  @override
  Widget build(BuildContext context) {
    final chips = [
      ('all', 'All'),
      ('videos', 'Videos'),
      ('screenshots', 'Screenshots'),
      ('recent', 'Last 30 days'),
    ];

    return SizedBox(
      height: 48,
      child: ListView.separated(
        scrollDirection: Axis.horizontal,
        padding: const EdgeInsets.symmetric(horizontal: 12),
        itemCount: chips.length,
        separatorBuilder: (_, _) => const SizedBox(width: 8),
        itemBuilder: (context, i) {
          final (value, label) = chips[i];
          return ChoiceChip(
            label: Text(label),
            selected: selected == value,
            onSelected: (_) => onSelect(value),
          );
        },
      ),
    );
  }
}
