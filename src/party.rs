//! # party
//!
//! ポケモンパーティ判定パイプライン。
//!
//! - `cutout`  : キャプチャ画像からアイコン領域を切り出す
//! - `identifier` : DINOv2 ONNX でアイコンを同定する

pub mod cutout;
pub mod identifier;

// 実際に外部（main）から呼ばれる型のみを re-export
pub use cutout::default_crop_config;
pub use identifier::PartyIdentifier;
