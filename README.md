# champions-agent

## 前提

wsl2で動かすためには事前に管理者権限のPowershellで次を実行する
`usbipd list | Select-String "GC311G2" | ForEach-Object { $id = $_.ToString().Split(' ')[0]; usbipd attach --wsl --busid $id }`
