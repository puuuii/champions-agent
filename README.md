# champions-agent

## 前提

wsl2で動かすためには事前に管理者権限のPowershellで次を実行する
`usbipd list | Select-String "GC311G2" | ForEach-Object { $id = $_.ToString().Split(' ')[0]; usbipd attach --wsl --busid $id }`

また種々の設定のためにwsl2で次を実行する
export LIBGL_ALWAYS_SOFTWARE=1
export TRANSFORMERS_OFFLINE=1
export HF_DATASETS_OFFLINE=1

## 情報源
- csv: https://github.com/PokeAPI/pokeapi/tree/master/data/v2/csv
- ダメージ計算: https://champsone.com/#/articles/damage-formula
