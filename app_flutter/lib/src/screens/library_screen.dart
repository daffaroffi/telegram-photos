import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../../src/rust/api/db.dart' as core;
import '../../src/rust/api/mirror.dart';

/// Tab Library (PRD Part 2 S3.3): Collections, Folders, Favorites, Memories,
/// Trash. Collections are backed by the real DB; the rest are P1 follow-ups.
class LibraryScreen extends StatefulWidget {
  const LibraryScreen({super.key});

  @override
  State<LibraryScreen> createState() => _LibraryScreenState();
}

class _LibraryScreenState extends State<LibraryScreen> {
  List<Collection> _collections = [];

  @override
  void initState() {
    super.initState();
    _reload();
  }

  void _reload() {
    setState(() => _collections = core.listCollections());
  }

  Future<void> _createCollection() async {
    final controller = TextEditingController();
    final name = await showDialog<String>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('New collection'),
        content: TextField(
          controller: controller,
          autofocus: true,
          decoration: const InputDecoration(hintText: 'Collection name'),
          onSubmitted: (v) => Navigator.pop(context, v),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, controller.text),
            child: const Text('Create'),
          ),
        ],
      ),
    );
    if (name != null && name.trim().isNotEmpty) {
      core.createCollection(name: name.trim());
      _reload();
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Library'),
        actions: [
          IconButton(
            tooltip: 'New collection',
            icon: const Icon(LucideIcons.plus),
            onPressed: _createCollection,
          ),
        ],
      ),
      body: ListView(
        padding: const EdgeInsets.all(8),
        children: [
          _SectionHeader(
            icon: LucideIcons.layers,
            title: 'Collections',
            trailing: Text('${_collections.length}',
                style: Theme.of(context).textTheme.bodySmall),
          ),
          if (_collections.isEmpty)
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 24),
              child: Column(
                children: [
                  Icon(
                    LucideIcons.layers,
                    size: 48,
                    color: Theme.of(context).colorScheme.outline,
                  ),
                  const SizedBox(height: 12),
                  Text(
                    'No collections yet',
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                  const SizedBox(height: 4),
                  Text(
                    'Tap + to create your first collection.',
                    style: Theme.of(context).textTheme.bodySmall?.copyWith(
                          color: Theme.of(context).colorScheme.onSurfaceVariant,
                        ),
                  ),
                ],
              ),
            )
          else
            ..._collections.map((c) => _CollectionTile(
                  collection: c,
                  onTap: () => _openCollection(c),
                )),
          const Divider(height: 24),
          _SectionHeader(
            icon: LucideIcons.clock,
            title: 'Coming soon',
          ),
          const _PlaceholderTile(
            icon: LucideIcons.heart,
            title: 'Favorites',
            subtitle: 'P1',
          ),
          const _PlaceholderTile(
            icon: LucideIcons.clock,
            title: 'Memories',
            subtitle: 'P1',
          ),
          const _PlaceholderTile(
            icon: LucideIcons.trash2,
            title: 'Trash',
            subtitle: 'P1',
          ),
          const _PlaceholderTile(
            icon: LucideIcons.folder,
            title: 'Device folders',
            subtitle: 'P1',
          ),
        ],
      ),
    );
  }

  void _openCollection(Collection c) {
    Navigator.push(
      context,
      MaterialPageRoute(
        builder: (_) => _CollectionScreen(collection: c),
      ),
    );
  }
}

// ─── Collection Screen ────────────────────────────────────────────────────────

class _CollectionScreen extends StatefulWidget {
  const _CollectionScreen({required this.collection});

  final Collection collection;

  @override
  State<_CollectionScreen> createState() => _CollectionScreenState();
}

class _CollectionScreenState extends State<_CollectionScreen> {
  late final List<MediaItem> _items;

  @override
  void initState() {
    super.initState();
    _items = core.listCollectionItems(collectionId: widget.collection.id);
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: Text(widget.collection.name)),
      body: _items.isEmpty
          ? Center(
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Icon(
                    LucideIcons.image,
                    size: 48,
                    color: Theme.of(context).colorScheme.outline,
                  ),
                  const SizedBox(height: 12),
                  Text(
                    'Collection is empty',
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                  const SizedBox(height: 4),
                  Text(
                    'Add photos from the Photos tab.',
                    style: Theme.of(context).textTheme.bodySmall?.copyWith(
                          color: Theme.of(context).colorScheme.onSurfaceVariant,
                        ),
                  ),
                ],
              ),
            )
          : GridView.builder(
              padding: const EdgeInsets.all(2),
              gridDelegate: const SliverGridDelegateWithFixedCrossAxisCount(
                crossAxisCount: 3,
                mainAxisSpacing: 2,
                crossAxisSpacing: 2,
              ),
              itemCount: _items.length,
              itemBuilder: (context, i) {
                final m = _items[i];
                return Container(
                  color: Colors.primaries[
                          m.id.hashCode.abs() % Colors.primaries.length]
                      .shade200,
                  child: Center(
                    child: Icon(
                      m.mediaType == 'video'
                          ? LucideIcons.video
                          : LucideIcons.image,
                    ),
                  ),
                );
              },
            ),
    );
  }
}

// ─── Collection Tile ──────────────────────────────────────────────────────────

class _CollectionTile extends StatelessWidget {
  const _CollectionTile({required this.collection, required this.onTap});

  final Collection collection;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return ListTile(
      leading: CircleAvatar(
        child: Icon(
          collection.isCloud ? LucideIcons.cloud : LucideIcons.image,
        ),
      ),
      title: Text(collection.name),
      subtitle: Text(
        '${collection.itemCount} item${collection.itemCount == 1 ? '' : 's'}'
        '${collection.isCloud ? ' . cloud' : ''}',
      ),
      trailing: const Icon(LucideIcons.chevronRight),
      onTap: onTap,
    );
  }
}

// ─── Section Header ───────────────────────────────────────────────────────────

class _SectionHeader extends StatelessWidget {
  const _SectionHeader({
    required this.icon,
    required this.title,
    this.trailing,
  });

  final IconData icon;
  final String title;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
      child: Row(
        children: [
          Icon(icon, size: 18, color: Theme.of(context).colorScheme.primary),
          const SizedBox(width: 8),
          Text(title, style: Theme.of(context).textTheme.titleSmall),
          const Spacer(),
          ?trailing,
        ],
      ),
    );
  }
}

// ─── Placeholder Tile ─────────────────────────────────────────────────────────

class _PlaceholderTile extends StatelessWidget {
  const _PlaceholderTile({
    required this.icon,
    required this.title,
    required this.subtitle,
  });

  final IconData icon;
  final String title;
  final String subtitle;

  @override
  Widget build(BuildContext context) {
    return ListTile(
      enabled: false,
      leading: Icon(icon),
      title: Text(title),
      subtitle: Text(subtitle),
    );
  }
}
