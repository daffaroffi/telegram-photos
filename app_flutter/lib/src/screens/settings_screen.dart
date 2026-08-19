import 'dart:io';

import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../platform/backup_service.dart';
import '../rust/api/crypto.dart' as crypto;
import '../rust/api/db.dart' as core;
import '../rust/api/mirror.dart';
import '../rust/api/telegram.dart' as tg;
import '../../main.dart' show telegramHandle;

/// Settings screen (PRD Part 2 S3.4).
///
/// Design principles:
/// - Subtractive: grouped sections, no decoration without purpose
/// - Progressive disclosure: encryption setup behind a dialog
/// - Engineering constraints: confirmation dialogs for destructive actions
/// - Platform conventions: Material 3 switches, proper spacing
/// - Lucide icons throughout for consistency
class SettingsScreen extends StatefulWidget {
  const SettingsScreen({super.key});

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  late AppSettings _settings;
  VaultInfo? _vaultInfo;

  @override
  void initState() {
    super.initState();
    _settings = core.getSettings();
    _vaultInfo = core.getVaultInfo();
  }

  void _saveSettings() {
    core.saveSettings(settings: _settings);
  }

  AppSettings _copyWith({
    bool? autoBackupEnabled,
    bool? backupOverWifiOnly,
    bool? backupWhileChargingOnly,
    bool? uploadOriginalQuality,
    bool? clientEncryptionEnabled,
    bool? vaultPassphraseSet,
    int? gridColumnCount,
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
      vaultPassphraseSet: vaultPassphraseSet ?? _settings.vaultPassphraseSet,
      gridColumnCount: gridColumnCount ?? _settings.gridColumnCount,
      theme: _settings.theme,
      telegramApiId: _settings.telegramApiId,
      telegramApiHash: _settings.telegramApiHash,
      googleClientId: _settings.googleClientId,
      googleClientSecret: _settings.googleClientSecret,
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Settings')),
      body: ListView(
        children: [
          // ── Backup section ──────────────────────────────────────
          _SectionHeader(icon: LucideIcons.cloud, title: 'Backup'),
          SwitchListTile(
            title: const Text('Auto backup'),
            subtitle: const Text('Automatically upload new photos'),
            value: _settings.autoBackupEnabled,
            onChanged: (v) async {
              setState(() {
                _settings = _copyWith(autoBackupEnabled: v);
                _saveSettings();
              });
              if (v) {
                final pending = core.listPendingBackup(limit: 200);
                if (pending.isNotEmpty) {
                  final items = pending
                      .map((m) => {
                            'contentUri': m.id,
                            'fileName': m.fileName,
                            'mimeType': m.mimeType,
                            'isVideo': m.mediaType == 'video',
                          })
                      .toList();
                  await BackupService.startBackup(items);
                }
              } else {
                await BackupService.cancelBackup();
              }
            },
          ),
          SwitchListTile(
            title: const Text('WiFi only'),
            subtitle: const Text('Skip backup on cellular data'),
            value: _settings.backupOverWifiOnly,
            onChanged: _settings.autoBackupEnabled
                ? (v) => setState(() {
                      _settings = _copyWith(backupOverWifiOnly: v);
                      _saveSettings();
                    })
                : null,
          ),
          SwitchListTile(
            title: const Text('While charging'),
            subtitle: const Text('Only upload when plugged in'),
            value: _settings.backupWhileChargingOnly,
            onChanged: _settings.autoBackupEnabled
                ? (v) => setState(() {
                      _settings = _copyWith(backupWhileChargingOnly: v);
                      _saveSettings();
                    })
                : null,
          ),
          SwitchListTile(
            title: const Text('Original quality'),
            subtitle: const Text('Upload full resolution'),
            value: _settings.uploadOriginalQuality,
            onChanged: (v) => setState(() {
              _settings = _copyWith(uploadOriginalQuality: v);
              _saveSettings();
            }),
          ),

          const Divider(height: 1),

          // ── Encryption section ──────────────────────────────────
          _SectionHeader(icon: LucideIcons.shieldCheck, title: 'Encryption'),
          SwitchListTile(
            title: const Text('Client-side encryption'),
            subtitle: Text(
              _settings.vaultPassphraseSet
                  ? 'Encrypts files before upload'
                  : 'Set up a passphrase first',
            ),
            value: _settings.clientEncryptionEnabled,
            onChanged: _settings.vaultPassphraseSet
                ? (v) => setState(() {
                      _settings = _copyWith(clientEncryptionEnabled: v);
                      _saveSettings();
                    })
                : null,
          ),
          if (!_settings.vaultPassphraseSet)
            ListTile(
              leading: const Icon(LucideIcons.lock),
              title: const Text('Set up encryption'),
              subtitle: const Text('Create a passphrase to protect your photos'),
              trailing: const Icon(LucideIcons.chevronRight),
              onTap: _showEncryptionSetup,
            ),

          const Divider(height: 1),

          // ── Vault section ───────────────────────────────────────
          _SectionHeader(icon: LucideIcons.database, title: 'Vault'),
          if (_vaultInfo != null)
            ListTile(
              leading: const Icon(LucideIcons.database),
              title: const Text('Vault channel'),
              subtitle: Text(
                '${_vaultInfo!.channelTitle}\n'
                '${_vaultInfo!.totalBackedUpFiles} files . '
                '${_formatBytes(_vaultInfo!.totalStorageUsedBytes)}',
              ),
              isThreeLine: true,
            ),

          const Divider(height: 1),

          // ── Display section ─────────────────────────────────────
          _SectionHeader(icon: LucideIcons.layoutGrid, title: 'Display'),
          ListTile(
            leading: const Icon(LucideIcons.grid3x3),
            title: const Text('Grid columns'),
            trailing: DropdownButton<int>(
              value: _settings.gridColumnCount.toInt(),
              items: const [
                DropdownMenuItem(value: 3, child: Text('3')),
                DropdownMenuItem(value: 4, child: Text('4')),
                DropdownMenuItem(value: 5, child: Text('5')),
                DropdownMenuItem(value: 6, child: Text('6')),
              ],
              onChanged: (v) {
                if (v == null) return;
                setState(() {
                  _settings = _copyWith(gridColumnCount: v);
                  _saveSettings();
                });
              },
            ),
          ),

          const Divider(height: 1),

          // ── Performance section ─────────────────────────────────
          _SectionHeader(icon: LucideIcons.zap, title: 'Performance'),
          ListTile(
            leading: const Icon(LucideIcons.gauge),
            title: const Text('Run benchmark'),
            subtitle: const Text('Measure cold start, scan, and memory usage'),
            trailing: const Icon(LucideIcons.chevronRight),
            onTap: _runBenchmark,
          ),

          const Divider(height: 1),

          // ── Account section ─────────────────────────────────────
          _SectionHeader(icon: LucideIcons.user, title: 'Account'),
          ListTile(
            leading: Icon(
              LucideIcons.logOut,
              color: Theme.of(context).colorScheme.error,
            ),
            title: Text(
              'Logout',
              style: TextStyle(color: Theme.of(context).colorScheme.error),
            ),
            subtitle: const Text('Remove Telegram session from this device'),
            onTap: _confirmLogout,
          ),

          const SizedBox(height: 32),
        ],
      ),
    );
  }

  // ─── Encryption Setup ────────────────────────────────────────────────────

  Future<void> _showEncryptionSetup() async {
    final ctrl = TextEditingController();
    final result = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('Set up encryption'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const Text(
              'Choose a strong passphrase. If you lose it, encrypted '
              'photos cannot be recovered.',
              style: TextStyle(fontSize: 13),
            ),
            const SizedBox(height: 16),
            TextField(
              controller: ctrl,
              obscureText: true,
              decoration: const InputDecoration(
                labelText: 'Passphrase',
                border: OutlineInputBorder(),
                prefixIcon: Icon(LucideIcons.lock),
              ),
            ),
          ],
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(ctx).pop(true),
            child: const Text('Save'),
          ),
        ],
      ),
    );

    if (result == true && ctrl.text.isNotEmpty) {
      try {
        await crypto.vaultSetup(passphrase: ctrl.text);
        setState(() {
          _settings = _copyWith(
            clientEncryptionEnabled: true,
            vaultPassphraseSet: true,
          );
          _saveSettings();
        });
        if (mounted) {
          ScaffoldMessenger.of(context).showSnackBar(
            const SnackBar(content: Text('Encryption enabled')),
          );
        }
      } catch (e) {
        if (mounted) {
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(content: Text('Error: $e')),
          );
        }
      }
    }
    ctrl.dispose();
  }

  // ─── Logout ──────────────────────────────────────────────────────────────

  Future<void> _confirmLogout() async {
    final result = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('Logout?'),
        content: const Text(
          'This will remove the Telegram session. '
          'You will need to login again to continue backing up.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(ctx).pop(true),
            style: FilledButton.styleFrom(
              backgroundColor: Theme.of(ctx).colorScheme.error,
            ),
            child: const Text('Logout'),
          ),
        ],
      ),
    );

    if (result == true && mounted) {
      try {
        await tg.logout(handle: telegramHandle, appDataDir: '');
        if (mounted) {
          ScaffoldMessenger.of(context).showSnackBar(
            const SnackBar(content: Text('Logged out')),
          );
        }
      } catch (e) {
        if (mounted) {
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(content: Text('Logout failed: $e')),
          );
        }
      }
    }
  }

  // ─── Benchmark ───────────────────────────────────────────────────────────

  Future<void> _runBenchmark() async {
    final sw = Stopwatch()..start();

    sw.reset();
    final count = core.countMedia();
    final dbTime = sw.elapsedMilliseconds;

    sw.reset();
    final items = core.listTimeline(beforeTimestamp: null, limit: 1000);
    final loadTime = sw.elapsedMilliseconds;

    final totalMem = ProcessInfo.currentRss;
    sw.stop();

    if (!mounted) return;
    showDialog(
      context: context,
      builder: (_) => AlertDialog(
        title: const Text('Benchmark Results'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            _BenchmarkRow(label: 'Total items in DB', value: '$count'),
            _BenchmarkRow(label: 'DB count query', value: '${dbTime}ms'),
            _BenchmarkRow(label: 'Timeline load (1k)', value: '${loadTime}ms'),
            _BenchmarkRow(label: 'Items loaded', value: '${items.length}'),
            _BenchmarkRow(
              label: 'RSS memory',
              value: '${(totalMem / 1024 / 1024).toStringAsFixed(1)} MB',
            ),
          ],
        ),
        actions: [
          FilledButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('Close'),
          ),
        ],
      ),
    );
  }

  String _formatBytes(int bytes) {
    if (bytes < 1024 * 1024) return '${(bytes / 1024).toStringAsFixed(0)} KB';
    if (bytes < 1024 * 1024 * 1024) {
      return '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB';
    }
    return '${(bytes / (1024 * 1024 * 1024)).toStringAsFixed(1)} GB';
  }
}

// ─── Section Header ───────────────────────────────────────────────────────────

class _SectionHeader extends StatelessWidget {
  const _SectionHeader({required this.icon, required this.title});

  final IconData icon;
  final String title;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 16, 16, 4),
      child: Row(
        children: [
          Icon(icon, size: 18, color: Theme.of(context).colorScheme.primary),
          const SizedBox(width: 8),
          Text(
            title,
            style: Theme.of(context).textTheme.titleSmall?.copyWith(
                  color: Theme.of(context).colorScheme.primary,
                ),
          ),
        ],
      ),
    );
  }
}

// ─── Benchmark Row ────────────────────────────────────────────────────────────

class _BenchmarkRow extends StatelessWidget {
  const _BenchmarkRow({required this.label, required this.value});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 4),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          Text(label, style: Theme.of(context).textTheme.bodyMedium),
          Text(
            value,
            style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                  fontWeight: FontWeight.w600,
                ),
          ),
        ],
      ),
    );
  }
}
