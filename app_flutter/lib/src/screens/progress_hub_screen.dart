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

// TODO(feat): wire the Progress Hub to the Rust TaskHub event stream
// (see task_hub.rs) and reintroduce a TaskRow widget. Until then the
// screen only shows its empty state.
