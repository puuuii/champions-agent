"""
DINOv2 (vits14) を ONNX 形式にエクスポートする。
一度だけ実行すれば OK。

Usage:
    python export_dino.py

Output:
    models/dinov2_vits14.onnx
"""

import os
import torch

ONNX_PATH = "models/dinov2_vits14.onnx"
DEVICE = "cpu"  # エクスポートはCPUで十分


# DINOv2のforward(x, masks=None)のmasksをラップして単入力に固定する
class DINOv2Wrapper(torch.nn.Module):
    def __init__(self, model):
        super().__init__()
        self.model = model

    def forward(self, x):
        return self.model(x, masks=None)


def main():
    os.makedirs("models", exist_ok=True)

    print("Loading DINOv2 vits14...")
    model = torch.hub.load("facebookresearch/dinov2", "dinov2_vits14")
    model.eval().to(DEVICE)

    wrapped = DINOv2Wrapper(model).eval()
    dummy_input = torch.randn(1, 3, 224, 224, device=DEVICE)

    print(f"Exporting to {ONNX_PATH} ...")
    torch.onnx.export(
        wrapped,
        (dummy_input,),
        ONNX_PATH,
        input_names=["pixel_values"],
        output_names=["embedding"],
        dynamic_axes={
            "pixel_values": {0: "batch_size"},
            "embedding": {0: "batch_size"},
        },
        opset_version=17,
        dynamo=False,
    )

    print(f"Done: {ONNX_PATH}")

    # 簡易動作確認
    import onnxruntime as ort
    import numpy as np

    sess = ort.InferenceSession(ONNX_PATH, providers=["CPUExecutionProvider"])
    dummy_np = np.random.randn(1, 3, 224, 224).astype(np.float32)
    out = sess.run(None, {"pixel_values": dummy_np})
    print(f"Output shape: {out[0].shape}")  # 期待値: (1, 384)
    print("Export OK!")


if __name__ == "__main__":
    main()
