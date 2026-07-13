adb:
    adb devices | awk 'NR>1 && $2=="device" {print $1}' | while read serial; do adb -s "$serial" reverse tcp:6060 tcp:6060 && adb -s "$serial" reverse tcp:5050 tcp:5050; done