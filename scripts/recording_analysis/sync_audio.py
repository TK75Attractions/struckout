#!/usr/bin/env python3
"""別々の端末で撮った動画どうしの時刻のずれを、音声の照合で求める。

    python sync_audio.py <基準の動画> <合わせる動画>...

基準の動画の 0 秒が、ほかの動画の何秒にあたるかを表示する。
基準には短いほうの動画を渡す (その全体が、ほかの動画の中に含まれている必要がある)。

必要なもの: Python 3、numpy、PATH 上の ffmpeg。

音は 1 ms で約 34 cm しか進まないので、端末が音源から 3 m 離れていると 9 ms ほど遅れて録音される。
結果にはこの程度の誤差が残る。コマ単位で合わせるには、両方に映った出来事で確かめること。
"""
import subprocess
import sys

import numpy as np

SAMPLE_RATE = 16000


def read_audio(video):
    cmd = [
        "ffmpeg", "-v", "error", "-i", str(video), "-vn", "-ac", "1", "-ar", str(SAMPLE_RATE),
        "-f", "f32le", "-",
    ]
    raw = subprocess.run(cmd, stdout=subprocess.PIPE, check=True).stdout
    return np.frombuffer(raw, np.float32).astype(np.float64)


def find_offset(reference, other):
    """reference の先頭が other の何秒にあたるかと、照合の確からしさを返す。"""
    n = 1 << int(np.ceil(np.log2(len(reference) + len(other))))
    spectrum = np.fft.rfft(other, n) * np.conj(np.fft.rfft(reference, n))
    # 位相だけで照合する (GCC-PHAT)。端末ごとのマイクの音質の違いや、声のような長く続く音に
    # 引きずられにくく、手を叩く音やボールが当たる音のような鋭い音で合う。
    correlation = np.fft.irfft(spectrum / np.maximum(np.abs(spectrum), 1e-12), n)
    valid = correlation[: len(other) - len(reference) + 1]
    peak = int(np.argmax(valid))
    # 2 番目に高い山との比。1 に近いほど、別の場所でも同じくらい合ってしまっている。
    masked = valid.copy()
    masked[max(0, peak - SAMPLE_RATE // 10): peak + SAMPLE_RATE // 10] = 0
    return peak / SAMPLE_RATE, float(valid[peak] / max(masked.max(), 1e-12))


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    reference = read_audio(sys.argv[1])
    print(f"基準: {sys.argv[1]} ({len(reference) / SAMPLE_RATE:.2f} 秒)")
    for path in sys.argv[2:]:
        other = read_audio(path)
        if len(other) < len(reference):
            print(f"{path}: 基準より短いので照合できません")
            continue
        offset, confidence = find_offset(reference, other)
        print(f"{path}: 基準の 0 秒 = {offset:.4f} 秒 (山の高さの比 {confidence:.1f})")


if __name__ == "__main__":
    main()
