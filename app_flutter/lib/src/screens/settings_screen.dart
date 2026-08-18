import 'package:flutter/material.dart';

import '../../src/rust/api/db.dart' as core;
import '../../src/rust/api/mirror.dart';

/// Tab Settings (PRD Part 2 §3.4): backup status, free up space, account,
/// encryption, auto-backup, encrypted DB backup, about.
class SettingsScreen extends StatefulWidget {
  const SettingsScreen({super.key});

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  late AppSettings _settings = core.getSettings();
  late final VaultInfo _vault = core.getVaultInfo();
  late final UploadsSummary _summary = core.uploadsSummary();

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Settings')),
      body: ListView(
        padding: const EdgeInsets.all(12),
        children: [
          _BackupStatusCard(
            summary: _summary,
            onBackupNow: () => _toast('Backup started'),
          ),
          const SizedBox(height: 8),
          _Section(
            title: 'Account',
            children: [
              ListTile(
                leading: const Icon(Icons.account_circle_outlined),
                title: Text(_vault.channelTitle.isEmpty
                    ? 'Not signed in'
                    : _vault.channelTitle),
                subtitle: Text(_vault.isPrivate
                    ? 'Private vault channel'
                    : 'Vault channel'),
                trailing: const Icon(Icons.chevron_right),
              ),
              ListTile(
                leading: const Icon(Icons.logout),
                title: const Text('Log out'),
                onTap: () => _toast('Log out coming with account manager'),
              ),
            ],
          ),
          _Section(
            title: 'Encryption',
            children: [
              SwitchListTile(
                secondary: const Icon(Icons.lock_outline),
                title: const Text('Client-side encryption'),
                subtitle: Text(_settings.vaultPassphraseSet
                    ? 'Passphrase set'
                    : 'Not set — media stored encrypted'),
                value: _settings.clientEncryptionEnabled,
                onChanged: (v) => _save(_copy(
                  clientEncryptionEnabled: v,
                )),
              ),
            ],
          ),
          _Section(
            title: 'Auto backup',
            children: [
              SwitchListTile(
                secondary: const Icon(Icons.cloud_upload_outlined),
                title: const Text('Auto backup'),
                value: _settings.autoBackupEnabled,
                onChanged: (v) =>
                    _save(_copy(autoBackupEnabled: v)),
              ),
              SwitchListTile(
                secondary: const Icon(Icons.wifi),
                title: const Text('Wi-Fi only'),
                value: _settings.backupOverWifiOnly,
                onChanged: (v) =>
                    _save(_copy(backupOverWifiOnly: v)),
              ),
              SwitchListTile(
                secondary: const Icon(Icons.battery_charging_full),
                title: const Text('While charging only'),
                value: _settings.backupWhileChargingOnly,
                onChanged: (v) =>
                    _save(_copy(backupWhileChargingOnly: v)),
              ),
            ],
          ),
          _Section(
            title: 'Storage',
            children: [
              ListTile(
                leading: const Icon(Icons.sd_storage_outlined),
                title: const Text('Free up space'),
                subtitle: Text(_freeableText()),
                trailing: const Icon(Icons.chevron_right),
                onTap: () => _toast('Free up space (P1, hash-verified)'),
              ),
              SwitchListTile(
                secondary: const Icon(Icons.high_quality_outlined),
                title: const Text('Upload original quality'),
                subtitle: const Text('MTProto allows files up to 2 GB'),
                value: _settings.uploadOriginalQuality,
                onChanged: (v) =>
                    _save(_copy(uploadOriginalQuality: v)),
              ),
            ],
          ),
          _Section(
            title: 'Backup & restore',
            children: [
              ListTile(
                leading: const Icon(Icons.backup_outlined),
                title: const Text('Encrypted database backup'),
                subtitle: const Text('Metadata + settings, passphrase protected'),
                trailing: const Icon(Icons.chevron_right),
                onTap: () => _toast('Encrypted DB backup (P1)'),
              ),
            ],
          ),
          const _Section(
            title: 'About',
            children: [
              ListTile(
                leading: Icon(Icons.info_outline),
                title: Text('Telegram Photos v2.0'),
                subtitle: Text('Zero-knowledge photo backup via your own '
                    'Telegram account'),
              ),
            ],
          ),
        ],
      ),
    );
  }

  String _freeableText() {
    if (_summary.backedUpBytes <= 0) return 'Nothing backed up yet';
    return '${_fmt(_summary.backedUpBytes)} backed up';
  }

  String _fmt(int b) {
    if (b < 1024 * 1024) return '${(b / 1024).toStringAsFixed(0)} KB';
    if (b < 1024 * 1024 * 1024) {
      return '${(b / (1024 * 1024)).toStringAsFixed(1)} MB';
    }
    return '${(b / (1024 * 1024 * 1024)).toStringAsFixed(2)} GB';
  }

  /// FRB classes have no copyWith; build a new AppSettings explicitly.
  AppSettings _copy({
    bool? autoBackupEnabled,
    bool? backupOverWifiOnly,
    bool? backupWhileChargingOnly,
    bool? uploadOriginalQuality,
    bool? clientEncryptionEnabled,
  }) {
    return AppSettings(
      autoBackupEnabled: autoBackupEnabled ?? _settings.autoBackupEnabled,
      backupOverWifiOnly: backupOverWifiOnly ?? _settings.backupOverWifiOnly,
      backupWhileChargingOnly:
          backupWhileChargingOnly ?? _settings.backupWhileChargingOnly,
      uploadOriginalQuality:
          uploadOriginalQuality ?? _settings.uploadOriginalQuality,
      folderBackupSettings: _settings.folderBackupSettings,
      clientEncryptionEnabled:
          clientEncryptionEnabled ?? _settings.clientEncryptionEnabled,
      vaultPassphraseSet: _settings.vaultPassphraseSet,
      gridColumnCount: _settings.gridColumnCount,
      theme: _settings.theme,
      telegramApiId: _settings.telegramApiId,
      telegramApiHash: _settings.telegramApiHash,
      googleClientId: _settings.googleClientId,
      googleClientSecret: _settings.googleClientSecret,
    );
  }

  void _save(AppSettings s) {
    core.saveSettings(settings: s);
    setState(() => _settings = s);
  }

  void _toast(String msg) {
    ScaffoldMessenger.of(context)
      ..hideCurrentSnackBar()
      ..showSnackBar(SnackBar(content: Text(msg), duration: const Duration(seconds: 2)));
  }
}

class _BackupStatusCard extends StatelessWidget {
  const _BackupStatusCard({required this.summary, required this.onBackupNow});

  final UploadsSummary summary;
  final VoidCallback onBackupNow;

  @override
  Widget build(BuildContext context) {
    final total = summary.backedUpCount + summary.uploadingCount + summary.queuedCount;
    final scheme = Theme.of(context).colorScheme;

    return Card(
      elevation: 0,
      color: scheme.surfaceContainerHighest,
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text('Backup status', style: Theme.of(context).textTheme.titleMedium),
            const SizedBox(height: 8),
            Row(
              children: [
                _Stat(icon: Icons.cloud_done_outlined, label: 'Backed up',
                    value: '${summary.backedUpCount}'),
                const SizedBox(width: 16),
                _Stat(icon: Icons.cloud_upload_outlined, label: 'Pending',
                    value: '${summary.uploadingCount + summary.queuedCount}'),
                const SizedBox(width: 16),
                _Stat(icon: Icons.error_outline, label: 'Failed',
                    value: '${summary.failedCount}'),
              ],
            ),
            const SizedBox(height: 12),
            Text('$total items in backup queue',
                style: Theme.of(context).textTheme.bodySmall),
            const SizedBox(height: 12),
            FilledButton.icon(
              onPressed: onBackupNow,
              icon: const Icon(Icons.cloud_upload_outlined),
              label: const Text('Back up now'),
            ),
          ],
        ),
      ),
    );
  }
}

class _Stat extends StatelessWidget {
  const _Stat({required this.icon, required this.label, required this.value});

  final IconData icon;
  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(children: [Icon(icon, size: 16), const SizedBox(width: 4),
            Text(value, style: Theme.of(context).textTheme.titleMedium)]),
        Text(label, style: Theme.of(context).textTheme.bodySmall),
      ],
    );
  }
}

class _Section extends StatelessWidget {
  const _Section({required this.title, required this.children});

  final String title;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(12, 16, 12, 4),
          child: Text(title, style: Theme.of(context).textTheme.titleSmall),
        ),
        Card(
          elevation: 0,
          margin: EdgeInsets.zero,
          color: Theme.of(context).colorScheme.surfaceContainerLow,
          child: Column(children: children),
        ),
      ],
    );
  }
}
