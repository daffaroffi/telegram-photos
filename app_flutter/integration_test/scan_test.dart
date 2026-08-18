import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:path_provider/path_provider.dart';
import 'package:telegram_photos/src/platform/media_scan.dart';
import 'package:telegram_photos/src/rust/api/db.dart' as core;
import 'package:telegram_photos/src/rust/frb_generated.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('scan MediaStore and import into core', (tester) async {
    await RustLib.init();
    final dir = await getApplicationSupportDirectory();
    core.initCore(dbPath: '${dir.path}/telegram_photos.db');

    final json = await MediaScan.scanGalleryJson();
    expect(json, isNotEmpty);
    expect(json, startsWith('['));

    final added = core.importScanResults(json: json);
    // Emulator has 26 seeded images; accept any sane positive count.
    expect(added, greaterThan(0));

    final count = core.countMedia();
    expect(count, greaterThan(0));
  });
}
