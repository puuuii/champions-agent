# champions-agent

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

## 情報源
- csv: https://github.com/PokeAPI/pokeapi/tree/master/data/v2/csv
- ダメージ計算: https://champsone.com/#/articles/damage-formula
- manga-ocr: https://huggingface.co/l0wgear/manga-ocr-2025-onnx/tree/main

## 外部データ準備手順

1. uv venv
2. .venv\Scripts\activate
3. python .\scripts\export_dino.py
4. python .\scripts\refresh_usage_data.py
