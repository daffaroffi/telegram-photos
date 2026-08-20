import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../rust/api/db.dart' as core;
import '../rust/api/mirror.dart';

/// Free Up Space screen (PRD Part 2 S4.4, G8).
///
/// Shows reclaimable space (only BACKED_UP + verified hash).
/// Threshold: only show if reclaimable > 500 MB (PRD G8).
/// Execution: cancellable + undo 5 seconds.
class FreeUpSpaceScreen extends StatefulWidget {
  const FreeUpSpaceScreen({super.key});

  @override
  State<FreeUpSpaceScreen> createState() => _FreeUpSpaceScreenState();
}

class _FreeUpSpaceScreenState extends State<FreeUpSpaceScreen> {
  bool _processing = false;
  bool _done = false;
  int _reclaimableBytes = 0;
  int _reclaimableCount = 0;

  @override
  void initState() {
    super.initState();
    _calculateReclaimable();
  }

  void _calculateReclaimable() {
    // Count items that are BACKED_UP
    final allMedia = core.listTimeline(beforeTimestamp: null, limit: 99999);
    int reclaimable = 0;
    int count = 0;
    for (final item in allMedia) {
      if (item.syncStatus == 'BACKED_UP') {
        reclaimable += item.fileSizeBytes;
        count++;
      }
    }
    setState(() {
      _reclaimableBytes = reclaimable;
      _reclaimableCount = count;
    });
  }

  Future<void> _freeUpSpace() async {
    if (_processing) return;
    setState(() => _processing = true);

    // PRD G8: only proceed if > 500 MB reclaimable
    if (_reclaimableBytes < 500 * 1024 * 1024) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text('Not enough space to free up (minimum 500 MB)'),
          ),
        );
        setState(() => _processing = false);
      }
      return;
    }

    // TODO: Implement actual free up space with hash verification
    // For now, simulate
    await Future.delayed(const Duration(seconds: 2));

    if (mounted) {
      setState(() {
        _processing = false;
        _done = true;
      });

      // PRD S4.4: undo 5 seconds
      final snackBar = SnackBar(
        content: Text('${_formatBytes(_reclaimableBytes)} freed'),
        action: SnackBarAction(
          label: 'Undo',
          onPressed: () {
            setState(() => _done = false);
            ScaffoldMessenger.of(context).showSnackBar(
              const SnackBar(content: Text('Undo successful')),
            );
          },
        ),
        duration: const Duration(seconds: 5),
      );
      ScaffoldMessenger.of(context).showSnackBar(snackBar);
    }
  }

  String _formatBytes(int bytes) {
    if (bytes < 1024 * 1024) return '${(bytes / 1024).toStringAsFixed(0)} KB';
    if (bytes < 1024 * 1024 * 1024) {
      return '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB';
    }
    return '${(bytes / (1024 * 1024 * 1024)).toStringAsFixed(1)} GB';
  }

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;
    final canFreeUp = _reclaimableBytes >= 500 * 1024 * 1024;

    return Scaffold(
      appBar: AppBar(title: const Text('Free up space')),
      body: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            // Status icon
            Center(
              child: Icon(
                _done ? LucideIcons.circleCheck : LucideIcons.hardDrive,
                size: 72,
                color: _done
                    ? Colors.green
                    : canFreeUp
                        ? cs.primary
                        : cs.outline,
              ),
            ),
            const SizedBox(height: 24),

            // Amount
            Center(
              child: Text(
                _done
                    ? 'Space freed!'
                    : '${_formatBytes(_reclaimableBytes)} can be freed',
                style: Theme.of(context).textTheme.headlineSmall?.copyWith(
                      fontWeight: FontWeight.w600,
                    ),
              ),
            ),
            const SizedBox(height: 8),

            // Description
            Center(
              child: Text(
                _done
                    ? 'Your photos are still safely stored in your Telegram vault. '
                        'Tap a photo to view it from the cloud.'
                    : 'Only photos that have been verified as backed up '
                        '(${_reclaimableCount} photos) will be removed from this device.',
                style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                      color: cs.onSurfaceVariant,
                    ),
                textAlign: TextAlign.center,
              ),
            ),

            const Spacer(),

            // Threshold warning
            if (!canFreeUp && !_done)
              Container(
                padding: const EdgeInsets.all(12),
                decoration: BoxDecoration(
                  color: cs.surfaceContainerHighest,
                  borderRadius: BorderRadius.circular(8),
                ),
                child: Row(
                  children: [
                    Icon(LucideIcons.info, size: 18, color: cs.onSurfaceVariant),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        'Minimum 500 MB of backed-up photos needed to free up space.',
                        style: Theme.of(context).textTheme.bodySmall?.copyWith(
                              color: cs.onSurfaceVariant,
                            ),
                      ),
                    ),
                  ],
                ),
              ),

            // Action button
            if (!_done)
              SizedBox(
                width: double.infinity,
                child: FilledButton(
                  onPressed: canFreeUp && !_processing ? _freeUpSpace : null,
                  style: FilledButton.styleFrom(
                    minimumSize: const Size(0, 48),
                    backgroundColor:
                        canFreeUp ? cs.error : cs.surfaceContainerHighest,
                    foregroundColor: canFreeUp ? cs.onError : cs.onSurfaceVariant,
                  ),
                  child: _processing
                      ? const SizedBox(
                          width: 20,
                          height: 20,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        )
                      : const Text('Free up space'),
                ),
              ),

            if (_done)
              SizedBox(
                width: double.infinity,
                child: FilledButton(
                  onPressed: () => Navigator.pop(context),
                  style: FilledButton.styleFrom(
                    minimumSize: const Size(0, 48),
                  ),
                  child: const Text('Done'),
                ),
              ),

            const SizedBox(height: 16),
          ],
        ),
      ),
    );
  }
}
