#! /usr/bin/env bash
adb devices |
  awk 'NR > 1 && $2 == "device" {print $1}' |
  while IFS= read -r serial; do
    echo "Using device: $serial"
    adb -s "$serial" reverse tcp:6060 tcp:6060 || exit 1
    adb -s "$serial" reverse tcp:5050 tcp:5050 || exit 1
  done
