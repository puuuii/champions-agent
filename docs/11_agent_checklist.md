# 11. AI エージェント向け実装チェックリスト

## この文書の範囲

この文書は、AI エージェントが現行システムを v3 設計へ移行する際の作業チェックリストである。設計理由の詳細は `02_decisions_and_principles.md`、移行順は `09_migration_plan.md` を参照する。

## 最優先ルール

作業中に迷った場合は、この順に優先する。

1. `champions-domain` を外部技術から守る。
2. `champions-application` を `champions-interface` から守る。
3. `champions-runtime` を `champions-infrastructure` から守る。
4. `opencv::core::Mat` を infrastructure 外へ漏らさない。
5. UI thread で blocking / heavy 処理をしない。
6. preview frame と full frame を unbounded queue に積まない。
7. `resources/` に user-writable data を置かない。
8. 各 phase 終了時点で `cargo check --workspace` を通す。

## 実装順チェックリスト

### P1. Workspace skeleton

- [ ] root `Cargo.toml` を workspace 化した。
- [ ] `apps/desktop` を作った。
- [ ] `crates/champions-domain` を作った。
- [ ] `crates/champions-application` を作った。
- [ ] `crates/champions-infrastructure` を作った。
- [ ] `crates/champions-runtime` を作った。
- [ ] `crates/champions-interface` を作った。
- [ ] `mod.rs` を作っていない。
- [ ] `cargo check --workspace` が通る。

### P2. Domain damage

- [ ] `src/damage/calc.rs` の計算本体を domain battle へ移した。
- [ ] `DamageArgs` を `DamageInput` に寄せた。
- [ ] file I/O を domain に入れていない。
- [ ] 既存 damage regression test 相当が通る。
- [ ] `champions-domain/Cargo.toml` に `iced`, `opencv`, `ort`, `manga-ocr-rs`, `reqwest`, `csv` がない。

### P3. Repository / AppPaths

- [ ] `AppPaths` を `champions-infrastructure/src/config.rs` に作った。
- [ ] `resources`, `user_data`, `cache`, `debug` を分離した。
- [ ] `JsonPartyRepository` を作った。
- [ ] `JsonUsageRepository` を作った。
- [ ] `CsvCatalogRepository` を作った。
- [ ] `party.json` は `user_data` に保存される。
- [ ] `usage.json` は `cache` に保存される。
- [ ] JSON 保存は atomic write である。

### P4. Application use cases

- [ ] `LoadPartyUseCase` を作った。
- [ ] `SavePartyUseCase` を作った。
- [ ] `SuggestNamesUseCase` を作った。
- [ ] `CalculateDamageUseCase` を作った。
- [ ] `DetectSelectionScreenUseCase` を作った。
- [ ] `IdentifyOpponentPartyUseCase` を作った。
- [ ] `RefreshUsageDataUseCase` を作った。
- [ ] `GetPokemonUsageUseCase` を作った。
- [ ] use case output に `champions-interface` の型を使っていない。
- [ ] port trait は原則 sync である。

### P5. UI decoupling

- [ ] `PokeEditorApp` から `std::fs::read_to_string` を削除した。
- [ ] `PokeEditorApp` から `std::fs::write` を削除した。
- [ ] `PokemonState` から `MasterData` を削除した。
- [ ] サジェストは `SuggestNamesUseCase` 経由になった。
- [ ] UI state は入力途中 state と domain model を混同していない。
- [ ] `apps/desktop/src/pages`, `components`, `state`, `app.rs` から `champions_infrastructure` import がない。

### P6. Runtime skeleton

- [ ] `RuntimeHandle` を作った。
- [ ] `RuntimeBuilder` を作った。
- [ ] `FrameSource` trait を runtime に作った。
- [ ] `PreviewFrameConverter` trait を runtime に作った。
- [ ] `RecognitionImageExtractor` trait を runtime に作った。
- [ ] `CapturedFrame` は owned bytes を持ち、`Mat` を含まない。
- [ ] preview stream と runtime event stream が分かれている。
- [ ] shutdown test が通る。

### P7. Iced preview

- [ ] OpenCV capture は infrastructure adapter にある。
- [ ] `Mat` は infrastructure adapter 内で owned bytes に変換される。
- [ ] HighGUI は通常起動 path から消えている。
- [ ] preview は Iced component で表示される。
- [ ] preview は latest-only / drop old policy である。
- [ ] preview frame が runtime event channel に混ざっていない。

### P8. Recognition integration

- [ ] `MangaOcrEngine` が `OcrEngine` port を実装している。
- [ ] `OnnxPartyIdentifier` が `PartyIdentifier` port を実装している。
- [ ] `OpenCvCropper` が target_text と opponent slots を抽出できる。
- [ ] `RecognitionScheduler` が OCR / DINOv2 の頻度を制御する。
- [ ] 選出画面に入ったときだけ DINOv2 が走る。
- [ ] confidence threshold 未満は unknown になる。
- [ ] top candidates を保持している。
- [ ] duplicate conflict を表現している。
- [ ] usage がなくても recognized slot は表示できる。

### P9. Cleanup / CI

- [ ] 旧 `src/party`, `src/damage`, `src/domain`, `src/ui` の未使用 module を削除した。
- [ ] 仮実装 `OnnxPartyIdentifier` を削除または本実装に置換した。
- [ ] `cargo fmt --check` が通る。
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` が通る。
- [ ] `cargo test --workspace` が通る。
- [ ] UI import guard が通る。
- [ ] `Mat` leak guard が通る。
- [ ] forbidden dependency guard が通る。

## Do / Don't

| Do | Don't |
|---|---|
| `apps/desktop/src/composition.rs` で adapter を組み立てる | `pages/*` から repository を直接呼ぶ |
| use case input/output を `champions-application` に置く | application use case に `PartyView` を返させる |
| `ImageBuffer` で OCR / ONNX port に画像を渡す | `OcrImageRef<'_>` や `Mat` を application に渡す |
| preview と runtime event を分ける | `RuntimeEvent::PreviewFrame` に全部混ぜる |
| `FrameSequence`, `EventSequence`, `RecognitionAttemptId` を分ける | `sequence: u64` だけで全部管理する |
| `party.json` を user data に置く | `resources/master_data/mypoke/party.json` に保存する |
| `usage.json` を cache に置く | refresh 結果を bundled resources に書く |
| `tracing` で log を出す | worker で `println!` と `process::exit` を使う |
| fake adapter で runtime test を書く | OpenCV camera がないと runtime test できない設計にする |

## Forbidden import checklist

### `champions-domain`

- [ ] `std::fs` なし
- [ ] `csv` なし
- [ ] `serde_json` なし
- [ ] `opencv` なし
- [ ] `iced` なし
- [ ] `ort` なし
- [ ] `manga_ocr_rs` なし
- [ ] `reqwest` なし

### `champions-application`

- [ ] `champions_interface` なし
- [ ] `iced` なし
- [ ] `opencv` なし
- [ ] `ort` なし
- [ ] `manga_ocr_rs` なし
- [ ] direct `reqwest` なし
- [ ] path 定数直書きなし

### `champions-runtime`

- [ ] `champions_infrastructure` なし
- [ ] `opencv` なし
- [ ] `ort` なし
- [ ] `manga_ocr_rs` なし
- [ ] `iced::widget` なし

### `apps/desktop` UI modules

以下に `champions_infrastructure` import がないこと。

- [ ] `app.rs`
- [ ] `message.rs`
- [ ] `mapping.rs`
- [ ] `subscriptions.rs`
- [ ] `state/*`
- [ ] `pages/*`
- [ ] `components/*`

## 現行コードを読む時の注意

| 現行箇所 | 注意 |
|---|---|
| `src/main.rs` | 本体処理が集約されている。移植時は capture、recognition、usage lookup、UI 起動に分解する |
| `src/application/party_service.rs` | 設計意図の断片だが、本 pipeline ではない |
| `src/infrastructure/party_identifier_impl.rs` | 仮実装。ONNX 実装の移植元ではない |
| `src/party/identifier.rs` | DINOv2 ONNX の本実装。移植元として重要 |
| `src/party/cutout.rs` | crop 比率を維持する |
| `src/ui/app.rs` | UI 表示は参考になるが、保存・DTO・subscription は分離する |
| `src/domain/master_data.rs` | domain に置かず repository へ移す |

## 完了時の最終確認

```text
[ ] アプリが起動する
[ ] パーティ編集画面が表示される
[ ] party save/load が user_data で動く
[ ] サジェストが use case 経由で動く
[ ] Iced preview が表示される
[ ] HighGUI window が出ない
[ ] 選出画面で OCR 判定が動く
[ ] 選出画面 entry 時だけ DINOv2 が動く
[ ] usage 情報が表示される
[ ] usage がない場合も認識名と confidence が表示される
[ ] shutdown が graceful に完了する
[ ] cargo check/test/clippy が通る
[ ] CI guard が通る
```
