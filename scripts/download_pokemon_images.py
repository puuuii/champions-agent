"""
usage.json から img_url を取得し、
master_data/pokemon_images/<name>.png としてダウンロードするスクリプト
RGBA画像は白背景で合成して非透過PNGとして保存する
"""

import json
import time
import urllib.request
import urllib.error
from io import BytesIO
from pathlib import Path

from PIL import Image

INPUT_JSON = "master_data/usage.json"
OUTPUT_DIR = Path("master_data/pokemon_images")
DELAY_SEC = 0.5  # サーバー負荷軽減のためのウェイト


def sanitize_filename(name: str) -> str:
    """ファイル名に使えない文字を除去する"""
    invalid_chars = r'\/:*?"<>|'
    for c in invalid_chars:
        name = name.replace(c, "_")
    return name


def download_image(url: str) -> bytes | None:
    """画像をダウンロードして bytes を返す"""
    req = urllib.request.Request(
        url,
        headers={"User-Agent": "Mozilla/5.0 (compatible; PokemonImageDownloader/1.0)"},
    )

    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            return resp.read()

    except urllib.error.HTTPError as e:
        print(f"  [HTTP {e.code}] {url}")

    except urllib.error.URLError as e:
        print(f"  [URL Error] {e.reason} - {url}")

    except Exception as e:
        print(f"  [Error] {e} - {url}")

    return None


def save_as_non_transparent_png(image_bytes: bytes, dest: Path):
    """
    透過画像なら白背景で合成して非透過PNG保存
    """

    img = Image.open(BytesIO(image_bytes))

    # RGBA / LA / P(透過あり) 対応
    if img.mode in ("RGBA", "LA") or (
        img.mode == "P" and "transparency" in img.info
    ):
        rgba = img.convert("RGBA")

        # 白背景
        background = Image.new("RGB", rgba.size, (255, 255, 255))
        background.paste(rgba, mask=rgba.getchannel("A"))

        background.save(dest, format="PNG")

    else:
        img.convert("RGB").save(dest, format="PNG")


def main():
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    with open(INPUT_JSON, encoding="utf-8") as f:
        pokemon_list = json.load(f)

    total = len(pokemon_list)
    success, skip, fail = 0, 0, 0

    for i, entry in enumerate(pokemon_list, 1):
        name: str = entry.get("name", f"unknown_{i}")
        url: str = entry.get("img_url", "")

        if not url:
            print(f"[{i}/{total}] {name}: img_url が空のためスキップ")
            skip += 1
            continue

        filename = sanitize_filename(name) + ".png"
        dest = OUTPUT_DIR / filename

        if dest.exists():
            print(f"[{i}/{total}] {name}: 既存のためスキップ ({filename})")
            skip += 1
            continue

        print(f"[{i}/{total}] {name}: ダウンロード中 ...", end=" ")

        image_bytes = download_image(url)

        if image_bytes is None:
            fail += 1
            continue

        try:
            save_as_non_transparent_png(image_bytes, dest)

            size_kb = dest.stat().st_size / 1024
            print(f"OK ({size_kb:.1f} KB) -> {filename}")

            success += 1

        except Exception as e:
            print(f"  [Save Error] {e}")
            fail += 1

        time.sleep(DELAY_SEC)

    print(f"\n完了: 成功 {success} / スキップ {skip} / 失敗 {fail} (合計 {total})")


if __name__ == "__main__":
    main()