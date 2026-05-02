## ホスト側のセットアップ
### USBの設定(Linux)
`sudo cp ./resources/99-struckout-mcu-esp.rules /etc/udev/rules.d/`
`sudo udevadm control --reload`
`sudo udevadm trigger`
