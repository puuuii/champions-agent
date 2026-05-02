# champions-agent

## 前提

wsl2で動かすためには事前に管理者権限のPowershellで次を実行する
`usbipd list | Select-String "GC311G2" | ForEach-Object { $id = $_.ToString().Split(' ')[0]; usbipd attach --wsl --busid $id }`

## 情報源
- pokemon.csv: https://github.com/PokeAPI/pokeapi/blob/master/data/v2/csv/pokemon.csv
- pokemon_stats.csv: https://github.com/PokeAPI/pokeapi/blob/master/data/v2/csv/pokemon_stats.csv
- stats.csv: https://github.com/PokeAPI/pokeapi/blob/master/data/v2/csv/stats.csv
- ダメージ計算: https://champsone.com/#/articles/damage-formula
