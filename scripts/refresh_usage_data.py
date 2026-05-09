#!/usr/bin/env python3
"""Fetch usage data from GameWith and write it to cache/usage.json."""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import tempfile
from pathlib import Path
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

GAMEWITH_URL = "https://gamewith.jp/pokemon-champions/555373"
USER_AGENT = "Mozilla/5.0"

JS_OBJ_RE = re.compile(r"(?s)const pkchPokemonData\s*=\s*(\{.*?\});")
KEY_RE = re.compile(r"\b([A-Za-z_]\w*)\s*:")
TRAILING_RE = re.compile(r",\s*([\]}])")

REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT_PATH = REPO_ROOT / "cache" / "usage.json"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Fetch usage data from GameWith and write usage.json."
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=DEFAULT_OUTPUT_PATH,
        help=f"Output path (default: {DEFAULT_OUTPUT_PATH})",
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=30.0,
        help="HTTP timeout in seconds",
    )
    return parser.parse_args()


def fetch_html(url: str, timeout: float) -> str:
    request = Request(url, headers={"User-Agent": USER_AGENT})
    try:
        with urlopen(request, timeout=timeout) as response:
            charset = response.headers.get_content_charset() or "utf-8"
            return response.read().decode(charset, errors="replace")
    except HTTPError as exc:
        raise RuntimeError(f"request failed with HTTP {exc.code}") from exc
    except URLError as exc:
        raise RuntimeError(f"request failed: {exc.reason}") from exc


def extract_js_object(html: str) -> str:
    match = JS_OBJ_RE.search(html)
    if match is None:
        raise RuntimeError("pkchPokemonData not found in GameWith page")
    return match.group(1)


def js_to_json(js_text: str) -> str:
    step1 = KEY_RE.sub(r'"\1":', js_text)
    step2 = step1.replace("'", '"')
    return TRAILING_RE.sub(r"\1", step2)


def str_field(value: dict[str, Any], key: str) -> str:
    raw = value.get(key)
    return raw if isinstance(raw, str) else ""


def str_array(value: Any) -> list[str]:
    if not isinstance(value, list):
        return []
    return [item for item in value if isinstance(item, str)]


def parse_moves(value: Any) -> list[dict[str, str]]:
    if not isinstance(value, list):
        return []

    results: list[dict[str, str]] = []
    for item in value:
        if not isinstance(item, list) or len(item) < 2 or not isinstance(item[1], str):
            continue
        rate = item[2] if len(item) > 2 and isinstance(item[2], str) else ""
        results.append({"name": item[1], "rate": rate})
    return results


def parse_items(value: Any) -> list[dict[str, str]]:
    if not isinstance(value, list):
        return []

    results: list[dict[str, str]] = []
    for item in value:
        if not isinstance(item, list) or not item or not isinstance(item[0], str):
            continue
        rate = item[1] if len(item) > 1 and isinstance(item[1], str) else ""
        results.append({"name": item[0], "rate": rate})
    return results


def parse_natures(value: Any) -> list[dict[str, str]]:
    if not isinstance(value, list):
        return []

    results: list[dict[str, str]] = []
    for item in value:
        if not isinstance(item, list) or not item or not isinstance(item[0], str):
            continue
        rate = item[1] if len(item) > 1 and isinstance(item[1], str) else ""
        results.append({"name": item[0], "rate": rate})
    return results


def parse_evs(value: Any) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        return []

    results: list[dict[str, Any]] = []
    for item in value:
        if not isinstance(item, list) or len(item) < 2:
            continue
        stats = item[0]
        if not isinstance(stats, list):
            continue
        rate = item[1] if isinstance(item[1], str) else ""

        def n(index: int) -> int:
            raw = stats[index] if index < len(stats) else 0
            return raw if isinstance(raw, int) else 0

        results.append(
            {
                "h": n(0),
                "a": n(1),
                "b": n(2),
                "c": n(3),
                "d": n(4),
                "s": n(5),
                "rate": rate,
            }
        )
    return results


def build_pokemon_list(raw_data: dict[str, Any]) -> list[dict[str, Any]]:
    results: list[dict[str, Any]] = []
    for gamewith_poke_id, value in raw_data.items():
        if not isinstance(value, dict):
            continue
        results.append(
            {
                "id": gamewith_poke_id,
                "name": str_field(value, "name"),
                "types": str_array(value.get("types")),
                "moves": parse_moves(value.get("moves")),
                "items": parse_items(value.get("items")),
                "effort_values": parse_evs(value.get("evDistributions")),
                "natures": parse_natures(value.get("natures")),
            }
        )
    return results


def atomic_write_json(path: Path, data: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = json.dumps(data, ensure_ascii=False, indent=2) + "\n"

    temp_path: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="w",
            encoding="utf-8",
            newline="\n",
            dir=path.parent,
            delete=False,
            prefix=f"{path.name}.",
            suffix=".tmp",
        ) as handle:
            handle.write(payload)
            temp_path = Path(handle.name)

        os.replace(temp_path, path)
    finally:
        if temp_path is not None and temp_path.exists():
            temp_path.unlink()


def main() -> int:
    args = parse_args()
    output_path = args.output.expanduser().resolve()

    try:
        html = fetch_html(GAMEWITH_URL, timeout=args.timeout)
        js_text = extract_js_object(html)
        json_text = js_to_json(js_text)
        raw_data = json.loads(json_text)
        if not isinstance(raw_data, dict):
            raise RuntimeError("unexpected payload shape")
        pokemon_list = build_pokemon_list(raw_data)
        atomic_write_json(output_path, pokemon_list)
    except Exception as exc:
        print(f"Failed to refresh usage data: {exc}", file=sys.stderr)
        return 1

    print(f"Wrote {len(pokemon_list)} entries to {output_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
