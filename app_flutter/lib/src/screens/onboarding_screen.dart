import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:qr_flutter/qr_flutter.dart';

import '../rust/api/telegram.dart' as tg;

/// Global Telegram handle from main.dart.
import '../../main.dart' show telegramHandle;

/// Onboarding screen (PRD Part 2 S2): Telegram login via QR or phone OTP.
///
/// Uses progressive disclosure: API credentials first, then method picker,
/// then phone/QR flow. Simplifies Hick's Law by reducing choices per step.
class OnboardingScreen extends StatefulWidget {
  final VoidCallback onAuthenticated;

  const OnboardingScreen({super.key, required this.onAuthenticated});

  @override
  State<OnboardingScreen> createState() => _OnboardingScreenState();
}

enum _Step { credentials, method, phoneLogin, codeEntry, passwordEntry }

class _OnboardingScreenState extends State<OnboardingScreen> {
  // API credentials
  final _apiIdCtrl = TextEditingController();
  final _apiHashCtrl = TextEditingController();

  // Login state
  _Step _step = _Step.credentials;
  bool _loading = false;
  String? _error;

  // Phone login
  final _phoneCtrl = TextEditingController();
  final _codeCtrl = TextEditingController();
  final _passwordCtrl = TextEditingController();

  // QR login
  String? _qrUrl;
  Timer? _pollTimer;

  @override
  void dispose() {
    _apiIdCtrl.dispose();
    _apiHashCtrl.dispose();
    _phoneCtrl.dispose();
    _codeCtrl.dispose();
    _passwordCtrl.dispose();
    _pollTimer?.cancel();
    super.dispose();
  }

  bool get _validApiCreds =>
      _apiIdCtrl.text.trim().isNotEmpty &&
      _apiHashCtrl.text.trim().isNotEmpty &&
      int.tryParse(_apiIdCtrl.text.trim()) != null;

  Future<String> _appDataDir() async {
    final dir = await MethodChannel('com.telegramphotos.app/media')
        .invokeMethod<String>('getAppDataDir');
    return dir ?? '';
  }

  // ─── Actions ────────────────────────────────────────────────────────────

  Future<void> _startQRLogin() async {
    if (!_validApiCreds) return;
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final apiId = int.parse(_apiIdCtrl.text.trim());
      final appDataDir = await _appDataDir();
      final url = await tg.authQrLogin(
        handle: telegramHandle,
        apiId: apiId,
        apiHash: _apiHashCtrl.text.trim(),
        appDataDir: appDataDir,
      );
      if (url == '__authorized__') {
        widget.onAuthenticated();
        return;
      }
      setState(() {
        _qrUrl = url;
        _loading = false;
        _step = _Step.method; // Show QR + phone choice
      });
      // Start polling.
      _pollTimer = Timer.periodic(const Duration(seconds: 3), (_) async {
        try {
          final result = await tg.authQrPoll(handle: telegramHandle);
          if (result.status == 'authorized') {
            _pollTimer?.cancel();
            widget.onAuthenticated();
          }
        } catch (_) {}
      });
    } catch (e) {
      setState(() {
        _error = _mapError(e);
        _loading = false;
      });
    }
  }

  Future<void> _startPhoneLogin() async {
    if (!_validApiCreds) return;
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final apiId = int.parse(_apiIdCtrl.text.trim());
      final appDataDir = await _appDataDir();
      await tg.authRequestCode(
        handle: telegramHandle,
        phone: _phoneCtrl.text,
        apiId: apiId,
        apiHash: _apiHashCtrl.text.trim(),
        appDataDir: appDataDir,
      );
      setState(() {
        _loading = false;
        _step = _Step.codeEntry;
      });
    } catch (e) {
      setState(() {
        _error = _mapError(e);
        _loading = false;
      });
    }
  }

  Future<void> _submitCode() async {
    if (_codeCtrl.text.trim().isEmpty) return;
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final result = await tg.authSignIn(
        handle: telegramHandle,
        code: _codeCtrl.text.trim(),
      );
      if (result.status == 'authorized') {
        widget.onAuthenticated();
      } else if (result.status == 'password_required') {
        setState(() {
          _loading = false;
          _step = _Step.passwordEntry;
        });
      }
    } catch (e) {
      setState(() {
        _error = _mapError(e);
        _loading = false;
      });
    }
  }

  Future<void> _submitPassword() async {
    if (_passwordCtrl.text.isEmpty) return;
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final result = await tg.authCheckPassword(
        handle: telegramHandle,
        password: _passwordCtrl.text,
      );
      if (result.status == 'authorized') {
        widget.onAuthenticated();
      }
    } catch (e) {
      setState(() {
        _error = _mapError(e);
        _loading = false;
      });
    }
  }

  String _mapError(dynamic e) {
    final s = e.toString();
    if (s.contains('AUTH_RESTART')) return 'Session expired. Please try again.';
    if (s.contains('API_ID_INVALID')) return 'Invalid API ID or API Hash.';
    if (s.contains('PHONE_NUMBER_INVALID')) return 'Invalid phone number.';
    if (s.contains('PHONE_CODE_INVALID')) return 'Invalid verification code.';
    if (s.contains('FLOOD_WAIT')) return 'Too many attempts. Please wait.';
    return s;
  }

  // ─── Build ──────────────────────────────────────────────────────────────

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: SafeArea(
        child: SingleChildScrollView(
          padding: const EdgeInsets.symmetric(horizontal: 24),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const SizedBox(height: 48),
              // Header
              Text(
                'Telegram Photos',
                style: Theme.of(context).textTheme.headlineMedium?.copyWith(
                      fontWeight: FontWeight.w700,
                    ),
              ),
              const SizedBox(height: 8),
              Text(
                'Back up your photos to Telegram',
                style: Theme.of(context).textTheme.titleMedium?.copyWith(
                      color: Theme.of(context).colorScheme.onSurfaceVariant,
                    ),
              ),
              const SizedBox(height: 8),
              Text(
                'Connect your Telegram account to start backing up photos to your private vault with zero-knowledge encryption.',
                style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                      color: Theme.of(context).colorScheme.onSurfaceVariant,
                    ),
              ),
              const SizedBox(height: 48),

              // Step indicator
              _StepIndicator(current: _step),
              const SizedBox(height: 32),

              // Content based on step
              if (_step == _Step.credentials) _buildCredentialsStep(),
              if (_step == _Step.method) _buildMethodStep(),
              if (_step == _Step.phoneLogin) _buildPhoneStep(),
              if (_step == _Step.codeEntry) _buildCodeStep(),
              if (_step == _Step.passwordEntry) _buildPasswordStep(),

              // Error
              if (_error != null) ...[
                const SizedBox(height: 16),
                Container(
                  padding: const EdgeInsets.all(12),
                  decoration: BoxDecoration(
                    color: Theme.of(context).colorScheme.errorContainer,
                    borderRadius: BorderRadius.circular(8),
                  ),
                  child: Row(
                    children: [
                      Icon(
                        LucideIcons.circleAlert,
                        size: 18,
                        color: Theme.of(context).colorScheme.onErrorContainer,
                      ),
                      const SizedBox(width: 8),
                      Expanded(
                        child: Text(
                          _error!,
                          style: TextStyle(
                            color: Theme.of(context)
                                .colorScheme
                                .onErrorContainer,
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
              ],
              const SizedBox(height: 32),
            ],
          ),
        ),
      ),
    );
  }

  // ─── Step Builders ──────────────────────────────────────────────────────

  Widget _buildCredentialsStep() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          'Step 1: API Credentials',
          style: Theme.of(context).textTheme.titleSmall?.copyWith(
                color: Theme.of(context).colorScheme.primary,
              ),
        ),
        const SizedBox(height: 4),
        Text(
          'Get these from my.telegram.org/apps',
          style: Theme.of(context).textTheme.bodySmall?.copyWith(
                color: Theme.of(context).colorScheme.onSurfaceVariant,
              ),
        ),
        const SizedBox(height: 16),
        TextField(
          controller: _apiIdCtrl,
          keyboardType: TextInputType.number,
          decoration: const InputDecoration(
            labelText: 'API ID',
            hintText: '12345678',
            prefixIcon: Icon(LucideIcons.key),
            border: OutlineInputBorder(),
          ),
          onChanged: (_) => setState(() {}),
        ),
        const SizedBox(height: 12),
        TextField(
          controller: _apiHashCtrl,
          decoration: const InputDecoration(
            labelText: 'API Hash',
            hintText: 'a1b2c3d4e5f6...',
            prefixIcon: Icon(LucideIcons.hash),
            border: OutlineInputBorder(),
          ),
          onChanged: (_) => setState(() {}),
        ),
        const SizedBox(height: 24),
        FilledButton(
          onPressed: _validApiCreds
              ? () => setState(() => _step = _Step.method)
              : null,
          style: FilledButton.styleFrom(
            minimumSize: const Size(0, 48),
          ),
          child: const Text('Continue'),
        ),
      ],
    );
  }

  Widget _buildMethodStep() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          'Step 2: Login method',
          style: Theme.of(context).textTheme.titleSmall?.copyWith(
                color: Theme.of(context).colorScheme.primary,
              ),
        ),
        const SizedBox(height: 16),
        // QR Code option
        if (_qrUrl != null) ...[
          Card(
            child: Padding(
              padding: const EdgeInsets.all(16),
              child: Column(
                children: [
                  QrImageView(
                    data: _qrUrl!,
                    version: QrVersions.auto,
                    size: 180,
                  ),
                  const SizedBox(height: 12),
                  Text(
                    'Scan with Telegram app',
                    style: Theme.of(context).textTheme.bodyMedium,
                  ),
                ],
              ),
            ),
          ),
          const SizedBox(height: 16),
          const Divider(),
          const SizedBox(height: 16),
        ],
        // Phone number option
        OutlinedButton.icon(
          onPressed: () => setState(() => _step = _Step.phoneLogin),
          icon: const Icon(LucideIcons.phone),
          label: const Text('Login with phone number'),
          style: OutlinedButton.styleFrom(
            minimumSize: const Size(0, 48),
          ),
        ),
        const SizedBox(height: 12),
        OutlinedButton.icon(
          onPressed: _startQRLogin,
          icon: _loading
              ? const SizedBox(
                  width: 18,
                  height: 18,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : const Icon(LucideIcons.scan),
          label: Text(_loading ? 'Generating QR...' : 'Refresh QR code'),
          style: OutlinedButton.styleFrom(
            minimumSize: const Size(0, 48),
          ),
        ),
      ],
    );
  }

  Widget _buildPhoneStep() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        // Back button
        Align(
          alignment: Alignment.centerLeft,
          child: IconButton(
            icon: const Icon(LucideIcons.arrowLeft),
            onPressed: () => setState(() => _step = _Step.method),
            tooltip: 'Back',
          ),
        ),
        const SizedBox(height: 8),
        Text(
          'Enter your phone number',
          style: Theme.of(context).textTheme.titleSmall?.copyWith(
                color: Theme.of(context).colorScheme.primary,
              ),
        ),
        const SizedBox(height: 16),
        TextField(
          controller: _phoneCtrl,
          keyboardType: TextInputType.phone,
          decoration: const InputDecoration(
            labelText: 'Phone number',
            hintText: '+6281234567890',
            prefixIcon: Icon(LucideIcons.phone),
            border: OutlineInputBorder(),
          ),
        ),
        const SizedBox(height: 24),
        FilledButton(
          onPressed: _loading ? null : _startPhoneLogin,
          style: FilledButton.styleFrom(
            minimumSize: const Size(0, 48),
          ),
          child: _loading
              ? const SizedBox(
                  width: 20,
                  height: 20,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : const Text('Send Code'),
        ),
      ],
    );
  }

  Widget _buildCodeStep() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Align(
          alignment: Alignment.centerLeft,
          child: IconButton(
            icon: const Icon(LucideIcons.arrowLeft),
            onPressed: () => setState(() => _step = _Step.phoneLogin),
            tooltip: 'Back',
          ),
        ),
        const SizedBox(height: 8),
        Text(
          'Enter verification code',
          style: Theme.of(context).textTheme.titleSmall?.copyWith(
                color: Theme.of(context).colorScheme.primary,
              ),
        ),
        const SizedBox(height: 4),
        Text(
          'Check your Telegram app for the code.',
          style: Theme.of(context).textTheme.bodySmall?.copyWith(
                color: Theme.of(context).colorScheme.onSurfaceVariant,
              ),
        ),
        const SizedBox(height: 16),
        TextField(
          controller: _codeCtrl,
          keyboardType: TextInputType.number,
          maxLength: 5,
          decoration: const InputDecoration(
            labelText: 'Code',
            hintText: '12345',
            prefixIcon: Icon(LucideIcons.shieldCheck),
            border: OutlineInputBorder(),
            counterText: '',
          ),
        ),
        const SizedBox(height: 24),
        FilledButton(
          onPressed: _loading ? null : _submitCode,
          style: FilledButton.styleFrom(
            minimumSize: const Size(0, 48),
          ),
          child: _loading
              ? const SizedBox(
                  width: 20,
                  height: 20,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : const Text('Verify'),
        ),
      ],
    );
  }

  Widget _buildPasswordStep() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Align(
          alignment: Alignment.centerLeft,
          child: IconButton(
            icon: const Icon(LucideIcons.arrowLeft),
            onPressed: () => setState(() => _step = _Step.codeEntry),
            tooltip: 'Back',
          ),
        ),
        const SizedBox(height: 8),
        Text(
          'Two-factor authentication',
          style: Theme.of(context).textTheme.titleSmall?.copyWith(
                color: Theme.of(context).colorScheme.primary,
              ),
        ),
        const SizedBox(height: 4),
        Text(
          'Enter your 2FA password.',
          style: Theme.of(context).textTheme.bodySmall?.copyWith(
                color: Theme.of(context).colorScheme.onSurfaceVariant,
              ),
        ),
        const SizedBox(height: 16),
        TextField(
          controller: _passwordCtrl,
          obscureText: true,
          decoration: const InputDecoration(
            labelText: 'Password',
            prefixIcon: Icon(LucideIcons.lock),
            border: OutlineInputBorder(),
          ),
        ),
        const SizedBox(height: 24),
        FilledButton(
          onPressed: _loading ? null : _submitPassword,
          style: FilledButton.styleFrom(
            minimumSize: const Size(0, 48),
          ),
          child: _loading
              ? const SizedBox(
                  width: 20,
                  height: 20,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : const Text('Submit'),
        ),
      ],
    );
  }
}

// ─── Step Indicator ───────────────────────────────────────────────────────────

class _StepIndicator extends StatelessWidget {
  const _StepIndicator({required this.current});

  final _Step current;

  @override
  Widget build(BuildContext context) {
    final steps = [
      (_Step.credentials, 'Credentials'),
      (_Step.method, 'Method'),
    ];
    final currentIndex = steps.indexWhere((s) => s.$1 == current);

    return Row(
      children: [
        for (var i = 0; i < steps.length; i++) ...[
          if (i > 0)
            Expanded(
              child: Container(
                height: 2,
                color: i <= currentIndex
                    ? Theme.of(context).colorScheme.primary
                    : Theme.of(context).colorScheme.outlineVariant,
              ),
            ),
          _StepDot(
            label: steps[i].$2,
            active: i <= currentIndex,
          ),
        ],
      ],
    );
  }
}

class _StepDot extends StatelessWidget {
  const _StepDot({required this.label, required this.active});

  final String label;
  final bool active;

  @override
  Widget build(BuildContext context) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        Container(
          width: 24,
          height: 24,
          decoration: BoxDecoration(
            shape: BoxShape.circle,
            color: active
                ? Theme.of(context).colorScheme.primary
                : Theme.of(context).colorScheme.outlineVariant,
          ),
          child: active
              ? const Icon(LucideIcons.check, size: 14, color: Colors.white)
              : null,
        ),
        const SizedBox(height: 4),
        Text(
          label,
          style: Theme.of(context).textTheme.bodySmall?.copyWith(
                color: active
                    ? Theme.of(context).colorScheme.primary
                    : Theme.of(context).colorScheme.outline,
              ),
        ),
      ],
    );
  }
}
