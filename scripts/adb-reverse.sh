#!/bin/sh
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd) || exit 1
ports=$(python "$script_dir/generate_server_ports.py" --ports ball-tracker camera-ctrl camera-data camera-video) || exit 1

adb devices |
  awk 'NR > 1 && $2 == "device" {print $1}' |
  while IFS= read -r serial; do
    echo "Using device: $serial"
    for port in $ports; do
      adb -s "$serial" reverse "tcp:$port" "tcp:$port" || exit 1
    done
  done
