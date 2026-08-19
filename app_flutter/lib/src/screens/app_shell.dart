import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'library_screen.dart';
import 'photos_screen.dart';
import 'search_screen.dart';
import 'settings_screen.dart';

/// 4-tab shell (PRD Part 2 S3): Photos / Search / Library / Settings.
/// Uses Lucide icons for consistent, modern visual language.
class AppShell extends StatefulWidget {
  const AppShell({super.key});

  @override
  State<AppShell> createState() => _AppShellState();
}

class _AppShellState extends State<AppShell> {
  int _index = 0;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: IndexedStack(
        index: _index,
        children: const [
          PhotosScreen(),
          SearchScreen(),
          LibraryScreen(),
          SettingsScreen(),
        ],
      ),
      bottomNavigationBar: NavigationBar(
        selectedIndex: _index,
        // >=48dp hit target per PRD Part 2 S3.
        destinations: const [
          NavigationDestination(
            icon: Icon(LucideIcons.image),
            selectedIcon: Icon(LucideIcons.images),
            label: 'Photos',
          ),
          NavigationDestination(
            icon: Icon(LucideIcons.search),
            selectedIcon: Icon(LucideIcons.search),
            label: 'Search',
          ),
          NavigationDestination(
            icon: Icon(LucideIcons.folder),
            selectedIcon: Icon(LucideIcons.folderOpen),
            label: 'Library',
          ),
          NavigationDestination(
            icon: Icon(LucideIcons.settings2),
            selectedIcon: Icon(LucideIcons.settings2),
            label: 'Settings',
          ),
        ],
        onDestinationSelected: (i) => setState(() => _index = i),
      ),
    );
  }
}
