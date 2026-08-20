import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

/// Task Progress Hub (PRD Part 2 S6.1, S9.2).
///
/// Shows all running/paused/completed/failed tasks with progress bars,
/// pause/resume/retry/cancel actions. Accessible from Photos tab banner.
class ProgressHubScreen extends StatelessWidget {
  const ProgressHubScreen({super.key});

  @override
  Widget build(BuildContext context) {
    // TODO: Wire to real TaskHub event stream from Rust
    // For now, show empty state
    return Scaffold(
      appBar: AppBar(title: const Text('Tasks')),
      body: _EmptyHub(),
    );
  }
}

class _EmptyHub extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(32),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              LucideIcons.listChecks,
              size: 64,
              color: Theme.of(context).colorScheme.outline,
            ),
            const SizedBox(height: 16),
            Text(
              'No active tasks',
              style: Theme.of(context).textTheme.headlineSmall,
            ),
            const SizedBox(height: 8),
            Text(
              'Upload progress, scans, and other background tasks will appear here.',
              style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                    color: Theme.of(context).colorScheme.onSurfaceVariant,
                  ),
              textAlign: TextAlign.center,
            ),
          ],
        ),
      ),
    );
  }
}

/// A single task row in the Progress Hub.
class TaskRow extends StatelessWidget {
  const TaskRow({
    required this.title,
    required this.progress, // 0.0 - 1.0
    required this.status, // running, paused, completed, failed
    this.subtitle,
    this.onRetry,
    this.onPause,
    this.onCancel,
  });

  final String title;
  final double progress;
  final String status;
  final String? subtitle;
  final VoidCallback? onRetry;
  final VoidCallback? onPause;
  final VoidCallback? onCancel;

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;
    final isRunning = status == 'running';
    final isFailed = status == 'failed';

    return Card(
      margin: const EdgeInsets.symmetric(horizontal: 16, vertical: 4),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Icon(
                  isFailed
                      ? LucideIcons.circleAlert
                      : isRunning
                          ? LucideIcons.loader
                          : LucideIcons.circleCheck,
                  size: 18,
                  color: isFailed
                      ? cs.error
                      : isRunning
                          ? cs.primary
                          : cs.outline,
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    title,
                    style: Theme.of(context).textTheme.titleSmall,
                  ),
                ),
                if (subtitle != null)
                  Text(
                    subtitle!,
                    style: Theme.of(context).textTheme.bodySmall?.copyWith(
                          color: cs.onSurfaceVariant,
                        ),
                  ),
              ],
            ),
            const SizedBox(height: 8),
            LinearProgressIndicator(
              value: progress,
              backgroundColor: cs.surfaceContainerHighest,
              color: isFailed ? cs.error : cs.primary,
            ),
            const SizedBox(height: 8),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                if (isFailed && onRetry != null)
                  TextButton.icon(
                    onPressed: onRetry,
                    icon: const Icon(LucideIcons.refreshCw, size: 16),
                    label: const Text('Retry'),
                  ),
                if (isRunning && onPause != null)
                  TextButton.icon(
                    onPressed: onPause,
                    icon: const Icon(LucideIcons.pause, size: 16),
                    label: const Text('Pause'),
                  ),
                if (!isRunning &&
                    !isFailed &&
                    onCancel != null)
                  TextButton.icon(
                    onPressed: onCancel,
                    icon: const Icon(LucideIcons.x, size: 16),
                    label: const Text('Cancel'),
                  ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}
