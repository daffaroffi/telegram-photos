import 'package:flutter/material.dart';
import 'package:path_provider/path_provider.dart';
import 'package:telegram_photos/src/rust/api/db.dart' as core;
import 'package:telegram_photos/src/rust/frb_generated.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await RustLib.init();

  // Open the real SQLite database through the Rust core.
  final dir = await getApplicationSupportDirectory();
  core.initCore(dbPath: '${dir.path}/telegram_photos.db');

  runApp(const TelegramPhotosApp());
}

class TelegramPhotosApp extends StatelessWidget {
  const TelegramPhotosApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Telegram Photos',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: const Color(0xFF2AABEE)),
        useMaterial3: true,
      ),
      home: const CoreProbeScreen(),
    );
  }
}

/// Boot probe: proves the Rust core answers real DB queries through FRB.
/// Replaced by the 4-tab Photos/Search/Library/Settings shell (PRD Part 2 §3).
class CoreProbeScreen extends StatelessWidget {
  const CoreProbeScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final count = core.countMedia();
    final summary = core.uploadsSummary();
    final collections = core.listCollections();

    return Scaffold(
      appBar: AppBar(title: const Text('Telegram Photos — core probe')),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          Text('Media items: $count', style: _style(context)),
          Text(
            'Uploads — queued: ${summary.queuedCount} · uploading: '
            '${summary.uploadingCount} · failed: ${summary.failedCount} · '
            'backed up: ${summary.backedUpCount}',
            style: _style(context),
          ),
          Text('Collections: ${collections.length}', style: _style(context)),
          const SizedBox(height: 16),
          const Text('Rust core (SQLite) is answering through flutter_rust_bridge.'),
        ],
      ),
    );
  }

  TextStyle _style(BuildContext context) =>
      Theme.of(context).textTheme.bodyLarge!;
}
