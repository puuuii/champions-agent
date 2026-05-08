# champions-agent

ポケモンチャンピオンズ対戦支援エージェント。キャプチャ映像から選出画面を検出し、DINOv2 + OCR で相手パーティを識別してリアルタイムに使用率データを表示する。

## アーキテクチャ

Cargo workspace 構成。レイヤー分離により domain / application は外部技術に依存しない。

```
champions-agent/
├── apps/desktop/          # Iced GUI + OpenCV capture (binary)
├── crates/
│   ├── champions-domain/         # 純粋ドメインロジック (battle, party, recognition, usage)
│   ├── champions-application/    # use cases + port traits
│   ├── champions-infrastructure/ # 技術実装 (CSV, JSON, ONNX, OCR, OpenCV)
│   ├── champions-runtime/        # capture worker, scheduler, preview stream
│   └── champions-interface/      # commands, events, shared DTOs
├── resources/             # 読み取り専用バンドルデータ
├── user_data/             # ユーザー書き込み (party.json)
├── cache/                 # 再取得可能キャッシュ (usage.json)
└── docs/                  # 設計資料群
```

## 前提

wsl2で動かすためには事前に管理者権限のPowershellで次を実行する:

```powershell
usbipd list | Select-String "GC311G2" | ForEach-Object { $id = $_.ToString().Split(' ')[0]; usbipd attach --wsl --busid $id }
```

また種々の設定のためにwsl2で次を実行する:

```bash
export LIBGL_ALWAYS_SOFTWARE=1
export TRANSFORMERS_OFFLINE=1
export HF_DATASETS_OFFLINE=1
```

## ビルド

```bash
cargo check --workspace
cargo test --workspace
```

desktop binary のビルドには OpenCV と libclang が必要:

```bash
# LIBCLANG_PATH を設定後
cargo build -p champions-agent-desktop
```

## CI ガード

```bash
bash scripts/ci_guards.sh
```

forbidden dependency / UI import / Mat leak の 3 ガードを検証する。

## 情報源
- csv: https://github.com/PokeAPI/pokeapi/tree/master/data/v2/csv
- ダメージ計算: https://champsone.com/#/articles/damage-formula
