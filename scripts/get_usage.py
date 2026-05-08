import requests
from bs4 import BeautifulSoup
import re
import json
import os

def save_pokemon_usage_to_json():
    url = "https://gamewith.jp/pokemon-champions/555373"
    headers = {
        "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
    }
    output_path = "./cache/usage.json"

    try:
        session = requests.Session()
        response = session.get(url, headers=headers, timeout=15)
        response.raise_for_status()

        soup = BeautifulSoup(response.text, 'html.parser')
        scripts = soup.find_all('script')

        target_content = None
        for s in scripts:
            if s.string and 'pkchPokemonData' in s.string:
                target_content = s.string
                break

        if not target_content:
            print("scriptタグ内に 'pkchPokemonData' が見つかりませんでした。")
            return

        # 1. pkchPokemonData = { ... } の中身を抽出
        # 前後の変数宣言やスクリプト終了記号に影響されないよう、波括弧の対応を考慮して抽出
        match = re.search(r'pkchPokemonData\s*=\s*(\{.*?\});', target_content, re.DOTALL)
        if not match:
            # セミコロンがない場合のフォールバック
            match = re.search(r'pkchPokemonData\s*=\s*(\{.*?})(?=\n|const|var|let|$)', target_content, re.DOTALL)

        if not match:
            print("データの抽出に失敗しました。")
            return

        obj_content = match.group(1)

        # 2. JSオブジェクト形式をJSON形式に変換
        # キー（引用符なし）を引用符で囲む (例: name: -> "name":, 445: -> "445":)
        json_like = re.sub(r'([{,]\s*)([a-zA-Z0-9_]+):', r'\1"\2":', obj_content)

        # 3. JSONとしてパース
        try:
            data_dict = json.loads(json_like)
        except json.JSONDecodeError:
            # 末尾のカンマなどを削除して再試行
            json_like = re.sub(r',\s*([\]}])', r'\1', json_like)
            data_dict = json.loads(json_like)

        # 4. ご要望の形式 [ { "445": {...} }, { "730": {...} }, ... ] に変換
        # 元のデータの並び順（出現順）を維持します
        ordered_list = []
        for key, value in data_dict.items():
            ordered_list.append({key: value})

        # 5. 保存
        os.makedirs(os.path.dirname(output_path), exist_ok=True)
        with open(output_path, 'w', encoding='utf-8') as f:
            json.dump(ordered_list, f, ensure_ascii=False, indent=4)

        print(f"正常に保存されました: {output_path} (合計 {len(ordered_list)} 件)")

    except Exception as e:
        print(f"エラーが発生しました: {e}")

if __name__ == "__main__":
    save_pokemon_usage_to_json()
