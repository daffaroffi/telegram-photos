import 'package:flutter/material.dart';

/// Status badge per photo (PRD Part 2 §3.1):
/// 🔵 backed up · ⏳ uploading (animated) · ⚠ failed · 📱 local only.
enum PhotoStatus { backedUp, uploading, failed, localOnly }

class StatusBadge extends StatelessWidget {
  const StatusBadge({super.key, required this.status});

  final PhotoStatus status;

  @override
  Widget build(BuildContext context) {
    final (icon, color, tooltip) = switch (status) {
      PhotoStatus.backedUp => (
          Icons.cloud_done_outlined,
          const Color(0xFF2AABEE),
          'Backed up',
        ),
      PhotoStatus.uploading => (
          Icons.cloud_upload_outlined,
          const Color(0xFFFFB300),
          'Uploading…',
        ),
      PhotoStatus.failed => (Icons.error_outline, const Color(0xFFE53935), 'Retry'),
      PhotoStatus.localOnly => (Icons.smartphone, Colors.grey.shade600, 'On this device'),
    };

    return Tooltip(
      message: tooltip,
      child: Container(
        padding: const EdgeInsets.all(3),
        decoration: BoxDecoration(
          color: Colors.black.withValues(alpha: 0.45),
          shape: BoxShape.circle,
        ),
        child: status == PhotoStatus.uploading
            ? _UploadingIndicator(color: color)
            : Icon(icon, size: 16, color: color),
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
        child: const Icon(Icons.sync, size: 16),
      ),
    );
  }
}

/// Map a core `syncStatus` string to a [PhotoStatus].
/// Core statuses are uppercase (BACKED_UP / UPLOADING / PENDING / FAILED).
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
