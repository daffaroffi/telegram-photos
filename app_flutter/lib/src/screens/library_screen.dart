import 'package:flutter/material.dart';

import '../../src/rust/api/db.dart' as core;
import '../../src/rust/api/mirror.dart';

/// Tab Library (PRD Part 2 §3.3): Collections, Folders, Favorites, Memories,
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
            icon: const Icon(Icons.create_new_folder_outlined),
            onPressed: _createCollection,
          ),
        ],
      ),
      body: ListView(
        padding: const EdgeInsets.all(8),
        children: [
          _SectionHeader(
            title: 'Collections',
            trailing: Text('${_collections.length}',
                style: Theme.of(context).textTheme.bodySmall),
          ),
          if (_collections.isEmpty)
            Padding(
              padding: const EdgeInsets.all(16),
              child: Text(
                'No collections yet. Tap + to create one.',
                style: Theme.of(context).textTheme.bodyMedium,
              ),
            )
          else
            ..._collections.map((c) => _CollectionTile(
                  collection: c,
                  onTap: () => _openCollection(c),
                )),
          const Divider(height: 24),
          const _SectionHeader(title: 'Coming in P1'),
          ...const [
            _PlaceholderTile(icon: Icons.favorite_outline, title: 'Favorites'),
            _PlaceholderTile(icon: Icons.history, title: 'Memories'),
            _PlaceholderTile(icon: Icons.delete_outline, title: 'Trash'),
            _PlaceholderTile(icon: Icons.folder_outlined, title: 'Device folders'),
          ],
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
          ? const Center(child: Text('Collection is empty.'))
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
                          ? Icons.movie_outlined
                          : Icons.image_outlined,
                    ),
                  ),
                );
              },
            ),
    );
  }
}

class _CollectionTile extends StatelessWidget {
  const _CollectionTile({required this.collection, required this.onTap});

  final Collection collection;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return ListTile(
      leading: CircleAvatar(
        child: Icon(
          collection.isCloud ? Icons.cloud_outlined : Icons.photo_library_outlined,
        ),
      ),
      title: Text(collection.name),
      subtitle: Text(
        '${collection.itemCount} item${collection.itemCount == 1 ? '' : 's'}'
        '${collection.isCloud ? ' · cloud' : ''}',
      ),
      trailing: const Icon(Icons.chevron_right),
      onTap: onTap,
    );
  }
}

class _SectionHeader extends StatelessWidget {
  const _SectionHeader({required this.title, this.trailing});

  final String title;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
      child: Row(
        children: [
          Text(title, style: Theme.of(context).textTheme.titleSmall),
          const Spacer(),
          ?trailing,
        ],
      ),
    );
  }
}

class _PlaceholderTile extends StatelessWidget {
  const _PlaceholderTile({required this.icon, required this.title});

  final IconData icon;
  final String title;

  @override
  Widget build(BuildContext context) {
    return ListTile(
      enabled: false,
      leading: Icon(icon),
      title: Text(title),
      subtitle: const Text('Planned'),
    );
  }
}
