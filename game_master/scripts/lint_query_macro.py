# Checks that all call of 'sqlx::query' family in 'src/data.rs' are compile-time verified macro,
# not runtime-verified functions.

from pathlib import Path


def main() -> int:
    path = Path("src/data.rs")
    buf = path.open()
    line_cnt: int = 0
    errors: list[int] = []
    while True:
        line = buf.readline()
        line_cnt = line_cnt + 1
        if line == "":
            break

        if "sqlx::query" in line and "!" not in line:
            errors.append(line_cnt)

    if errors:
        for err_line in errors:
            print(f"found non-macro call of sqlx::query family at {err_line}")

        return 1

    print("everything ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
