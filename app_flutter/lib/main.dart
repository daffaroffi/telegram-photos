import 'package:flutter/material.dart';
import 'package:path_provider/path_provider.dart';

import 'src/screens/app_shell.dart';
import 'src/screens/onboarding_screen.dart';
import 'src/rust/api/db.dart' as core;
import 'src/rust/api/telegram.dart' as tg;
import 'src/rust/frb_generated.dart';

/// Global Telegram handle -- created once at startup, shared across screens.
tg.TelegramHandle? _telegramHandle;
tg.TelegramHandle get telegramHandle => _telegramHandle!;

/// App data directory (from path_provider). Shared across screens.
String _appDataDir = '';
String get appDataDir => _appDataDir;

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();

  // Show the app shell immediately -- heavy Rust/Telegram init runs async
  // below so the first frame renders without blocking the UI thread.
  runApp(const TelegramPhotosApp());

  // Yield to let the first frame render before doing heavy work.
  await Future<void>.delayed(Duration.zero);

  // Load the Rust native library.
  await RustLib.init();
  await Future<void>.delayed(Duration.zero);

  // Resolve the app data directory.
  _appDataDir = (await getApplicationSupportDirectory()).path;
  await Future<void>.delayed(Duration.zero);

  // Open the SQLite database.
  core.initCore(dbPath: '$_appDataDir/telegram_photos.db');
  await Future<void>.delayed(Duration.zero);

  // Create the Telegram MTProto state handle.
  _telegramHandle = await tg.TelegramHandle.newInstance();
}

class TelegramPhotosApp extends StatefulWidget {
  const TelegramPhotosApp({super.key});

  @override
  State<TelegramPhotosApp> createState() => _TelegramPhotosAppState();
}

class _TelegramPhotosAppState extends State<TelegramPhotosApp> {
  bool _authorized = false;
  bool _checking = true;
  String? _initError;

  @override
  void initState() {
    super.initState();
    _waitForInitAndCheck();
  }

  /// Polls until the global Telegram handle is ready, then checks auth.
  Future<void> _waitForInitAndCheck() async {
    // Wait for main() to finish the heavy Rust/Telegram init.
    while (_telegramHandle == null) {
      await Future<void>.delayed(const Duration(milliseconds: 100));
    }
    if (!mounted) return;
    await _checkAuth();
  }

  Future<void> _checkAuth() async {
    try {
      // Use a 15-second timeout so the UI doesn't hang indefinitely.
      final connected = await tg.checkConnection(
        handle: telegramHandle,
        appDataDir: appDataDir,
      ).timeout(
        const Duration(seconds: 15),
        onTimeout: () => false,
      );
      if (mounted) {
        setState(() {
          _authorized = connected;
          _checking = false;
        });
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          _initError = e.toString();
          _checking = false;
        });
      }
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
      home: _initError != null
          ? Scaffold(
              body: Center(
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    const Icon(Icons.error_outline, size: 48, color: Colors.red),
                    const SizedBox(height: 16),
                    Text('Failed to initialize: $_initError'),
                    const SizedBox(height: 16),
                    FilledButton(
                      onPressed: () {
                        setState(() {
                          _initError = null;
                          _checking = true;
                        });
                        _checkAuth();
                      },
                      child: const Text('Retry'),
                    ),
                  ],
                ),
              ),
            )
          : _checking
              ? const Scaffold(
                  body: Center(child: CircularProgressIndicator()),
                )
              : _authorized
                  ? const AppShell()
                  : OnboardingScreen(onAuthenticated: _checkAuth),
    );
  }
}
