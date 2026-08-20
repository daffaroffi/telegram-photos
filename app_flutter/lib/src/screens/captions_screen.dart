import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../rust/api/db.dart' as core;
import '../rust/api/mirror.dart';

/// Captions and hashtags editing panel (PRD Part 2 S6.3).
///
/// Edit caption text, add #tags, apply to multi-item.
/// Caption syncs as Telegram message caption via editMessageCaption.
class CaptionsScreen extends StatefulWidget {
  const CaptionsScreen({super.key, required this.item});

  final MediaItem item;

  @override
  State<CaptionsScreen> createState() => _CaptionsScreenState();
}

class _CaptionsScreenState extends State<CaptionsScreen> {
  late TextEditingController _captionCtrl;
  bool _saving = false;

  @override
  void initState() {
    super.initState();
    final existing = core.getCaption(mediaId: widget.item.id);
    _captionCtrl = TextEditingController(text: existing ?? '');
  }

  @override
  void dispose() {
    _captionCtrl.dispose();
    super.dispose();
  }

  Future<void> _saveCaption() async {
    if (_saving) return;
    setState(() => _saving = true);

    try {
      final text = _captionCtrl.text.trim();
      core.saveCaption(mediaId: widget.item.id, text: text);

      HapticFeedback.lightImpact();
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Caption saved')),
        );
        Navigator.of(context).pop(true);
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Error: $e')),
        );
      }
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;

    return Container(
      padding: EdgeInsets.fromLTRB(
        16,
        16,
        16,
        16 + MediaQuery.of(context).viewInsets.bottom,
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // Handle bar
          Center(
            child: Container(
              width: 40,
              height: 4,
              decoration: BoxDecoration(
                color: cs.outlineVariant,
                borderRadius: BorderRadius.circular(2),
              ),
            ),
          ),
          const SizedBox(height: 16),

          // Title
          Row(
            children: [
              Icon(LucideIcons.penLine, size: 18, color: cs.primary),
              const SizedBox(width: 8),
              Text(
                'Edit caption',
                style: Theme.of(context).textTheme.titleMedium,
              ),
            ],
          ),
          const SizedBox(height: 16),

          // Caption text field
          TextField(
            controller: _captionCtrl,
            maxLines: 3,
            minLines: 2,
            decoration: InputDecoration(
              hintText: 'Add a caption...',
              border: const OutlineInputBorder(),
              contentPadding: const EdgeInsets.all(12),
            ),
          ),
          const SizedBox(height: 12),

          // Hashtag support exists in the DB layer (caption_tags table,
          // addCaptionTag / searchByHashtag) but no per-media list API
          // is exposed yet, so the chips would just keep adding tags
          // blindly. Show a placeholder until the Rust side exposes
          // list_caption_tags and the FRB binding is regenerated.
          Text(
            'Hashtags coming in a follow-up',
            style: Theme.of(context).textTheme.bodySmall?.copyWith(
                  color: cs.onSurfaceVariant,
                ),
          ),
          const SizedBox(height: 16),

          // Save button
          SizedBox(
            width: double.infinity,
            child: FilledButton(
              onPressed: _saving ? null : _saveCaption,
              style: FilledButton.styleFrom(
                minimumSize: const Size(0, 48),
              ),
              child: _saving
                  ? const SizedBox(
                      width: 20,
                      height: 20,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Text('Save caption'),
            ),
          ),
        ],
      ),
    );
  }
}
