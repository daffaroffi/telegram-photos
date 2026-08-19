import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:qr_flutter/qr_flutter.dart';

import '../rust/api/telegram.dart' as tg;

/// Global Telegram handle from main.dart.
import '../../main.dart' show telegramHandle;

/// Onboarding screen (PRD Part 2 §2): Telegram login via QR or phone OTP.
class OnboardingScreen extends StatefulWidget {
  const OnboardingScreen({super.key});

  @override
  State<OnboardingScreen> createState() => _OnboardingScreenState();
}

class _OnboardingScreenState extends State<OnboardingScreen> {
  // API credentials
  final _apiIdCtrl = TextEditingController();
  final _apiHashCtrl = TextEditingController();
  bool _credentialsEntered = false;

  // Login state
  _LoginMethod? _method;
  bool _loading = false;
  String? _error;

  // Phone login
  final _phoneCtrl = TextEditingController();
  final _codeCtrl = TextEditingController();
  final _passwordCtrl = TextEditingController();
  _PhoneStep _phoneStep = _phoneStepCredential;

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

  // ── Actions ──────────────────────────────────────────────────────────────

  Future<void> _startQRLogin() async {
    if (!_validApiCreds) return;
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final apiId = int.parse(_apiIdCtrl.text.trim());
      final url = await tg.authQrLogin(
        handle: telegramHandle,
        apiId: apiId,
        apiHash: _apiHashCtrl.text.trim(),
        appDataDir: await _appDataDir(),
      );
      setState(() {
        _qrUrl = url;
        _method = _LoginMethod.qr;
        _loading = false;
      });
      _startPolling();
    } catch (e) {
      setState(() {
        _error = e.toString();
        _loading = false;
      });
    }
  }

  void _startPolling() {
    _pollTimer?.cancel();
    _pollTimer = Timer.periodic(const Duration(seconds: 3), (_) async {
      try {
        final result = await tg.authQrPoll(handle: telegramHandle);
        if (result.status == 'authorized' && mounted) {
          _pollTimer?.cancel();
          Navigator.of(context).pop(true);
        }
      } catch (_) {
        // Silently retry
      }
    });
  }

  Future<void> _requestPhoneCode() async {
    if (!_validApiCreds || _phoneCtrl.text.trim().isEmpty) return;
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final apiId = int.parse(_apiIdCtrl.text.trim());
      await tg.authRequestCode(
        handle: telegramHandle,
        phone: _phoneCtrl.text.trim(),
        apiId: apiId,
        apiHash: _apiHashCtrl.text.trim(),
        appDataDir: await _appDataDir(),
      );
      setState(() {
        _phoneStep = _phoneStepCode;
        _loading = false;
      });
    } catch (e) {
      setState(() {
        _error = e.toString();
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
        if (mounted) Navigator.of(context).pop(true);
      } else if (result.status == 'password_required') {
        setState(() {
          _phoneStep = _phoneStepPassword;
          _loading = false;
        });
      }
    } catch (e) {
      setState(() {
        _error = e.toString();
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
      if (result.status == 'authorized' && mounted) {
        Navigator.of(context).pop(true);
      }
    } catch (e) {
      setState(() {
        _error = e.toString();
        _loading = false;
      });
    }
  }

  Future<String> _appDataDir() async {
    final dir = await const MethodChannel('com.telegramphotos.app/media')
        .invokeMethod<String>('getAppDataDir');
    return dir ?? '/data/data/com.telegramphotos.app/files';
  }

  // ── Build ────────────────────────────────────────────────────────────────

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Telegram Photos')),
      body: SafeArea(
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(24),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              // Header
              Icon(
                Icons.cloud_upload_outlined,
                size: 64,
                color: Theme.of(context).colorScheme.primary,
              ),
              const SizedBox(height: 16),
              Text(
                'Back up your photos to Telegram',
                style: Theme.of(context).textTheme.headlineSmall,
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: 8),
              Text(
                'Connect your Telegram account to start backing up photos '
                'to your private vault with zero-knowledge encryption.',
                style: Theme.of(context).textTheme.bodyMedium,
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: 32),

              // Content
              if (!_credentialsEntered) ...[
                _buildCredentialsForm(),
              ] else if (_method == null) ...[
                _buildMethodPicker(),
              ] else if (_method == _LoginMethod.qr) ...[
                _buildQRView(),
              ] else ...[
                _buildPhoneView(),
              ],

              // Error
              if (_error != null) ...[
                const SizedBox(height: 16),
                Container(
                  padding: const EdgeInsets.all(12),
                  decoration: BoxDecoration(
                    color: Theme.of(context).colorScheme.errorContainer,
                    borderRadius: BorderRadius.circular(8),
                  ),
                  child: Text(
                    _error!,
                    style: TextStyle(
                      color: Theme.of(context).colorScheme.onErrorContainer,
                    ),
                  ),
                ),
              ],

              // Loading
              if (_loading) ...[
                const SizedBox(height: 24),
                const Center(child: CircularProgressIndicator()),
              ],
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildCredentialsForm() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          'Step 1: Enter API credentials',
          style: Theme.of(context).textTheme.titleMedium,
        ),
        const SizedBox(height: 4),
        Text(
          'Get these from my.telegram.org → API Development Tools',
          style: Theme.of(context).textTheme.bodySmall,
        ),
        const SizedBox(height: 16),
        TextField(
          controller: _apiIdCtrl,
          keyboardType: TextInputType.number,
          decoration: const InputDecoration(
            labelText: 'API ID',
            border: OutlineInputBorder(),
            hintText: '12345678',
          ),
        ),
        const SizedBox(height: 12),
        TextField(
          controller: _apiHashCtrl,
          decoration: const InputDecoration(
            labelText: 'API Hash',
            border: OutlineInputBorder(),
            hintText: 'abcdef1234567890abcdef',
          ),
        ),
        const SizedBox(height: 16),
        FilledButton(
          onPressed: _validApiCreds
              ? () => setState(() => _credentialsEntered = true)
              : null,
          child: const Text('Continue'),
        ),
      ],
    );
  }

  Widget _buildMethodPicker() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          'Step 2: Choose login method',
          style: Theme.of(context).textTheme.titleMedium,
        ),
        const SizedBox(height: 16),
        Card(
          child: ListTile(
            leading: const Icon(Icons.qr_code),
            title: const Text('QR Code Login'),
            subtitle: const Text('Scan with Telegram on another device'),
            trailing: const Icon(Icons.chevron_right),
            onTap: _startQRLogin,
          ),
        ),
        const SizedBox(height: 8),
        Card(
          child: ListTile(
            leading: const Icon(Icons.phone),
            title: const Text('Phone Number Login'),
            subtitle: const Text('Receive a verification code via SMS'),
            trailing: const Icon(Icons.chevron_right),
            onTap: () => setState(() => _method = _LoginMethod.phone),
          ),
        ),
        const SizedBox(height: 12),
        TextButton(
          onPressed: () => setState(() {
            _credentialsEntered = false;
            _method = null;
          }),
          child: const Text('Change API credentials'),
        ),
      ],
    );
  }

  Widget _buildQRView() {
    return Column(
      children: [
        Text(
          'Scan this QR code with Telegram',
          style: Theme.of(context).textTheme.titleMedium,
        ),
        const SizedBox(height: 16),
        if (_qrUrl != null)
          QrImageView(
            data: _qrUrl!,
            version: QrVersions.auto,
            size: 220,
          )
        else
          const SizedBox(
            height: 220,
            child: Center(child: CircularProgressIndicator()),
          ),
        const SizedBox(height: 12),
        Text(
          'Open Telegram → Settings → Devices → Link Desktop Device',
          style: Theme.of(context).textTheme.bodySmall,
          textAlign: TextAlign.center,
        ),
        const SizedBox(height: 8),
        Text(
          'Waiting for scan...',
          style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                color: Theme.of(context).colorScheme.primary,
              ),
        ),
      ],
    );
  }

  Widget _buildPhoneView() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (_phoneStep == _phoneStepCredential) ...[
          Text(
            'Enter your phone number',
            style: Theme.of(context).textTheme.titleMedium,
          ),
          const SizedBox(height: 16),
          TextField(
            controller: _phoneCtrl,
            keyboardType: TextInputType.phone,
            decoration: const InputDecoration(
              labelText: 'Phone number',
              border: OutlineInputBorder(),
              hintText: '+6281234567890',
            ),
          ),
          const SizedBox(height: 16),
          FilledButton(
            onPressed: _requestPhoneCode,
            child: const Text('Send Code'),
          ),
        ] else if (_phoneStep == _phoneStepCode) ...[
          Text(
            'Enter verification code',
            style: Theme.of(context).textTheme.titleMedium,
          ),
          const SizedBox(height: 16),
          TextField(
            controller: _codeCtrl,
            keyboardType: TextInputType.number,
            maxLength: 5,
            decoration: const InputDecoration(
              labelText: 'Code',
              border: OutlineInputBorder(),
              hintText: '12345',
            ),
          ),
          const SizedBox(height: 16),
          FilledButton(
            onPressed: _submitCode,
            child: const Text('Verify'),
          ),
        ] else if (_phoneStep == _phoneStepPassword) ...[
          Text(
            'Enter 2FA password',
            style: Theme.of(context).textTheme.titleMedium,
          ),
          const SizedBox(height: 16),
          TextField(
            controller: _passwordCtrl,
            obscureText: true,
            decoration: const InputDecoration(
              labelText: 'Password',
              border: OutlineInputBorder(),
            ),
          ),
          const SizedBox(height: 16),
          FilledButton(
            onPressed: _submitPassword,
            child: const Text('Unlock'),
          ),
        ],
        const SizedBox(height: 12),
        TextButton(
          onPressed: () => setState(() {
            _method = null;
            _phoneStep = _phoneStepCredential;
          }),
          child: const Text('Back'),
        ),
      ],
    );
  }
}

enum _LoginMethod { qr, phone }

enum _PhoneStep { credential, code, password }

const _phoneStepCredential = _PhoneStep.credential;
const _phoneStepCode = _PhoneStep.code;
const _phoneStepPassword = _PhoneStep.password;
