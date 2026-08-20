import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../rust/api/crypto.dart' as crypto;
import '../rust/api/db.dart' as core;

/// Encrypted DB backup/restore screen (PRD Part 2 S6.6, T10).
///
/// Export: media_items + uploads + captions + collections + settings
/// -> JSON -> encrypt XChaCha20-Poly1305 -> save to vault (.tphotos-backup)
/// Restore: decrypt -> parse -> merge (not replace) via sha256_hash
class BackupRestoreScreen extends StatefulWidget {
  const BackupRestoreScreen({super.key});

  @override
  State<BackupRestoreScreen> createState() => _BackupRestoreScreenState();
}

class _BackupRestoreScreenState extends State<BackupRestoreScreen> {
  bool _backingUp = false;
  bool _restoring = false;

  Future<void> _backupDatabase() async {
    if (_backingUp) return;

    // The full DB-export pipeline (serialize -> encrypt -> upload) is
    // not yet wired up in v0.7. Show a clear "coming soon" instead of
    // faking success with a delay.
    if (mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text(
            'Database backup is not implemented yet — coming in a follow-up',
          ),
        ),
      );
    }
  }

  Future<void> _restoreDatabase() async {
    if (_restoring) return;
    // Symmetric to _backupDatabase: the restore pipeline is not wired
    // up in v0.7, so don't fake success with a delay.
    if (mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text(
            'Database restore is not implemented yet — coming in a follow-up',
          ),
        ),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;

    return Scaffold(
      appBar: AppBar(title: const Text('Database backup')),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          // Info card
          Container(
            padding: const EdgeInsets.all(16),
            decoration: BoxDecoration(
              color: cs.primaryContainer,
              borderRadius: BorderRadius.circular(12),
            ),
            child: Row(
              children: [
                Icon(LucideIcons.shieldCheck, color: cs.onPrimaryContainer),
                const SizedBox(width: 12),
                Expanded(
                  child: Text(
                    'Your database is encrypted before backup. '
                    'Only you can read it with your passphrase.',
                    style: TextStyle(color: cs.onPrimaryContainer),
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 24),

          // Backup button
          SizedBox(
            width: double.infinity,
            child: OutlinedButton.icon(
              onPressed: _backingUp ? null : _backupDatabase,
              icon: _backingUp
                  ? const SizedBox(
                      width: 18,
                      height: 18,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(LucideIcons.upload),
              label: Text(_backingUp ? 'Backing up...' : 'Backup now'),
              style: OutlinedButton.styleFrom(
                minimumSize: const Size(0, 48),
              ),
            ),
          ),
          const SizedBox(height: 12),

          // Restore button
          SizedBox(
            width: double.infinity,
            child: OutlinedButton.icon(
              onPressed: _restoring ? null : _restoreDatabase,
              icon: _restoring
                  ? const SizedBox(
                      width: 18,
                      height: 18,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(LucideIcons.download),
              label: Text(_restoring ? 'Restoring...' : 'Restore from vault'),
              style: OutlinedButton.styleFrom(
                minimumSize: const Size(0, 48),
              ),
            ),
          ),
          const SizedBox(height: 24),

          // What's backed up
          Text(
            'What is backed up',
            style: Theme.of(context).textTheme.titleSmall,
          ),
          const SizedBox(height: 8),
          _InfoItem(
            icon: LucideIcons.image,
            title: 'Photo metadata',
            subtitle: 'File info, dates, locations',
          ),
          _InfoItem(
            icon: LucideIcons.upload,
            title: 'Upload history',
            subtitle: 'What was backed up and when',
          ),
          _InfoItem(
            icon: LucideIcons.penLine,
            title: 'Captions & tags',
            subtitle: 'Your notes and hashtags',
          ),
          _InfoItem(
            icon: LucideIcons.folder,
            title: 'Collections',
            subtitle: 'Album organization',
          ),
          _InfoItem(
            icon: LucideIcons.settings,
            title: 'Settings',
            subtitle: 'Backup preferences and encryption config',
          ),
        ],
      ),
    );
  }
}

class _InfoItem extends StatelessWidget {
  const _InfoItem({
    required this.icon,
    required this.title,
    required this.subtitle,
  });

  final IconData icon;
  final String title;
  final String subtitle;

  @override
  Widget build(BuildContext context) {
    return ListTile(
      leading: Icon(icon, size: 20),
      title: Text(title, style: const TextStyle(fontSize: 15)),
      subtitle: Text(subtitle, style: const TextStyle(fontSize: 13)),
      contentPadding: EdgeInsets.zero,
      dense: true,
    );
  }
}
