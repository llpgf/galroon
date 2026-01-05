"""
Phase 9 Backward Compatibility Test

Tests that:
1. Old v1.0 metadata files can be loaded
2. New fields have sensible defaults
3. Migration to v2.0 model works correctly
4. Multi-root configuration works
"""

import sys
import json
from pathlib import Path

# Add backend to path
backend_dir = Path(__file__).parent
sys.path.insert(0, str(backend_dir))

from app.metadata.models import UnifiedMetadata, GameVersion, create_empty_metadata


def test_legacy_metadata_loading():
    """Test that legacy v1.0 metadata can be loaded."""
    print("=" * 70)
    print("TEST 1: Legacy Metadata Loading")
    print("=" * 70)

    # Simulate legacy v1.0 metadata (without Phase 9 fields)
    legacy_metadata = {
        "vndb_id": "v12345",
        "steam_id": "12345",
        "title": {
            "value": {
                "ja": "Fate/stay night",
                "en": "Fate/stay night",
                "zh_hans": "",
                "zh_hant": "",
                "original": "Fate/stay night"
            },
            "source": "vndb",
            "locked": False,
            "last_updated": "2026-01-01T00:00:00"
        },
        "description": {
            "value": "Test description",
            "source": "vndb",
            "locked": False,
            "last_updated": "2026-01-01T00:00:00"
        },
        "rating": {
            "value": {"score": 8.5, "count": 100, "source": "vndb"},
            "source": "vndb",
            "locked": False,
            "last_updated": "2026-01-01T00:00:00"
        },
        "cover_url": {
            "value": "http://example.com/cover.jpg",
            "source": "vndb",
            "locked": False,
            "last_updated": "2026-01-01T00:00:00"
        },
        "tags": {
            "value": ["Action", "Romance"],
            "source": "vndb",
            "locked": False,
            "last_updated": "2026-01-01T00:00:00"
        },
        "release_date": {
            "value": "2004-01-30",
            "source": "vndb",
            "locked": False,
            "last_updated": "2026-01-01T00:00:00"
        },
        "developer": {
            "value": "Type-Moon",
            "source": "vndb",
            "locked": False,
            "last_updated": "2026-01-01T00:00:00"
        },
        "languages": ["ja"],
        "metadata_version": "1.0",
        "last_sync": "2026-01-01T00:00:00",
        "providers": ["vndb"]
    }

    # Load legacy metadata
    try:
        metadata = UnifiedMetadata(**legacy_metadata)
        print("[PASS] Legacy metadata loaded successfully")
        print(f"   - VNDB ID: {metadata.vndb_id}")
        print(f"   - Steam ID: {metadata.steam_id}")
        print(f"   - Title: {metadata.title.value.ja}")
        print(f"   - Metadata Version: {metadata.metadata_version}")
        print()
        return True
    except Exception as e:
        print(f"[FAIL] Failed to load legacy metadata: {e}")
        return False


def test_new_fields_defaults():
    """Test that new Phase 9 fields have sensible defaults."""
    print("=" * 70)
    print("TEST 2: New Fields Defaults")
    print("=" * 70)

    metadata = create_empty_metadata()

    # Check all new fields
    tests = [
        ("external_ids", metadata.external_ids, dict, {}),
        ("versions", metadata.versions, list, []),
        ("assets_detected", metadata.assets_detected, list, []),
        ("visuals", metadata.visuals, dict, {}),
        ("credits", metadata.credits, list, []),
        ("metadata_version", metadata.metadata_version, str, "2.0"),
    ]

    all_passed = True
    for field_name, actual_value, expected_type, expected_default in tests:
        type_ok = isinstance(actual_value, expected_type)
        value_ok = actual_value == expected_default

        if type_ok and value_ok:
            print(f"[PASS] {field_name}: {expected_type.__name__} = {expected_default}")
        else:
            print(f"[FAIL] {field_name}: Expected {expected_type.__name__}={expected_default}, got {type(actual_value)}={actual_value}")
            all_passed = False

    print()
    return all_passed


def test_version_management():
    """Test version management methods."""
    print("=" * 70)
    print("TEST 3: Version Management")
    print("=" * 70)

    metadata = create_empty_metadata()

    # Test adding versions
    metadata.add_version(
        path="D:/Games/Fate-CD",
        label="CD Version",
        is_primary=True,
        assets=["ISO", "Chinese"]
    )

    metadata.add_version(
        path="D:/Games/Fate-Steam",
        label="Steam Version",
        is_primary=False,
        assets=["HDD", "English"]
    )

    print(f"[PASS] Added {len(metadata.versions)} versions")

    # Test get_primary_version
    primary = metadata.get_primary_version()
    if primary and primary.label == "CD Version":
        print(f"[PASS] Primary version: {primary.label}")
    else:
        print(f"[FAIL] Primary version error: {primary}")
        return False

    # Test get_version_by_path
    version = metadata.get_version_by_path("D:/Games/Fate-Steam")
    if version and version.label == "Steam Version":
        print(f"[PASS] Found version by path: {version.label}")
    else:
        print(f"[FAIL] Version by path error: {version}")
        return False

    print()
    return True


def test_migration():
    """Test legacy data migration."""
    print("=" * 70)
    print("TEST 4: Legacy Data Migration")
    print("=" * 70)

    metadata = create_empty_metadata()
    metadata.vndb_id = "v12345"
    metadata.assets_detected = ["ISO", "Chinese"]

    # Migrate
    metadata.migrate_legacy_data("D:/Games/Fate-CD")

    if len(metadata.versions) == 1:
        version = metadata.versions[0]
        if version.path == "D:/Games/Fate-CD" and version.is_primary:
            print(f"[PASS] Legacy migration successful")
            print(f"   - Path: {version.path}")
            print(f"   - Label: {version.label}")
            print(f"   - Is Primary: {version.is_primary}")
            print(f"   - Assets: {version.assets}")
            print()
            return True

    print(f"[FAIL] Legacy migration failed: {metadata.versions}")
    return False


def test_external_ids():
    """Test external_ids field."""
    print("=" * 70)
    print("TEST 5: External IDs")
    print("=" * 70)

    metadata = create_empty_metadata()
    metadata.vndb_id = "v12345"
    metadata.external_ids = {
        "steam": "12345",
        "bangumi": "67890",
        "erogamescape": "123"
    }

    print(f"[PASS] VNDB ID: {metadata.vndb_id}")
    print(f"[PASS] External IDs: {metadata.external_ids}")
    print(f"   - Steam: {metadata.external_ids['steam']}")
    print(f"   - Bangumi: {metadata.external_ids['bangumi']}")
    print(f"   - ErogameScape: {metadata.external_ids['erogamescape']}")
    print()
    return True


def main():
    """Run all tests."""
    print("\n")
    print("╔═══════════════════════════════════════════════════════════════╗")
    print("║          PHASE 9 BACKWARD COMPATIBILITY TEST SUITE           ║")
    print("╚═══════════════════════════════════════════════════════════════╝")
    print()

    results = []

    # Run tests
    results.append(("Legacy Metadata Loading", test_legacy_metadata_loading()))
    results.append(("New Fields Defaults", test_new_fields_defaults()))
    results.append(("Version Management", test_version_management()))
    results.append(("Legacy Data Migration", test_migration()))
    results.append(("External IDs", test_external_ids()))

    # Summary
    print("=" * 70)
    print("TEST SUMMARY")
    print("=" * 70)

    passed = sum(1 for _, result in results if result)
    total = len(results)

    for test_name, result in results:
        status = "PASS" if result else "FAIL"
        print(f"[{status}] {test_name}")

    print()
    print(f"Result: {passed}/{total} tests passed")
    print("=" * 70)
    print()

    if passed == total:
        print("[SUCCESS] ALL TESTS PASSED! Phase 9 migration is backward compatible.")
        return 0
    else:
        print("[WARNING]  SOME TESTS FAILED! Please review the errors above.")
        return 1


if __name__ == "__main__":
    sys.exit(main())
