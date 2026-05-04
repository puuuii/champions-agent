from dataclasses import dataclass
from PIL import Image


@dataclass
class CutoutConfig:
    x_start_pct: float  # 切り出し開始X座標（0.0〜1.0）
    y_start_pct: float  # 切り出し開始Y座標（0.0〜1.0）
    x_end_pct: float    # 切り出し終了X座標（0.0〜1.0）
    y_end_pct: float    # 切り出し終了Y座標（0.0〜1.0）

    def __post_init__(self):
        for name, val in [
            ("x_start_pct", self.x_start_pct),
            ("y_start_pct", self.y_start_pct),
            ("x_end_pct", self.x_end_pct),
            ("y_end_pct", self.y_end_pct),
        ]:
            if not (0.0 <= val <= 1.0):
                raise ValueError(f"{name} は 0.0〜1.0 の範囲で指定してください: {val}")

        if self.x_start_pct >= self.x_end_pct:
            raise ValueError("x_start_pct は x_end_pct より小さくなければなりません")
        if self.y_start_pct >= self.y_end_pct:
            raise ValueError("y_start_pct は y_end_pct より小さくなければなりません")


def cutout_image(input_path: str, config: CutoutConfig, output_path: str) -> None:
    """
    入力画像をコンフィグの割合指定で切り抜き、出力パスに保存する。

    Args:
        input_path: 入力PNGファイルのパス
        config:     切り出し範囲を割合で指定するコンフィグ
        output_path: 出力PNGファイルのパス
    """
    with Image.open(input_path) as img:
        width, height = img.size

        left   = int(width  * config.x_start_pct)
        upper  = int(height * config.y_start_pct)
        right  = int(width  * config.x_end_pct)
        lower  = int(height * config.y_end_pct)

        cropped = img.crop((left, upper, right, lower))
        cropped.save(output_path, format="PNG")

    print(f"切り抜き完了: ({left}, {upper}) -> ({right}, {lower})  =>  {output_path}")


# ---- 使用例 ----
if __name__ == "__main__":
    config = CutoutConfig(
        x_start_pct=0.38,  # 左から25%
        y_start_pct=0.02,  # 上から10%
        x_end_pct=0.62,    # 左から75%
        y_end_pct=0.06,    # 上から90%
    )

    cutout_image(
        input_path="scripts/000003.png",
        config=config,
        output_path="master_data/ランクバトルシングルバトル切り出し.png",
    )
