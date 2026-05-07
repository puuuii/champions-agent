# P0. Baseline Capture Record

## Baseline Commit

```
Branch: rearchitect
Commit: ac2060b feat: Add party saving and loading
Date: 2026-05-07
```

This commit is the frozen baseline of pre-migration functionality.

## Cargo Test Results

**Status: BLOCKED by network (SSL error)**

`cargo test` and `cargo check` both fail due to SSL certificate revocation check errors when fetching crates.io index. This is an environment-specific issue (corporate proxy / certificate policy on Windows 11 Enterprise).

The existing tests in the codebase are:

| File | Tests | Type |
|---|---|---|
| `src/damage.rs` | `test_damage_variations` (4 cases), `test_rank_stages` (5 cases), `test_error_cases` (3 cases), `test_master_data_integrity` | rstest parametrized, requires `master_data/` CSVs |
| `src/damage/models.rs` | `test_models_module` | placeholder |
| `src/damage/loader.rs` | `test_loader_module` | placeholder |

All damage regression tests depend on `master_data/` directory which is gitignored and must be present locally.

## Current App Status and Known Issues

### Functional status

| Feature | Status | Notes |
|---|---|---|
| Party editor UI | Working | Iced-based, loads/saves `master_data/mypoke/party.json` |
| Party save/load | Working | Uses `std::fs` directly in UI |
| OpenCV camera capture | Working | Requires camera + `CAP_V4L2` (Linux-only backend) |
| HighGUI preview window | Working | Separate window from Iced UI |
| OCR screen detection | Working | Manga OCR, detects selection screen |
| DINOv2 party identification | Working | ONNX model required in `models/` |
| Usage data display | Working | Loads from `master_data/usage.json` |
| MasterData suggestions | Working | CSV-based name suggestions |

### Known issues

| Issue | Description |
|---|---|
| HighGUI + Iced dual UI | Two separate windows, no unified lifecycle |
| `process::exit(0)` shutdown | No graceful shutdown from HighGUI 'q' key |
| `CAP_V4L2` hardcoded | Windows incompatible (Linux-only backend) |
| `capture.png` saved every frame | High I/O load, always writes to project root |
| path constants in `main.rs` | `CAPTURE_PATH`, `ONNX_PATH`, `MASTER_IMG_DIR`, `USAGE_JSON_PATH`, `MASTER_DATA_DIR` |
| UI holds `MasterData` directly | Tight coupling between UI and domain data |
| UI does `std::fs::write` for save | No use case separation |
| `domain::master_data` does file I/O | Domain purity violation |
| `PartyRepository` trait = identifier | Naming mismatch (trait does image matching, not persistence) |
| `src/infrastructure/party_identifier_impl.rs` | Stale stub, diverged from real impl in `src/party/identifier.rs` |
| lib name `usage_fetcher` | Misleading; lib exposes all modules |
| SSL/network issue | Cannot fetch crates.io in current environment |

## Current Assets Confirmation

| Asset | Path | Git Tracked | Filesystem Present |
|---|---|---|---|
| master_data (CSVs, images, usage.json) | `master_data/` | No (gitignored) | No (not on this machine) |
| ONNX models | `models/` | No (gitignored) | No (not on this machine) |
| Fonts | `assets/fonts/NotoSansJP-Regular.ttf` | Yes | Yes |
| Scripts | `scripts/download_pokemon_images.py`, `scripts/export_dino.py` | Yes | Yes |

### Notes on missing assets

`master_data/` and `models/` are gitignored and must be provisioned separately. The damage regression tests cannot run without `master_data/`. The recognition pipeline cannot run without `models/dinov2_vits14.onnx` and `models/manga-ocr/`.

## Source File Inventory

```
src/main.rs                              - Entry point, capture thread, worker thread, Iced startup
src/lib.rs                               - GameWith fetcher, usage types, module exports
src/application.rs                       - Module declaration
src/application/party_service.rs         - PartyIdentifierService orchestrator (unused in main pipeline)
src/damage.rs                            - Module + rstest regression tests
src/damage/calc.rs                       - Damage calculation formula (pure logic)
src/damage/loader.rs                     - CSV MasterData loader
src/damage/models.rs                     - DamageArgs, MasterData struct, CSV record types
src/domain.rs                            - Module declaration
src/domain/damage.rs                     - Domain damage re-export
src/domain/damage_calc.rs               - Domain damage_calc wrapper
src/domain/master_data.rs                - MasterData with suggest/load (file I/O in domain)
src/domain/party.rs                      - SavedParty, SavedPokemon, PartyRepository trait
src/domain/services.rs                   - PartyIdentifierService trait
src/infrastructure.rs                    - Module declaration
src/infrastructure/damage.rs             - Infrastructure damage module
src/infrastructure/damage/calc_impl.rs   - DamageCalculator impl
src/infrastructure/party_identifier_impl.rs - OnnxPartyIdentifier stub (diverged from real impl)
src/party.rs                             - Module declaration, PartyIdentifier re-export
src/party/cutout.rs                      - OpenCV Mat crop, ROI extraction
src/party/identifier.rs                  - DINOv2 ONNX real implementation
src/party/ocr.rs                         - Manga OCR adapter
src/ui.rs                                - Module declaration
src/ui/app.rs                            - Iced app, PokemonUsage DTO, tabs, save/load
src/ui/pokemon.rs                        - Pokemon form, MasterData suggestions
```
