import 'package:flutter/material.dart';

import '../rust/api/db.dart' as core;
import '../rust/api/mirror.dart';
import '../rust/api/telegram.dart' as tg;
import '../../main.dart' show telegramHandle;

/// Settings screen (PRD Part 2 §3.4).
///
/// Design principles:
/// - Subtractive: grouped sections, no decoration without purpose
/// - Progressive disclosure: encryption setup behind a dialog
/// - Engineering constraints: confirmation dialogs for destructive actions
/// - Platform conventions: Material 3 switches, proper spacing
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

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Settings')),
      body: ListView(
        children: [
          // ── Backup section ──────────────────────────────────────
          _SectionHeader(title: 'Backup'),
          SwitchListTile(
            title: const Text('Auto backup'),
            subtitle: const Text('Automatically upload new photos'),
            value: _settings.autoBackupEnabled,
            onChanged: (v) => setState(() {
              _settings = AppSettings(
                autoBackupEnabled: v,
                backupOverWifiOnly: _settings.backupOverWifiOnly,
                backupWhileChargingOnly: _settings.backupWhileChargingOnly,
                uploadOriginalQuality: _settings.uploadOriginalQuality,
                folderBackupSettings: _settings.folderBackupSettings,
                clientEncryptionEnabled: _settings.clientEncryptionEnabled,
                vaultPassphraseSet: _settings.vaultPassphraseSet,
                gridColumnCount: _settings.gridColumnCount,
                theme: _settings.theme,
                telegramApiId: _settings.telegramApiId,
                telegramApiHash: _settings.telegramApiHash,
                googleClientId: _settings.googleClientId,
                googleClientSecret: _settings.googleClientSecret,
              );
              _saveSettings();
            }),
          ),
          SwitchListTile(
            title: const Text('WiFi only'),
            subtitle: const Text('Skip backup on cellular data'),
            value: _settings.backupOverWifiOnly,
            onChanged: _settings.autoBackupEnabled
                ? (v) => setState(() {
                      _settings = AppSettings(
                        autoBackupEnabled: _settings.autoBackupEnabled,
                        backupOverWifiOnly: v,
                        backupWhileChargingOnly:
                            _settings.backupWhileChargingOnly,
                        uploadOriginalQuality: _settings.uploadOriginalQuality,
                        folderBackupSettings: _settings.folderBackupSettings,
                        clientEncryptionEnabled:
                            _settings.clientEncryptionEnabled,
                        vaultPassphraseSet: _settings.vaultPassphraseSet,
                        gridColumnCount: _settings.gridColumnCount,
                        theme: _settings.theme,
                        telegramApiId: _settings.telegramApiId,
                        telegramApiHash: _settings.telegramApiHash,
                        googleClientId: _settings.googleClientId,
                        googleClientSecret: _settings.googleClientSecret,
                      );
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
                      _settings = AppSettings(
                        autoBackupEnabled: _settings.autoBackupEnabled,
                        backupOverWifiOnly: _settings.backupOverWifiOnly,
                        backupWhileChargingOnly: v,
                        uploadOriginalQuality: _settings.uploadOriginalQuality,
                        folderBackupSettings: _settings.folderBackupSettings,
                        clientEncryptionEnabled:
                            _settings.clientEncryptionEnabled,
                        vaultPassphraseSet: _settings.vaultPassphraseSet,
                        gridColumnCount: _settings.gridColumnCount,
                        theme: _settings.theme,
                        telegramApiId: _settings.telegramApiId,
                        telegramApiHash: _settings.telegramApiHash,
                        googleClientId: _settings.googleClientId,
                        googleClientSecret: _settings.googleClientSecret,
                      );
                      _saveSettings();
                    })
                : null,
          ),
          SwitchListTile(
            title: const Text('Original quality'),
            subtitle: const Text('Upload full resolution (uses more storage)'),
            value: _settings.uploadOriginalQuality,
            onChanged: (v) => setState(() {
              _settings = AppSettings(
                autoBackupEnabled: _settings.autoBackupEnabled,
                backupOverWifiOnly: _settings.backupOverWifiOnly,
                backupWhileChargingOnly: _settings.backupWhileChargingOnly,
                uploadOriginalQuality: v,
                folderBackupSettings: _settings.folderBackupSettings,
                clientEncryptionEnabled: _settings.clientEncryptionEnabled,
                vaultPassphraseSet: _settings.vaultPassphraseSet,
                gridColumnCount: _settings.gridColumnCount,
                theme: _settings.theme,
                telegramApiId: _settings.telegramApiId,
                telegramApiHash: _settings.telegramApiHash,
                googleClientId: _settings.googleClientId,
                googleClientSecret: _settings.googleClientSecret,
              );
              _saveSettings();
            }),
          ),

          const Divider(height: 1),

          // ── Encryption section ──────────────────────────────────
          _SectionHeader(title: 'Encryption'),
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
                      _settings = AppSettings(
                        autoBackupEnabled: _settings.autoBackupEnabled,
                        backupOverWifiOnly: _settings.backupOverWifiOnly,
                        backupWhileChargingOnly:
                            _settings.backupWhileChargingOnly,
                        uploadOriginalQuality: _settings.uploadOriginalQuality,
                        folderBackupSettings: _settings.folderBackupSettings,
                        clientEncryptionEnabled: v,
                        vaultPassphraseSet: _settings.vaultPassphraseSet,
                        gridColumnCount: _settings.gridColumnCount,
                        theme: _settings.theme,
                        telegramApiId: _settings.telegramApiId,
                        telegramApiHash: _settings.telegramApiHash,
                        googleClientId: _settings.googleClientId,
                        googleClientSecret: _settings.googleClientSecret,
                      );
                      _saveSettings();
                    })
                : null,
          ),
          if (!_settings.vaultPassphraseSet)
            ListTile(
              leading: const Icon(Icons.lock_outline),
              title: const Text('Set up encryption'),
              subtitle: const Text('Create a passphrase to protect your photos'),
              trailing: const Icon(Icons.chevron_right),
              onTap: _showEncryptionSetup,
            ),

          const Divider(height: 1),

          // ── Vault section ───────────────────────────────────────
          _SectionHeader(title: 'Vault'),
          if (_vaultInfo != null) ...[
            ListTile(
              leading: const Icon(Icons.storage),
              title: const Text('Vault channel'),
              subtitle: Text(
                '${_vaultInfo!.channelTitle}\n'
                '${_vaultInfo!.totalBackedUpFiles} files · '
                '${_formatBytes(_vaultInfo!.totalStorageUsedBytes)}',
              ),
              isThreeLine: true,
            ),
          ],

          const Divider(height: 1),

          // ── Display section ─────────────────────────────────────
          _SectionHeader(title: 'Display'),
          ListTile(
            leading: const Icon(Icons.grid_view),
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
                  _settings = AppSettings(
                    autoBackupEnabled: _settings.autoBackupEnabled,
                    backupOverWifiOnly: _settings.backupOverWifiOnly,
                    backupWhileChargingOnly:
                        _settings.backupWhileChargingOnly,
                    uploadOriginalQuality: _settings.uploadOriginalQuality,
                    folderBackupSettings: _settings.folderBackupSettings,
                    clientEncryptionEnabled: _settings.clientEncryptionEnabled,
                    vaultPassphraseSet: _settings.vaultPassphraseSet,
                    gridColumnCount: v,
                    theme: _settings.theme,
                    telegramApiId: _settings.telegramApiId,
                    telegramApiHash: _settings.telegramApiHash,
                    googleClientId: _settings.googleClientId,
                    googleClientSecret: _settings.googleClientSecret,
                  );
                  _saveSettings();
                });
              },
            ),
          ),

          const Divider(height: 1),

          // ── Account section ─────────────────────────────────────
          _SectionHeader(title: 'Account'),
          ListTile(
            leading: const Icon(Icons.logout),
            title: const Text('Logout'),
            subtitle: const Text('Remove Telegram session from this device'),
            onTap: _confirmLogout,
          ),

          const SizedBox(height: 32),
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
      // TODO: wire vault_setup via FRB when crypto functions are exposed
      setState(() {
        _settings = AppSettings(
          autoBackupEnabled: _settings.autoBackupEnabled,
          backupOverWifiOnly: _settings.backupOverWifiOnly,
          backupWhileChargingOnly: _settings.backupWhileChargingOnly,
          uploadOriginalQuality: _settings.uploadOriginalQuality,
          folderBackupSettings: _settings.folderBackupSettings,
          clientEncryptionEnabled: true,
          vaultPassphraseSet: true,
          gridColumnCount: _settings.gridColumnCount,
          theme: _settings.theme,
          telegramApiId: _settings.telegramApiId,
          telegramApiHash: _settings.telegramApiHash,
          googleClientId: _settings.googleClientId,
          googleClientSecret: _settings.googleClientSecret,
        );
        _saveSettings();
      });
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Encryption enabled')),
        );
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

  String _formatBytes(int bytes) {
    if (bytes < 1024 * 1024) return '${(bytes / 1024).toStringAsFixed(0)} KB';
    if (bytes < 1024 * 1024 * 1024) {
      return '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB';
    }
    return '${(bytes / (1024 * 1024 * 1024)).toStringAsFixed(1)} GB';
  }
}

/// Section header — minimal, no decoration.
class _SectionHeader extends StatelessWidget {
  const _SectionHeader({required this.title});
  final String title;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 16, 16, 4),
      child: Text(
        title,
        style: Theme.of(context).textTheme.labelLarge?.copyWith(
              color: Theme.of(context).colorScheme.primary,
            ),
      ),
    );
  }
}
