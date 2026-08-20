import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../rust/api/db.dart' as core;
import '../rust/api/mirror.dart';
import 'collections_screen.dart';
import 'import_screen.dart';

/// Tab Library (PRD Part 2 S3.3): collections, folders, favorites, memories, trash.
class LibraryScreen extends StatefulWidget {
  const LibraryScreen({super.key});

  @override
  State<LibraryScreen> createState() => _LibraryScreenState();
}

class _LibraryScreenState extends State<LibraryScreen> {
  late List<Collection> _collections;

  @override
  void initState() {
    super.initState();
    _collections = core.listCollections();
  }

  void _refresh() {
    setState(() {
      _collections = core.listCollections();
    });
  }

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;

    return Scaffold(
      appBar: AppBar(
        title: const Text('Library'),
        actions: [
          IconButton(
            icon: const Icon(LucideIcons.plus),
            tooltip: 'Import photos',
            onPressed: () async {
              await Navigator.of(context).push(
                MaterialPageRoute(builder: (_) => const ImportScreen()),
              );
              _refresh();
            },
          ),
        ],
      ),
      body: ListView(
        padding: const EdgeInsets.only(bottom: 32),
        children: [
          // -- Collections Section --
          _SectionHeader(
            icon: LucideIcons.folderOpen,
            title: 'Collections',
            trailing: TextButton(
              onPressed: () async {
                await Navigator.of(context).push(
                  MaterialPageRoute(builder: (_) => const CollectionsScreen()),
                );
                _refresh();
              },
              child: const Text('See all'),
            ),
          ),
          if (_collections.isEmpty)
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 24),
              child: Center(
                child: Column(
                  children: [
                    Icon(LucideIcons.folderPlus, size: 48, color: cs.outline),
                    const SizedBox(height: 12),
                    Text(
                      'No collections yet',
                      style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                            color: cs.onSurfaceVariant,
                          ),
                    ),
                    const SizedBox(height: 8),
                    Text(
                      'Create albums to organize your photos',
                      style: Theme.of(context).textTheme.bodySmall?.copyWith(
                            color: cs.onSurfaceVariant,
                          ),
                    ),
                  ],
                ),
              ),
            )
          else
            ..._collections.take(4).map(
                  (col) => ListTile(
                    leading: Icon(LucideIcons.folder, color: cs.primary),
                    title: Text(col.name),
                    trailing: const Icon(LucideIcons.chevronRight, size: 18),
                    onTap: () {
                      // TODO: Open collection detail
                    },
                  ),
                ),

          const Divider(),

          // -- Favorites Section --
          _SectionHeader(icon: LucideIcons.star, title: 'Favorites'),
          ListTile(
            leading: Icon(LucideIcons.star, color: Colors.amber),
            title: const Text('Favorites'),
            subtitle: const Text('Photos you starred'),
            trailing: const Icon(LucideIcons.chevronRight, size: 18),
            onTap: () {
              // TODO: Open favorites
              ScaffoldMessenger.of(context).showSnackBar(
                const SnackBar(content: Text('Favorites coming in v0.8')),
              );
            },
          ),

          const Divider(),

          // -- Memories Section --
          _SectionHeader(icon: LucideIcons.clock, title: 'Memories'),
          ListTile(
            leading: Icon(LucideIcons.history, color: cs.primary),
            title: const Text('On this day'),
            subtitle: const Text('Photos from previous years'),
            trailing: const Icon(LucideIcons.chevronRight, size: 18),
            onTap: () {
              // TODO: Open memories
              ScaffoldMessenger.of(context).showSnackBar(
                const SnackBar(content: Text('Memories coming in v0.8')),
              );
            },
          ),

          const Divider(),

          // -- Device Folders Section --
          _SectionHeader(icon: LucideIcons.smartphone, title: 'Device folders'),
          ListTile(
            leading: Icon(LucideIcons.camera, color: cs.onSurfaceVariant),
            title: const Text('Camera'),
            trailing: const Icon(LucideIcons.chevronRight, size: 18),
            onTap: () {
              // TODO: Open camera folder
            },
          ),
          ListTile(
            leading: Icon(LucideIcons.messageCircle, color: cs.onSurfaceVariant),
            title: const Text('WhatsApp'),
            trailing: const Icon(LucideIcons.chevronRight, size: 18),
            onTap: () {
              // TODO: Open WhatsApp folder
            },
          ),
          ListTile(
            leading: Icon(LucideIcons.download, color: cs.onSurfaceVariant),
            title: const Text('Downloads'),
            trailing: const Icon(LucideIcons.chevronRight, size: 18),
            onTap: () {
              // TODO: Open downloads folder
            },
          ),

          const Divider(),

          // -- Trash Section --
          _SectionHeader(icon: LucideIcons.trash2, title: 'Trash'),
          ListTile(
            leading: Icon(LucideIcons.trash2, color: cs.error),
            title: const Text('Trash'),
            subtitle: const Text('Photos kept for 30 days'),
            trailing: const Icon(LucideIcons.chevronRight, size: 18),
            onTap: () {
              // TODO: Open trash
              ScaffoldMessenger.of(context).showSnackBar(
                const SnackBar(content: Text('Trash coming in v0.8')),
              );
            },
          ),
        ],
      ),
    );
  }
}

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
    final cs = Theme.of(context).colorScheme;
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 16, 16, 4),
      child: Row(
        children: [
          Icon(icon, size: 16, color: cs.primary),
          const SizedBox(width: 8),
          Text(
            title,
            style: Theme.of(context).textTheme.labelLarge?.copyWith(
                  color: cs.primary,
                  fontWeight: FontWeight.w600,
                ),
          ),
          const Spacer(),
          if (trailing != null) trailing!,
        ],
      ),
    );
  }
}
