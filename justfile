adb:
    adb devices | awk 'NR>1 && $2=="device" {print $1}' | while read serial; do adb -s "$serial" reverse tcp:6060 tcp:6060 && adb -s "$serial" reverse tcp:5050 tcp:5050; done

[working-directory("./game_master")]
game-master-dev *args:
    docker compose --env-file .env.dev -f compose.yaml -f compose.dev.yaml up {{args}}

[working-directory("./game_master")]
game-master-prod *args:
    docker compose --env-file .env.prod -f compose.yaml -f compose.prod.yaml up {{args}}

game-master-db *args:
    docker compose --env-file .env.dev -f compose.yaml -f compose.dev.yaml up db {{args}}
