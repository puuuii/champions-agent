#!/usr/bin/env python3
"""
画像内の日本語テキストをローカルモデル (manga-ocr) で認識するスクリプト。
API 呼び出し不要・完全ローカル動作。

インストール:
    pip install manga-ocr

使い方:
    python ocr_japanese_local.py <画像パス>
"""

import sys
from pathlib import Path


SUPPORTED_SUFFIXES = {".jpg", ".jpeg", ".png", ".gif", ".webp", ".bmp"}


def recognize_japanese(image_path: str) -> str:
    """
    画像内の日本語テキストを認識して返す。

    Parameters
    ----------
    image_path : str
        認識対象の画像ファイルパス

    Returns
    -------
    str
        認識されたテキスト
    """
    from manga_ocr import MangaOcr
    from PIL import Image

    path = Path(image_path)
    if not path.exists():
        raise FileNotFoundError(f"ファイルが見つかりません: {image_path}")
    if path.suffix.lower() not in SUPPORTED_SUFFIXES:
        raise ValueError(f"未対応のフォーマット: {path.suffix}")

    print("モデルをロード中... (初回は数十秒かかります)")
    ocr = MangaOcr()

    image = Image.open(path)
    result = ocr(image)
    return result


def main():
    if len(sys.argv) < 2:
        print("使い方: python ocr_japanese_local.py <画像パス>")
        sys.exit(1)

    image_path = sys.argv[1]
    print(f"認識中: {image_path}")

    result = recognize_japanese(image_path)

    print("\n=== 認識結果 ===")
    print(result)


if __name__ == "__main__":
    main()
