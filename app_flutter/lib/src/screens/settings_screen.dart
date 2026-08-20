import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../platform/backup_service.dart';
import '../rust/api/crypto.dart' as crypto;
import '../rust/api/db.dart' as core;
import '../rust/api/mirror.dart';
import '../rust/api/telegram.dart' as tg;
import '../../main.dart' show telegramHandle;
import '../../main.dart' show appDataDir;
import 'upload_screen.dart';
import 'onboarding_screen.dart';
import 'free_up_space_screen.dart';
import 'backup_restore_screen.dart';

/// Minimalist Settings screen (PRD Part 2 S3.4).
///
/// Design principles (minimalist):
/// - Backup status prominent at top (one glance)
/// - Grouped sections with thin dividers
/// - Progressive disclosure: advanced settings collapsed
/// - Human-readable language, no jargon
/// - 48dp touch targets throughout
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
      backupOverWifiOnly:
          backupOverWifiOnly ?? _settings.backupOverWifiOnly,
      backupWhileChargingOnly:
          backupWhileChargingOnly ?? _settings.backupWhileChargingOnly,
      uploadOriginalQuality:
          uploadOriginalQuality ?? _settings.uploadOriginalQuality,
      folderBackupSettings: _settings.folderBackupSettings,
      clientEncryptionEnabled:
          clientEncryptionEnabled ?? _settings.clientEncryptionEnabled,
      vaultPassphraseSet:
          vaultPassphraseSet ?? _settings.vaultPassphraseSet,
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
    final cs = Theme.of(context).colorScheme;

    return Scaffold(
      appBar: AppBar(title: const Text('Settings')),
      body: ListView(
        padding: const EdgeInsets.only(bottom: 32),
        children: [
          // -- Backup Status Card (prominent, at-a-glance) --
          _BackupStatusCard(
            vaultInfo: _vaultInfo,
            settings: _settings,
            onBackupNow: _backupNow,
          ),

          const SizedBox(height: 8),

          // -- Backup Settings --
          _buildSection(
            context,
            icon: LucideIcons.cloud,
            title: 'Backup',
            children: [
              _SwitchTile(
                title: 'Auto backup',
                subtitle: 'Upload new photos automatically',
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
              if (_settings.autoBackupEnabled) ...[
                _SwitchTile(
                  title: 'WiFi only',
                  subtitle: 'Skip on cellular data',
                  value: _settings.backupOverWifiOnly,
                  onChanged: (v) => setState(() {
                    _settings = _copyWith(backupOverWifiOnly: v);
                    _saveSettings();
                  }),
                ),
                _SwitchTile(
                  title: 'While charging',
                  subtitle: 'Only when plugged in',
                  value: _settings.backupWhileChargingOnly,
                  onChanged: (v) => setState(() {
                    _settings = _copyWith(backupWhileChargingOnly: v);
                    _saveSettings();
                  }),
                ),
              ],
              _SwitchTile(
                title: 'Original quality',
                subtitle: 'Upload full resolution files',
                value: _settings.uploadOriginalQuality,
                onChanged: (v) => setState(() {
                  _settings = _copyWith(uploadOriginalQuality: v);
                  _saveSettings();
                }),
              ),
            ],
          ),

          // -- Encryption --
          _buildSection(
            context,
            icon: LucideIcons.shieldCheck,
            title: 'Encryption',
            children: [
              _SwitchTile(
                title: 'Client-side encryption',
                subtitle: _settings.vaultPassphraseSet
                    ? 'Files encrypted before upload'
                    : 'Set up a passphrase first',
                value: _settings.clientEncryptionEnabled,
                onChanged: _settings.vaultPassphraseSet
                    ? (v) => setState(() {
                          _settings =
                              _copyWith(clientEncryptionEnabled: v);
                          _saveSettings();
                        })
                    : null,
              ),
              if (!_settings.vaultPassphraseSet)
                _ActionTile(
                  icon: LucideIcons.lock,
                  title: 'Set up encryption',
                  subtitle: 'Protect your photos with a passphrase',
                  onTap: _showEncryptionSetup,
                ),
            ],
          ),

          // -- Display --
          _buildSection(
            context,
            icon: LucideIcons.layoutGrid,
            title: 'Display',
            children: [
              _ActionTile(
                icon: LucideIcons.grid3x3,
                title: 'Grid columns',
                trailing: DropdownButton<int>(
                  value: _settings.gridColumnCount.toInt(),
                  underline: const SizedBox(),
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
            ],
          ),

          // -- Storage --
          _buildSection(
            context,
            icon: LucideIcons.hardDrive,
            title: 'Storage',
            children: [
              _ActionTile(
                icon: LucideIcons.trash2,
                title: 'Free up space',
                subtitle: _vaultInfo != null
                    ? '${_vaultInfo!.totalBackedUpFiles} photos backed up'
                    : 'Back up photos first',
                onTap: _vaultInfo != null &&
                        _vaultInfo!.totalBackedUpFiles > 0
                    ? _freeUpSpace
                    : null,
              ),
              _ActionTile(
                icon: LucideIcons.database,
                title: 'Vault info',
                subtitle: _vaultInfo != null
                    ? '${_vaultInfo!.totalBackedUpFiles} files, '
                        '${_formatBytes(_vaultInfo!.totalStorageUsedBytes)}'
                    : 'No vault yet',
                onTap: () => _showVaultInfo(context),
              ),
              _ActionTile(
                icon: LucideIcons.save,
                title: 'Database backup',
                subtitle: 'Export or restore your encrypted vault index',
                onTap: _openDatabaseBackup,
              ),
            ],
          ),

          // -- Performance --
          _buildSection(
            context,
            icon: LucideIcons.gauge,
            title: 'Performance',
            children: [
              _ActionTile(
                icon: LucideIcons.zap,
                title: 'Run benchmark',
                subtitle: 'Measure speed and memory usage',
                onTap: _runBenchmark,
              ),
            ],
          ),

          // -- Account --
          _buildSection(
            context,
            icon: LucideIcons.user,
            title: 'Account',
            children: [
              _ActionTile(
                icon: LucideIcons.logOut,
                title: 'Logout',
                subtitle: 'Remove Telegram session',
                iconColor: cs.error,
                titleColor: cs.error,
                onTap: _confirmLogout,
              ),
            ],
          ),

          // -- Version --
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 24),
            child: Text(
              'Telegram Photos v0.7.0',
              style: Theme.of(context).textTheme.bodySmall?.copyWith(
                    color: cs.onSurfaceVariant,
                  ),
              textAlign: TextAlign.center,
            ),
          ),
        ],
      ),
    );
  }

  // -- Section Builder --

  Widget _buildSection(
    BuildContext context, {
    required IconData icon,
    required String title,
    required List<Widget> children,
  }) {
    final cs = Theme.of(context).colorScheme;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 20, 16, 4),
          child: Row(
            children: [
              Icon(icon, size: 16, color: cs.primary),
              const SizedBox(width: 8),
              Text(
                title,
                style: Theme.of(context).textTheme.labelLarge?.copyWith(
                      color: cs.primary,
                      fontWeight: FontWeight.w600,
                    ),
              ),
            ],
          ),
        ),
        ...children,
        const Divider(height: 1),
      ],
    );
  }

  // -- Actions --

  Future<void> _backupNow() async {
    final pending = core.listPendingBackup(limit: 200);
    if (pending.isEmpty) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('All photos already backed up')),
        );
      }
      return;
    }
    final items = pending
        .map((m) => {
              'contentUri': m.id,
              'fileName': m.fileName,
              'mimeType': m.mimeType,
              'isVideo': m.mediaType == 'video',
            })
        .toList();
    await BackupService.startBackup(items);
    if (mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Backing up ${pending.length} photos')),
      );
    }
  }

  Future<void> _freeUpSpace() async {
    // The screen itself is a stub today (see free_up_space_screen.dart),
    // but the route is wired so the user can reach it from Settings.
    await Navigator.of(context).push(
      MaterialPageRoute(builder: (_) => const FreeUpSpaceScreen()),
    );
  }

  Future<void> _openDatabaseBackup() async {
    await Navigator.of(context).push(
      MaterialPageRoute(builder: (_) => const BackupRestoreScreen()),
    );
  }

  void _showVaultInfo(BuildContext context) {
    showDialog(
      context: context,
      builder: (_) => AlertDialog(
        title: const Text('Vault Info'),
        content: _vaultInfo != null
            ? Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  _InfoRow('Channel', _vaultInfo!.channelTitle),
                  _InfoRow('Files', '${_vaultInfo!.totalBackedUpFiles}'),
                  _InfoRow(
                    'Size',
                    _formatBytes(_vaultInfo!.totalStorageUsedBytes),
                  ),
                ],
              )
            : const Text('No vault channel yet'),
        actions: [
          FilledButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('OK'),
          ),
        ],
      ),
    );
  }

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
              'Choose a strong passphrase. If you lose it, '
              'encrypted photos cannot be recovered.',
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

  Future<void> _confirmLogout() async {
    final result = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('Logout?'),
        content: const Text(
          'This will remove the Telegram session. '
          'You will need to login again.',
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
        await tg.logout(handle: telegramHandle, appDataDir: appDataDir);
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

  Future<void> _runBenchmark() async {
    final sw = Stopwatch()..start();
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
        title: const Text('Benchmark'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            _InfoRow('Total items', '$count'),
            _InfoRow('DB count', '${dbTime}ms'),
            _InfoRow('Timeline load', '${loadTime}ms'),
            _InfoRow('Items loaded', '${items.length}'),
            _InfoRow(
              'Memory (RSS)',
              '${(totalMem / 1024 / 1024).toStringAsFixed(1)} MB',
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

// -- Backup Status Card --

class _BackupStatusCard extends StatelessWidget {
  const _BackupStatusCard({
    required this.vaultInfo,
    required this.settings,
    required this.onBackupNow,
  });

  final VaultInfo? vaultInfo;
  final AppSettings settings;
  final VoidCallback onBackupNow;

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;
    final files = vaultInfo?.totalBackedUpFiles ?? 0;
    final size = vaultInfo?.totalStorageUsedBytes ?? 0;

    return Container(
      margin: const EdgeInsets.fromLTRB(16, 16, 16, 0),
      padding: const EdgeInsets.all(20),
      decoration: BoxDecoration(
        color: cs.primaryContainer,
        borderRadius: BorderRadius.circular(16),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Icon(LucideIcons.cloud, color: cs.onPrimaryContainer, size: 20),
              const SizedBox(width: 8),
              Text(
                'Backup',
                style: Theme.of(context).textTheme.titleMedium?.copyWith(
                      color: cs.onPrimaryContainer,
                    ),
              ),
              const Spacer(),
              if (settings.autoBackupEnabled)
                Container(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
                  decoration: BoxDecoration(
                    color: cs.onPrimaryContainer.withValues(alpha: 0.15),
                    borderRadius: BorderRadius.circular(12),
                  ),
                  child: Text(
                    'Auto',
                    style: Theme.of(context).textTheme.labelSmall?.copyWith(
                          color: cs.onPrimaryContainer,
                        ),
                  ),
                ),
            ],
          ),
          const SizedBox(height: 12),
          Text(
            '$files photos backed up',
            style: Theme.of(context).textTheme.headlineSmall?.copyWith(
                  color: cs.onPrimaryContainer,
                  fontWeight: FontWeight.w600,
                ),
          ),
          if (size > 0) ...[
            const SizedBox(height: 2),
            Text(
              '${_formatBytes(size)} in your private vault',
              style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                    color: cs.onPrimaryContainer.withValues(alpha: 0.7),
                  ),
            ),
          ],
          const SizedBox(height: 12),
          SizedBox(
            width: double.infinity,
            child: FilledButton.tonalIcon(
              onPressed: onBackupNow,
              icon: const Icon(LucideIcons.upload, size: 18),
              label: const Text('Back up now'),
              style: FilledButton.styleFrom(
                backgroundColor: cs.onPrimaryContainer.withValues(alpha: 0.15),
                foregroundColor: cs.onPrimaryContainer,
                minimumSize: const Size(0, 44),
              ),
            ),
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

// -- Switch Tile (minimalist) --

class _SwitchTile extends StatelessWidget {
  const _SwitchTile({
    required this.title,
    required this.value,
    this.subtitle,
    this.onChanged,
  });

  final String title;
  final String? subtitle;
  final bool value;
  final ValueChanged<bool>? onChanged;

  @override
  Widget build(BuildContext context) {
    return SwitchListTile(
      title: Text(title, style: const TextStyle(fontSize: 15)),
      subtitle: subtitle != null
          ? Text(subtitle!, style: const TextStyle(fontSize: 13))
          : null,
      value: value,
      onChanged: onChanged,
      contentPadding: const EdgeInsets.symmetric(horizontal: 16),
      visualDensity: VisualDensity.compact,
    );
  }
}

// -- Action Tile (minimalist) --

class _ActionTile extends StatelessWidget {
  const _ActionTile({
    required this.icon,
    required this.title,
    this.subtitle,
    this.trailing,
    this.onTap,
    this.iconColor,
    this.titleColor,
  });

  final IconData icon;
  final String title;
  final String? subtitle;
  final Widget? trailing;
  final VoidCallback? onTap;
  final Color? iconColor;
  final Color? titleColor;

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;
    return ListTile(
      leading: Icon(icon, size: 20, color: iconColor ?? cs.onSurfaceVariant),
      title: Text(
        title,
        style: TextStyle(
          fontSize: 15,
          color: titleColor,
        ),
      ),
      subtitle: subtitle != null
          ? Text(subtitle!, style: const TextStyle(fontSize: 13))
          : null,
      trailing: trailing ??
          (onTap != null
              ? Icon(LucideIcons.chevronRight,
                  size: 18, color: cs.onSurfaceVariant)
              : null),
      onTap: onTap,
      contentPadding: const EdgeInsets.symmetric(horizontal: 16),
      visualDensity: VisualDensity.compact,
    );
  }
}

// -- Info Row --

class _InfoRow extends StatelessWidget {
  const _InfoRow(this.label, this.value);
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
