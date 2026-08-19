import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

/// Status badge per photo (PRD Part 2 S3.1):
/// Uses Lucide icons + semantic Material 3 colors.
enum PhotoStatus { backedUp, uploading, failed, localOnly }

class StatusBadge extends StatelessWidget {
  const StatusBadge({super.key, required this.status});

  final PhotoStatus status;

  @override
  Widget build(BuildContext context) {
    final (icon, color, tooltip) = switch (status) {
      PhotoStatus.backedUp => (
          LucideIcons.cloud,
          Theme.of(context).colorScheme.primary,
          'Backed up',
        ),
      PhotoStatus.uploading => (
          LucideIcons.upload,
          Theme.of(context).colorScheme.tertiary,
          'Uploading...',
        ),
      PhotoStatus.failed => (
          LucideIcons.circleAlert,
          Theme.of(context).colorScheme.error,
          'Retry',
        ),
      PhotoStatus.localOnly => (
          LucideIcons.smartphone,
          Theme.of(context).colorScheme.outline,
          'On this device',
        ),
    };

    return Tooltip(
      message: tooltip,
      child: Container(
        padding: const EdgeInsets.all(3),
        decoration: BoxDecoration(
          color: Theme.of(context).colorScheme.surface.withValues(alpha: 0.85),
          shape: BoxShape.circle,
          border: Border.all(
            color: color.withValues(alpha: 0.3),
            width: 1,
          ),
        ),
        child: status == PhotoStatus.uploading
            ? _UploadingIndicator(color: color)
            : Icon(icon, size: 14, color: color),
      ),
    );
  }
}

class _UploadingIndicator extends StatefulWidget {
  const _UploadingIndicator({required this.color});

  final Color color;

  @override
  State<_UploadingIndicator> createState() => _UploadingIndicatorState();
}

class _UploadingIndicatorState extends State<_UploadingIndicator>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 1200),
  )..repeat();

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: _controller,
      builder: (context, _) => Transform.rotate(
        angle: _controller.value * 2 * 3.14159,
        child: Icon(LucideIcons.refreshCw, size: 14, color: widget.color),
      ),
    );
  }
}

/// Map a core `syncStatus` string to a [PhotoStatus].
PhotoStatus photoStatusFromSync(String syncStatus) {
  switch (syncStatus.toUpperCase()) {
    case 'BACKED_UP':
      return PhotoStatus.backedUp;
    case 'UPLOADING' || 'PENDING' || 'QUEUED':
      return PhotoStatus.uploading;
    case 'FAILED' || 'ERROR':
      return PhotoStatus.failed;
    default:
      return PhotoStatus.localOnly;
  }
}
