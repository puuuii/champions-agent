# 04. Workspace とディレクトリ構造

## この文書の範囲

この文書は、新しいファイル配置、workspace 構成、Rust 2024 module 配置、resource / user data / cache の配置を定義する。crate の依存契約は `05_crate_contracts.md` を正とする。

## Workspace ルート

```text
champions-agent/
├── Cargo.toml
├── apps/
│   └── desktop/
├── crates/
│   ├── champions-domain/
│   ├── champions-application/
│   ├── champions-infrastructure/
│   ├── champions-runtime/
│   └── champions-interface/
├── resources/
│   ├── master_data/
│   ├── models/
│   ├── pokemon_images/
│   └── usage_seed.json
├── assets/
│   └── fonts/
├── tools/
├── tests/
├── docs/
└── .github/
    └── workflows/
```

`apps/backend` は初期実装では作らない。将来、外部 client や headless API が必要になった場合だけ追加する。

## 実行時データディレクトリ

`user_data/`、`cache/`、`debug/` は repository 内に固定配置するとは限らない。開発時は project root 配下、配布時は OS 標準の user data directory を使えるように `AppPaths` で解決する。

```text
# 開発時の既定例
champions-agent/
├── user_data/
│   └── party.json
├── cache/
│   └── usage.json
└── debug/
    ├── captures/
    └── recognition/
```

| Directory | 性質 | 書き込み | 例 |
|---|---|---:|---|
| `resources/` | bundled read-only data | 原則不可 | CSV master、model、pokemon images |
| `assets/` | UI asset | 原則不可 | font |
| `user_data/` | ユーザー編集データ | 可 | `party.json` |
| `cache/` | 再取得可能な cache | 可 | `usage.json` |
| `debug/` | debug output | 可 | crop image、capture snapshot |
| `tools/` | 開発用 script | 実行時依存なし | image download、ONNX export |

## ルート `Cargo.toml`

```toml
[workspace]
resolver = "2"
members = [
    "apps/desktop",
    "crates/champions-domain",
    "crates/champions-application",
    "crates/champions-infrastructure",
    "crates/champions-runtime",
    "crates/champions-interface",
]

[workspace.package]
edition = "2024"
version = "0.1.0"

[workspace.dependencies]
anyhow = "1"
thiserror = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time"] }
```

依存 version の最終確定は実装時に行う。重要なのは、全 crate で同一 dependency を重複定義しないことである。

## Rust 2024 module ルール

`mod.rs` は使わない。親 module は `foo.rs`、子 module は `foo/bar.rs` に置く。

```text
src/
├── lib.rs
├── battle.rs
└── battle/
    ├── damage_calculator.rs
    ├── damage_formula.rs
    └── stat.rs
```

親 module `battle.rs` の例。

```rust
mod damage_calculator;
mod damage_formula;
mod stat;

pub use damage_calculator::DamageCalculator;
pub use stat::Stat;
```

## `apps/desktop`

```text
apps/desktop/
├── Cargo.toml
└── src/
    ├── main.rs
    ├── composition.rs
    ├── app.rs
    ├── message.rs
    ├── theme.rs
    ├── font.rs
    ├── state.rs
    ├── state/
    │   ├── app_state.rs
    │   ├── party_editor_state.rs
    │   ├── pokemon_form_state.rs
    │   ├── preview_state.rs
    │   └── selection_support_state.rs
    ├── pages.rs
    ├── pages/
    │   ├── party_editor.rs
    │   └── selection_support.rs
    ├── components.rs
    ├── components/
    │   ├── pokemon_form.rs
    │   ├── usage_table.rs
    │   ├── video_preview.rs
    │   └── status_bar.rs
    ├── mapping.rs
    └── subscriptions.rs
```

| ファイル | 役割 | infrastructure import |
|---|---|---:|
| `main.rs` | process entrypoint。`composition::build_app()` を呼ぶ | 可 |
| `composition.rs` | AppPaths、repository、adapter、use case、runtime、Iced app を組み立てる | 可 |
| `app.rs` | Iced app root。`update`、`view`、`subscription` を持つ | 不可 |
| `message.rs` | UI message を集約する | 不可 |
| `mapping.rs` | UI state と application input/output の変換 | 不可 |
| `subscriptions.rs` | runtime stream を Iced subscription に接続 | 不可 |
| `state/*` | 画面状態 | 不可 |
| `pages/*` | page 表示 | 不可 |
| `components/*` | component 表示 | 不可 |

## `champions-domain`

```text
crates/champions-domain/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── battle.rs
    ├── battle/
    │   ├── damage_calculator.rs
    │   ├── damage_formula.rs
    │   ├── damage_input.rs
    │   ├── damage_roll.rs
    │   ├── stat.rs
    │   ├── stat_stage.rs
    │   └── type_effectiveness.rs
    ├── party.rs
    ├── party/
    │   ├── pokemon_build.rs
    │   ├── saved_party.rs
    │   ├── effort_value_spread.rs
    │   └── move_set.rs
    ├── catalog.rs
    ├── catalog/
    │   ├── species_id.rs
    │   ├── pokemon_species.rs
    │   ├── move_data.rs
    │   ├── item.rs
    │   ├── nature.rs
    │   ├── ability.rs
    │   └── battle_master_data.rs
    ├── recognition.rs
    ├── recognition/
    │   ├── selection_slot.rs
    │   ├── screen_state.rs
    │   ├── confidence_score.rs
    │   ├── recognition_candidate.rs
    │   ├── recognized_pokemon.rs
    │   └── recognized_party.rs
    ├── usage.rs
    └── usage/
        ├── pokemon_usage_summary.rs
        ├── move_usage.rs
        ├── item_usage.rs
        ├── effort_value_usage.rs
        └── nature_usage.rs
```

## `champions-application`

```text
crates/champions-application/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── image.rs
    ├── errors.rs
    ├── ports.rs
    ├── ports/
    │   ├── catalog_repository.rs
    │   ├── party_repository.rs
    │   ├── usage_repository.rs
    │   ├── ocr_engine.rs
    │   ├── party_identifier.rs
    │   └── usage_fetcher.rs
    ├── use_cases.rs
    ├── use_cases/
    │   ├── load_party.rs
    │   ├── save_party.rs
    │   ├── suggest_names.rs
    │   ├── calculate_damage.rs
    │   ├── detect_selection_screen.rs
    │   ├── identify_opponent_party.rs
    │   ├── refresh_usage_data.rs
    │   └── get_pokemon_usage.rs
    └── io.rs
    └── io/
        ├── party_io.rs
        ├── suggestion_io.rs
        ├── damage_io.rs
        ├── recognition_io.rs
        └── usage_io.rs
```

`io/*` は use case の input / output 型を置く。`champions-interface` の view model は置かない。

## `champions-infrastructure`

```text
crates/champions-infrastructure/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── config.rs
    ├── fs_utils.rs
    ├── persistence.rs
    ├── persistence/
    │   ├── csv_catalog_repository.rs
    │   ├── json_party_repository.rs
    │   ├── json_usage_repository.rs
    │   └── atomic_write.rs
    ├── capture.rs
    ├── capture/
    │   ├── capture_config.rs
    │   └── opencv_capture.rs
    ├── vision.rs
    ├── vision/
    │   ├── cropper.rs
    │   ├── frame_converter.rs
    │   ├── manga_ocr_engine.rs
    │   └── onnx_party_identifier.rs
    └── external.rs
    └── external/
        └── gamewith_usage_client.rs
```

`highgui` は通常実装に置かない。性能比較用に残す場合は feature `debug-highgui` で隔離する。

## `champions-runtime`

```text
crates/champions-runtime/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── builder.rs
    ├── handle.rs
    ├── traits.rs
    ├── command_loop.rs
    ├── shutdown.rs
    ├── frame.rs
    ├── latest.rs
    ├── scheduler.rs
    ├── streams.rs
    ├── workers.rs
    └── workers/
        ├── capture_worker.rs
        ├── preview_worker.rs
        └── recognition_worker.rs
```

| ファイル | 役割 |
|---|---|
| `traits.rs` | `FrameSource`, `PreviewFrameConverter`, `RecognitionImageExtractor` など runtime adapter trait |
| `frame.rs` | `CapturedFrame`。owned bytes を持ち、`Mat` は含めない |
| `latest.rs` | latest-only slot / channel helper |
| `streams.rs` | preview stream と runtime event stream の公開 wrapper |
| `scheduler.rs` | OCR / DINOv2 実行頻度と状態遷移を制御する |
| `workers/*` | 長寿命 worker 本体 |

## `champions-interface`

```text
crates/champions-interface/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── ids.rs
    ├── command.rs
    ├── event.rs
    ├── error.rs
    ├── preview.rs
    ├── image_geometry.rs
    └── recognition_view.rs
```

この crate は HTTP DTO ではない。UI と runtime の共有 boundary model だけを置く。

置かないもの:

```text
PartyInput
PartyEditorState
DamageRequestView
DamageResultView
SuggestionList UI state
Iced widget 型
OpenCV Mat
repository 型
```

## `resources`

```text
resources/
├── master_data/
│   ├── pokemon_stats.csv
│   ├── pokemon_types.csv
│   ├── moves.csv
│   ├── move_names.csv
│   ├── move_meta_stat_changes.csv
│   ├── type_efficacy.csv
│   ├── natures.csv
│   ├── nature_names.csv
│   ├── item_names.csv
│   └── ability_names.csv
├── pokemon_images/
│   └── *.png
├── models/
│   ├── dinov2_vits14.onnx
│   └── manga-ocr/
└── usage_seed.json
```

`resources/master_data/mypoke/party.json` は作らない。`resources/master_data/usage.json` も通常は作らない。初回起動用の usage が必要な場合は `resources/usage_seed.json` を cache にコピーする。

## `tools`

```text
tools/
├── download_pokemon_images.py
└── export_dino.py
```

既存の `scripts/` は `tools/` に移す。アプリ実行時 dependency ではない。

## `tests`

```text
tests/
├── fixtures/
│   ├── master_data/
│   ├── party/
│   └── usage/
├── integration_damage.rs
├── integration_party_repository.rs
└── integration_usage_repository.rs
```

unit test は各 crate 内に置く。cross-crate の確認だけ `tests/` に置く。
