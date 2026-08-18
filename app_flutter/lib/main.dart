import 'package:flutter/material.dart';
import 'package:path_provider/path_provider.dart';

import 'src/screens/app_shell.dart';
import 'src/screens/onboarding_screen.dart';
import 'src/rust/api/db.dart' as core;
import 'src/rust/api/telegram.dart' as tg;
import 'src/rust/frb_generated.dart';

/// Global Telegram handle — created once at startup, shared across screens.
late final tg.TelegramHandle telegramHandle;

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await RustLib.init();

  // Open the real SQLite database through the Rust core.
  final dir = await getApplicationSupportDirectory();
  core.initCore(dbPath: '${dir.path}/telegram_photos.db');

  // Create the Telegram state handle (lives for the app lifetime).
  telegramHandle = await tg.TelegramHandle.newInstance();

  runApp(const TelegramPhotosApp());
}

class TelegramPhotosApp extends StatefulWidget {
  const TelegramPhotosApp({super.key});

  @override
  State<TelegramPhotosApp> createState() => _TelegramPhotosAppState();
}

class _TelegramPhotosAppState extends State<TelegramPhotosApp> {
  bool _authorized = false;
  bool _checking = true;

  @override
  void initState() {
    super.initState();
    _checkAuth();
  }

  Future<void> _checkAuth() async {
    final connected = await tg.checkConnection(
      handle: telegramHandle,
      appDataDir: '',
    );
    if (mounted) {
      setState(() {
        _authorized = connected;
        _checking = false;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Telegram Photos',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: const Color(0xFF2AABEE)),
        useMaterial3: true,
      ),
      home: _checking
          ? const Scaffold(
              body: Center(child: CircularProgressIndicator()),
            )
          : _authorized
              ? const AppShell()
              : const OnboardingScreen(),
    );
  }
}
