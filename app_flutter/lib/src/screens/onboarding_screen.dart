import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:qr_flutter/qr_flutter.dart';

import '../rust/api/telegram.dart' as tg;

/// Global Telegram handle from main.dart.
import '../../main.dart' show telegramHandle;

/// Onboarding screen (PRD Part 2 S2): Telegram login via QR or phone OTP.
///
/// Called with [onAuthenticated] callback that re-checks auth state
/// in the parent widget after successful login.
class OnboardingScreen extends StatefulWidget {
  final VoidCallback onAuthenticated;

  const OnboardingScreen({super.key, required this.onAuthenticated});

  @override
  State<OnboardingScreen> createState() => _OnboardingScreenState();
}

enum _LoginMethod { qr, phone }

enum _PhoneStep { credential, code, password }

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
  _PhoneStep _phoneStep = _PhoneStep.credential;

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

  // -- Helpers -----------------------------------------------------------------

  Future<String> _appDataDir() async {
    final dir = await MethodChannel('com.telegramphotos.app/media')
        .invokeMethod<String>('getAppDataDir');
    return dir ?? '';
  }

  // -- Actions -----------------------------------------------------------------

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
    _pollTimer = Timer.periodic(const Duration(seconds: 2), (_) async {
      try {
        final result = await tg.authQrPoll(handle: telegramHandle);
        if (result.status == 'authorized') {
          _pollTimer?.cancel();
          if (mounted) widget.onAuthenticated();
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
        _phoneStep = _PhoneStep.code;
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
        if (mounted) widget.onAuthenticated();
      } else if (result.status == 'password_required') {
        setState(() {
          _phoneStep = _PhoneStep.password;
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
        widget.onAuthenticated();
      }
    } catch (e) {
      setState(() {
        _error = e.toString();
        _loading = false;
      });
    }
  }

  // -- Build -------------------------------------------------------------------

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: SafeArea(
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 24),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              const SizedBox(height: 60),
              // -- Header
              Text(
                'Telegram Photos',
                style: Theme.of(context).textTheme.headlineMedium?.copyWith(
                      fontWeight: FontWeight.bold,
                    ),
              ),
              const SizedBox(height: 12),
              Text(
                'Back up your photos to Telegram',
                style: Theme.of(context).textTheme.titleLarge,
              ),
              const SizedBox(height: 8),
              Text(
                'Connect your Telegram account to start backing up photos '
                'to your private vault with zero-knowledge encryption.',
                style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                      color: Theme.of(context).colorScheme.onSurfaceVariant,
                    ),
              ),
              const Spacer(flex: 2),
              // -- Steps
              if (_method == null && !_credentialsEntered)
                _buildCredentialStep()
              else if (_method == null)
                _buildMethodPicker()
              else if (_method == _LoginMethod.qr)
                _buildQRStep()
              else
                _buildPhoneStep(),
              const Spacer(flex: 1),
              // -- Error
              if (_error != null)
                Padding(
                  padding: const EdgeInsets.only(bottom: 16),
                  child: Text(
                    _error!,
                    style: TextStyle(
                      color: Theme.of(context).colorScheme.error,
                    ),
                    textAlign: TextAlign.center,
                  ),
                ),
              // -- Loading
              if (_loading)
                const Padding(
                  padding: EdgeInsets.only(bottom: 16),
                  child: Center(child: CircularProgressIndicator()),
                ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildCredentialStep() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          'Step 1: Enter API credentials',
          style: Theme.of(context).textTheme.titleMedium,
        ),
        const SizedBox(height: 4),
        Text(
          'Get these from my.telegram.org -> API Development Tools',
          style: Theme.of(context).textTheme.bodySmall?.copyWith(
                color: Theme.of(context).colorScheme.onSurfaceVariant,
              ),
        ),
        const SizedBox(height: 16),
        TextField(
          controller: _apiIdCtrl,
          keyboardType: TextInputType.number,
          onChanged: (_) => setState(() {}),
          decoration: const InputDecoration(
            labelText: 'API ID',
            border: OutlineInputBorder(),
            hintText: '12345678',
          ),
        ),
        const SizedBox(height: 12),
        TextField(
          controller: _apiHashCtrl,
          onChanged: (_) => setState(() {}),
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
        OutlinedButton.icon(
          onPressed: _startQRLogin,
          icon: const Icon(Icons.qr_code_scanner),
          label: const Text('QR Code Login'),
          style: OutlinedButton.styleFrom(
            padding: const EdgeInsets.all(20),
            alignment: Alignment.centerLeft,
          ),
        ),
        const SizedBox(height: 12),
        OutlinedButton.icon(
          onPressed: () => setState(() => _method = _LoginMethod.phone),
          icon: const Icon(Icons.phone),
          label: const Text('Phone Number Login'),
          style: OutlinedButton.styleFrom(
            padding: const EdgeInsets.all(20),
            alignment: Alignment.centerLeft,
          ),
        ),
        const SizedBox(height: 16),
        TextButton(
          onPressed: () => setState(() {
            _method = null;
            _credentialsEntered = false;
          }),
          child: const Text('Change API credentials'),
        ),
      ],
    );
  }

  Widget _buildQRStep() {
    return Column(
      children: [
        Text(
          'Scan QR code with Telegram',
          style: Theme.of(context).textTheme.titleMedium,
        ),
        const SizedBox(height: 16),
        if (_qrUrl != null)
          QrImageView(
            data: _qrUrl!,
            size: 220,
          )
        else
          const SizedBox(
            height: 220,
            child: Center(child: CircularProgressIndicator()),
          ),
        const SizedBox(height: 16),
        Text(
          'Open Telegram on another device, go to Settings > Devices > Link Device',
          style: Theme.of(context).textTheme.bodySmall,
          textAlign: TextAlign.center,
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
            onPressed: () => setState(() {
              _method = null;
              _credentialsEntered = false;
            }),
            icon: const Icon(Icons.arrow_back),
          ),
        ),
        if (_phoneStep == _PhoneStep.credential) ...[
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
        ] else if (_phoneStep == _PhoneStep.code) ...[
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
        ] else if (_phoneStep == _PhoneStep.password) ...[
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
      ],
    );
  }
}
