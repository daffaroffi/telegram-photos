import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../platform/media_scan.dart';
import '../rust/api/db.dart' as core;
import '../rust/api/mirror.dart';
import '../rust/api/telegram.dart' as tg;
import '../../main.dart' show telegramHandle;
import '../../main.dart' show appDataDir;
import 'app_shell.dart';
import 'import_screen.dart';

/// Zero-setup onboarding (PRD Part 2 S4.1).
///
/// QR login is the PRIMARY flow (2 taps, no typing).
/// Phone OTP is the fallback.
/// After login, auto-scan gallery + auto-backup.
class OnboardingScreen extends StatefulWidget {
  final VoidCallback onAuthenticated;
  const OnboardingScreen({super.key, required this.onAuthenticated});

  @override
  State<OnboardingScreen> createState() => _OnboardingScreenState();
}

class _OnboardingScreenState extends State<OnboardingScreen> {
  int _step = 0; // 0=welcome, 1=method, 2=phone, 3=code, 4=setup, 5=done

  // Phone login state
  final _phoneCtrl = TextEditingController();
  final _codeCtrl = TextEditingController();
  bool _loading = false;
  String? _error;
  bool _needPassword = false;
  final _passwordCtrl = TextEditingController();

  @override
  void dispose() {
    _phoneCtrl.dispose();
    _codeCtrl.dispose();
    _passwordCtrl.dispose();
    super.dispose();
  }

  // -- QR Login Flow (PRD primary) --

  Future<void> _startQRLogin() async {
    setState(() {
      _loading = true;
      _error = null;
    });

    try {
      final apiId = core.getSettings().telegramApiId;
      final apiHash = core.getSettings().telegramApiHash;

      if (apiId == 0 || apiHash.isEmpty) {
        // Need API credentials for QR login
        setState(() {
          _loading = false;
          _step = 6; // API credentials input
        });
        return;
      }

      final qrUrl = await tg.authQrLogin(
        handle: telegramHandle,
        apiId: apiId,
        apiHash: apiHash,
        appDataDir: appDataDir,
      );

      if (qrUrl == '__authorized__') {
        // Already authorized
        _onLoginSuccess();
        return;
      }

      // Show QR code
      if (mounted) {
        setState(() {
          _loading = false;
          _step = 7; // QR code display
        });
        // Start polling
        _pollQRLogin();
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          _loading = false;
          _error = e.toString();
        });
      }
    }
  }

  Future<void> _pollQRLogin() async {
    while (mounted && _step == 7) {
      await Future.delayed(const Duration(seconds: 2));
      try {
        final result = await tg.authQrPoll(handle: telegramHandle);
        if (result.status == 'authorized') {
          _onLoginSuccess();
          return;
        }
      } catch (e) {
        // Keep polling
      }
    }
  }

  // -- Phone OTP Flow (fallback) --

  Future<void> _requestCode() async {
    setState(() {
      _loading = true;
      _error = null;
    });

    try {
      final phone = _phoneCtrl.text.trim();
      if (phone.isEmpty) {
        setState(() {
          _loading = false;
          _error = 'Enter your phone number',
        });
        return;
      }

      final apiId = core.getSettings().telegramApiId;
      final apiHash = core.getSettings().telegramApiHash;

      if (apiId == 0 || apiHash.isEmpty) {
        setState(() {
          _loading = false;
          _step = 6; // API credentials input
        });
        return;
      }

      await tg.authRequestCode(
        handle: telegramHandle,
        phone: phone,
        apiId: apiId,
        apiHash: apiHash,
        appDataDir: appDataDir,
      );

      if (mounted) {
        setState(() {
          _loading = false;
          _step = 3; // Code input
        });
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          _loading = false;
          _error = e.toString();
        });
      }
    }
  }

  Future<void> _signIn() async {
    setState(() {
      _loading = true;
      _error = null;
    });

    try {
      final code = _codeCtrl.text.trim();
      final result = await tg.authSignIn(
        handle: telegramHandle,
        code: code,
      );

      if (result.status == 'authorized') {
        _onLoginSuccess();
      } else if (result.status == 'password_required') {
        if (mounted) {
          setState(() {
            _loading = false;
            _needPassword = true;
          });
        }
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          _loading = false;
          _error = e.toString();
        });
      }
    }
  }

  Future<void> _checkPassword() async {
    setState(() {
      _loading = true;
      _error = null;
    });

    try {
      await tg.authCheckPassword(
        handle: telegramHandle,
        password: _passwordCtrl.text,
      );
      _onLoginSuccess();
    } catch (e) {
      if (mounted) {
        setState(() {
          _loading = false;
          _error = e.toString();
        });
      }
    }
  }

  void _onLoginSuccess() {
    widget.onAuthenticated();
  }

  // -- Build --

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;

    return Scaffold(
      body: SafeArea(
        child: IndexedStack(
          index: _step,
          children: [
            // Step 0: Welcome
            _buildWelcome(context, cs),
            // Step 1: Method selection
            _buildMethodSelection(context, cs),
            // Step 2: Phone input
            _buildPhoneInput(context, cs),
            // Step 3: Code input
            _buildCodeInput(context, cs),
            // Step 4: Setup (auto-scan)
            _buildSetup(context, cs),
            // Step 5: Done
            _buildDone(context, cs),
            // Step 6: API credentials (only if needed)
            _buildApiCredentials(context, cs),
            // Step 7: QR code display
            _buildQRCode(context, cs),
          ],
        ),
      ),
    );
  }

  Widget _buildWelcome(BuildContext context, ColorScheme cs) {
    return Padding(
      padding: const EdgeInsets.all(32),
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(LucideIcons.shield, size: 80, color: cs.primary),
          const SizedBox(height: 32),
          Text(
            'Telegram Photos',
            style: Theme.of(context).textTheme.headlineLarge?.copyWith(
                  fontWeight: FontWeight.w700,
                ),
          ),
          const SizedBox(height: 12),
          Text(
            'Back up your photos privately.\nEnd-to-end encrypted.',
            style: Theme.of(context).textTheme.bodyLarge?.copyWith(
                  color: cs.onSurfaceVariant,
                ),
            textAlign: TextAlign.center,
          ),
          const SizedBox(height: 48),
          SizedBox(
            width: double.infinity,
            child: FilledButton(
              onPressed: () => setState(() => _step = 1),
              style: FilledButton.styleFrom(
                minimumSize: const Size(0, 52),
              ),
              child: const Text('Get started'),
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildMethodSelection(BuildContext context, ColorScheme cs) {
    return Padding(
      padding: const EdgeInsets.all(32),
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Text(
            'How would you like to login?',
            style: Theme.of(context).textTheme.headlineSmall,
          ),
          const SizedBox(height: 8),
          Text(
            'QR code is fastest — no typing needed.',
            style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                  color: cs.onSurfaceVariant,
                ),
          ),
          const SizedBox(height: 32),

          // QR code option (primary)
          SizedBox(
            width: double.infinity,
            child: FilledButton.icon(
              onPressed: _loading ? null : _startQRLogin,
              icon: const Icon(LucideIcons.qrCode),
              label: const Text('Scan QR code'),
              style: FilledButton.styleFrom(
                minimumSize: const Size(0, 52),
              ),
            ),
          ),
          const SizedBox(height: 12),

          // Phone number option (secondary)
          SizedBox(
            width: double.infinity,
            child: OutlinedButton.icon(
              onPressed: () => setState(() => _step = 2),
              icon: const Icon(LucideIcons.smartphone),
              label: const Text('Use phone number'),
              style: OutlinedButton.styleFrom(
                minimumSize: const Size(0, 52),
              ),
            ),
          ),

          if (_error != null) ...[
            const SizedBox(height: 16),
            Container(
              padding: const EdgeInsets.all(12),
              decoration: BoxDecoration(
                color: cs.errorContainer,
                borderRadius: BorderRadius.circular(8),
              ),
              child: Row(
                children: [
                  Icon(LucideIcons.circleAlert,
                      color: cs.onErrorContainer, size: 18),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      _error!,
                      style: TextStyle(color: cs.onErrorContainer),
                    ),
                  ),
                ],
              ),
            ),
          ],
        ],
      ),
    );
  }

  Widget _buildPhoneInput(BuildContext context, ColorScheme cs) {
    return Padding(
      padding: const EdgeInsets.all(32),
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Text(
            'Enter your phone number',
            style: Theme.of(context).textTheme.headlineSmall,
          ),
          const SizedBox(height: 8),
          Text(
            'You will receive a verification code via Telegram.',
            style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                  color: cs.onSurfaceVariant,
                ),
          ),
          const SizedBox(height: 32),
          TextField(
            controller: _phoneCtrl,
            keyboardType: TextInputType.phone,
            autofocus: true,
            decoration: const InputDecoration(
              hintText: '+6281234567890',
              border: OutlineInputBorder(),
              prefixIcon: Icon(LucideIcons.smartphone),
            ),
          ),
          const SizedBox(height: 16),
          if (_error != null) ...[
            Container(
              padding: const EdgeInsets.all(12),
              decoration: BoxDecoration(
                color: cs.errorContainer,
                borderRadius: BorderRadius.circular(8),
              ),
              child: Text(
                _error!,
                style: TextStyle(color: cs.onErrorContainer),
              ),
            ),
            const SizedBox(height: 16),
          ],
          SizedBox(
            width: double.infinity,
            child: FilledButton(
              onPressed: _loading ? null : _requestCode,
              style: FilledButton.styleFrom(
                minimumSize: const Size(0, 52),
              ),
              child: _loading
                  ? const SizedBox(
                      width: 20,
                      height: 20,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Text('Send code'),
            ),
          ),
          const SizedBox(height: 12),
          TextButton(
            onPressed: () => setState(() {
              _step = 1;
              _error = null;
            }),
            child: const Text('Back'),
          ),
        ],
      ),
    );
  }

  Widget _buildCodeInput(BuildContext context, ColorScheme cs) {
    return Padding(
      padding: const EdgeInsets.all(32),
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Text(
            _needPassword ? 'Enter 2FA password' : 'Enter verification code',
            style: Theme.of(context).textTheme.headlineSmall,
          ),
          const SizedBox(height: 8),
          Text(
            _needPassword
                ? 'Your account has two-factor authentication enabled.'
                : 'Check your Telegram app for the code.',
            style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                  color: cs.onSurfaceVariant,
                ),
          ),
          const SizedBox(height: 32),
          TextField(
            controller: _needPassword ? _passwordCtrl : _codeCtrl,
            obscureText: _needPassword,
            keyboardType: _needPassword ? TextInputType.text : TextInputType.number,
            autofocus: true,
            decoration: InputDecoration(
              hintText: _needPassword ? 'Password' : '12345',
              border: const OutlineInputBorder(),
              prefixIcon: Icon(
                _needPassword ? LucideIcons.lock : LucideIcons.hash,
              ),
            ),
          ),
          const SizedBox(height: 16),
          if (_error != null) ...[
            Container(
              padding: const EdgeInsets.all(12),
              decoration: BoxDecoration(
                color: cs.errorContainer,
                borderRadius: BorderRadius.circular(8),
              ),
              child: Text(
                _error!,
                style: TextStyle(color: cs.onErrorContainer),
              ),
            ),
            const SizedBox(height: 16),
          ],
          SizedBox(
            width: double.infinity,
            child: FilledButton(
              onPressed: _loading
                  ? null
                  : _needPassword ? _checkPassword : _signIn,
              style: FilledButton.styleFrom(
                minimumSize: const Size(0, 52),
              ),
              child: _loading
                  ? const SizedBox(
                      width: 20,
                      height: 20,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Text('Continue'),
            ),
          ),
          const SizedBox(height: 12),
          TextButton(
            onPressed: () => setState(() {
              _step = _needPassword ? 3 : 2;
              _error = null;
              _needPassword = false;
            }),
            child: const Text('Back'),
          ),
        ],
      ),
    );
  }

  Widget _buildSetup(BuildContext context, ColorScheme cs) {
    return Padding(
      padding: const EdgeInsets.all(32),
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          const CircularProgressIndicator(),
          const SizedBox(height: 24),
          Text(
            'Setting up your vault...',
            style: Theme.of(context).textTheme.headlineSmall,
          ),
          const SizedBox(height: 8),
          Text(
            'Scanning your gallery and creating your private vault channel.',
            style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                  color: cs.onSurfaceVariant,
                ),
            textAlign: TextAlign.center,
          ),
        ],
      ),
    );
  }

  Widget _buildDone(BuildContext context, ColorScheme cs) {
    return Padding(
      padding: const EdgeInsets.all(32),
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(LucideIcons.circleCheck, size: 72, color: Colors.green),
          const SizedBox(height: 24),
          Text(
            'You are all set!',
            style: Theme.of(context).textTheme.headlineSmall,
          ),
          const SizedBox(height: 8),
          Text(
            'Your photos are being backed up securely to your private Telegram vault.',
            style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                  color: cs.onSurfaceVariant,
                ),
            textAlign: TextAlign.center,
          ),
          const SizedBox(height: 32),
          SizedBox(
            width: double.infinity,
            child: FilledButton(
              onPressed: () => Navigator.pushReplacement(
                context,
                MaterialPageRoute(builder: (_) => const AppShell()),
              ),
              style: FilledButton.styleFrom(
                minimumSize: const Size(0, 52),
              ),
              child: const Text('View my photos'),
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildApiCredentials(BuildContext context, ColorScheme cs) {
    final apiIdCtrl = TextEditingController();
    final apiHashCtrl = TextEditingController();

    return Padding(
      padding: const EdgeInsets.all(32),
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Text(
            'Telegram API credentials',
            style: Theme.of(context).textTheme.headlineSmall,
          ),
          const SizedBox(height: 8),
          Text(
            'Get these from my.telegram.org',
            style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                  color: cs.onSurfaceVariant,
                ),
          ),
          const SizedBox(height: 32),
          TextField(
            controller: apiIdCtrl,
            keyboardType: TextInputType.number,
            decoration: const InputDecoration(
              hintText: 'API ID (number)',
              border: OutlineInputBorder(),
            ),
          ),
          const SizedBox(height: 12),
          TextField(
            controller: apiHashCtrl,
            decoration: const InputDecoration(
              hintText: 'API Hash (string)',
              border: OutlineInputBorder(),
            ),
          ),
          const SizedBox(height: 16),
          SizedBox(
            width: double.infinity,
            child: FilledButton(
              onPressed: () {
                // Save and continue
                final apiId = int.tryParse(apiIdCtrl.text) ?? 0;
                final apiHash = apiHashCtrl.text.trim();
                if (apiId > 0 && apiHash.isNotEmpty) {
                  final settings = core.getSettings();
                  core.saveSettings(
                    settings: AppSettings(
                      autoBackupEnabled: settings.autoBackupEnabled,
                      backupOverWifiOnly: settings.backupOverWifiOnly,
                      backupWhileChargingOnly: settings.backupWhileChargingOnly,
                      uploadOriginalQuality: settings.uploadOriginalQuality,
                      folderBackupSettings: settings.folderBackupSettings,
                      clientEncryptionEnabled: settings.clientEncryptionEnabled,
                      vaultPassphraseSet: settings.vaultPassphraseSet,
                      gridColumnCount: settings.gridColumnCount,
                      theme: settings.theme,
                      telegramApiId: apiId,
                      telegramApiHash: apiHash,
                      googleClientId: settings.googleClientId,
                      googleClientSecret: settings.googleClientSecret,
                    ),
                  );
                  setState(() => _step = 1);
                }
              },
              style: FilledButton.styleFrom(
                minimumSize: const Size(0, 52),
              ),
              child: const Text('Save & continue'),
            ),
          ),
          const SizedBox(height: 12),
          TextButton(
            onPressed: () => setState(() => _step = 1),
            child: const Text('Back'),
          ),
        ],
      ),
    );
  }

  Widget _buildQRCode(BuildContext context, ColorScheme cs) {
    return Padding(
      padding: const EdgeInsets.all(32),
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Text(
            'Scan QR code',
            style: Theme.of(context).textTheme.headlineSmall,
          ),
          const SizedBox(height: 8),
          Text(
            'Open Telegram > Settings > Devices > Scan QR Code',
            style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                  color: cs.onSurfaceVariant,
                ),
            textAlign: TextAlign.center,
          ),
          const SizedBox(height: 32),
          Container(
            width: 200,
            height: 200,
            decoration: BoxDecoration(
              color: cs.surfaceContainerHighest,
              borderRadius: BorderRadius.circular(12),
            ),
            child: const Center(
              child: CircularProgressIndicator(),
            ),
          ),
          const SizedBox(height: 24),
          Text(
            'Waiting for scan...',
            style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                  color: cs.onSurfaceVariant,
                ),
          ),
          const SizedBox(height: 16),
          TextButton(
            // TODO(call-rust): when the FRB binding for auth_qr_cancel
            // is generated, call it here so the Rust-side login_token
            // is dropped on user back-out. For now the Dart poll loop
            // exits on _step != 7 and the token is cleared on next login.
            onPressed: () => setState(() {
              _step = 1;
              _error = null;
            }),
            child: const Text('Cancel'),
          ),
        ],
      ),
    );
  }
}
