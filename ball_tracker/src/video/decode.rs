//! その場の解析 (`video::pipeline`) と、録画を解析し直すプログラム (`replay_video`、`bench_analysis`) で、
//! 同じ NV12 のデコードと RGB への変換を使う。

use super::analysis::AnalysisSettings;

/// ffmpeg の出力サイズと、Rust で縮小する倍率。設定から一度だけ決める。
#[derive(Debug, Clone, Copy)]
pub struct DecodeLayout {
    width: usize,
    height: usize,
    scale: usize,
}

impl DecodeLayout {
    pub fn from_settings(settings: &AnalysisSettings) -> Self {
        Self::new(
            settings.video_width,
            settings.video_height,
            settings.width,
            settings.height,
        )
    }

    fn new(video_width: usize, video_height: usize, width: usize, height: usize) -> Self {
        let scale = if video_width == width * 2 && video_height == height * 2 {
            2
        } else {
            1
        };
        Self {
            width,
            height,
            scale,
        }
    }

    /// NV12 の輝度面と、交互に並ぶ U/V 面の合計バイト数。
    pub fn frame_bytes(self) -> usize {
        let width = self.width * self.scale;
        let height = self.height * self.scale;
        width * height + width.div_ceil(2) * 2 * height.div_ceil(2)
    }

    /// フルレンジ BT.601 の NV12 を RGB24 に変換する。
    ///
    /// カメラの yuvj420p を前提にする。リミテッドレンジの動画では色が少しずれる。
    /// 2 倍なら輝度の 2x2 を平均し、解析画素ごとに元の色差をそのまま使う。
    /// バッファはこのレイアウトに一致する大きさで用意する。
    pub fn convert(self, nv12: &[u8], rgb: &mut [u8]) {
        assert_eq!(nv12.len(), self.frame_bytes());
        assert_eq!(rgb.len(), self.width * self.height * 3);
        let stride = self.width * self.scale;
        let (luma, chroma) = nv12.split_at(stride * self.height * self.scale);
        let uv_stride = stride.div_ceil(2) * 2;
        for (y, out) in rgb.chunks_exact_mut(self.width * 3).enumerate() {
            if self.scale == 2 {
                let top = &luma[y * 2 * stride..][..stride];
                let bottom = &luma[(y * 2 + 1) * stride..][..stride];
                let uv = &chroma[y * uv_stride..][..self.width * 2];
                for (((pixel, top), bottom), uv) in out
                    .as_chunks_mut::<3>()
                    .0
                    .iter_mut()
                    .zip(top.as_chunks::<2>().0)
                    .zip(bottom.as_chunks::<2>().0)
                    .zip(uv.as_chunks::<2>().0)
                {
                    let value = (u16::from(top[0])
                        + u16::from(top[1])
                        + u16::from(bottom[0])
                        + u16::from(bottom[1])
                        + 2)
                        >> 2;
                    pixel.copy_from_slice(&rgb_pixel(i32::from(value), uv[0], uv[1]));
                }
            } else {
                let row = &luma[y * stride..][..self.width];
                let uv = &chroma[y / 2 * uv_stride..][..uv_stride];
                // 横に並ぶ 2 画素が同じ色差を使う。2 画素ずつまとめて、添字の計算を省く
                // (自動ベクトル化が効き、1 画素ずつより 2 倍ほど速い)
                let (pairs, last) = out.as_chunks_mut::<6>();
                for ((pair, values), uv) in pairs
                    .iter_mut()
                    .zip(row.as_chunks::<2>().0)
                    .zip(uv.as_chunks::<2>().0)
                {
                    pair[..3].copy_from_slice(&rgb_pixel(i32::from(values[0]), uv[0], uv[1]));
                    pair[3..].copy_from_slice(&rgb_pixel(i32::from(values[1]), uv[0], uv[1]));
                }
                if !last.is_empty() {
                    // 幅が奇数のときの右端
                    let x = self.width - 1;
                    last.copy_from_slice(&rgb_pixel(
                        i32::from(row[x]),
                        uv[x / 2 * 2],
                        uv[x / 2 * 2 + 1],
                    ));
                }
            }
        }
    }
}

#[inline]
fn rgb_pixel(y: i32, u: u8, v: u8) -> [u8; 3] {
    let (u, v) = (i32::from(u) - 128, i32::from(v) - 128);
    // 係数は 16 ビットの固定小数点。半分を足して丸めてから範囲内に切り詰める。
    let channel = |delta: i32| (((y << 16) + delta + 32768) >> 16).clamp(0, 255) as u8;
    [
        channel(91881 * v),
        channel(-22553 * u - 46802 * v),
        channel(116130 * u),
    ]
}

/// 低遅延の H.264 入力から NV12 を出す ffmpeg の引数。その場の解析と、録画を解析し直すプログラムで同じものを
/// 使い、解析する画像が食い違わないようにする。
pub fn ffmpeg_args(settings: &AnalysisSettings, hwaccel: Option<&str>) -> Vec<String> {
    let mut args: Vec<String> = [
        "-hide_banner",
        "-loglevel",
        "error",
        // 遅れは 60fps の入力で 40ms 前後 (2026-10-15 に測った)。どれも測って決めた:
        // - low_delay がないと、並べ替えに備えて 250ms ほどためる
        // - probesize を小さくすると、最初のフレームが出るまでが 0.9 秒から 0.08 秒になる
        // - "-fflags nobuffer" は付けない。付けると 1 秒 (60 フレーム) 遅れたまま戻らない
        "-flags",
        "low_delay",
        "-probesize",
        "32768",
        // フレーム単位の並列化は、その枚数ぶん出てくるのが遅れるので使わない
        "-threads",
        "1",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    if let Some(name) = hwaccel {
        // GPU の方式は入力より前に指定する。出力は CPU 側に戻す。
        args.extend(["-hwaccel".into(), name.into()]);
    }
    args.extend(["-f", "h264", "-i", "pipe:0"].map(str::to_owned));
    let layout = DecodeLayout::from_settings(settings);
    if settings.video_width != settings.width * layout.scale
        || settings.video_height != settings.height * layout.scale
    {
        args.extend([
            "-vf".into(),
            format!("scale={}:{}:flags=area", settings.width, settings.height),
        ]);
    }
    // yuvj420p のフルレンジを NV12 でも維持する。
    args.extend(
        [
            "-fps_mode",
            "passthrough",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "nv12",
            "-color_range",
            "pc",
            "pipe:1",
        ]
        .map(str::to_owned),
    );
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gray_and_primary_colors() {
        for y in [0, 1, 127, 128, 254, 255] {
            assert_eq!(rgb_pixel(y, 128, 128), [y as u8; 3]);
        }
        assert_eq!(rgb_pixel(76, 85, 255), [254, 0, 0]);
        assert_eq!(rgb_pixel(150, 44, 21), [0, 255, 1]);
        assert_eq!(rgb_pixel(29, 255, 107), [0, 0, 254]);
    }

    #[test]
    fn clips_out_of_range_channels() {
        assert_eq!(rgb_pixel(255, 255, 255), [255, 121, 255]);
        assert_eq!(rgb_pixel(0, 0, 0), [0, 135, 0]);
    }

    #[test]
    fn averages_two_by_two_and_rounds_half_up() {
        let layout = DecodeLayout::new(4, 2, 2, 1);
        let mut rgb = [0; 6];
        layout.convert(&[0, 0, 0, 1, 0, 1, 1, 0, 128, 128, 255, 107], &mut rgb);
        assert_eq!(&rgb[..3], &[0; 3]);
        assert_eq!(&rgb[3..], &rgb_pixel(1, 255, 107));
    }

    #[test]
    fn shares_chroma_over_two_by_two_at_original_size() {
        let layout = DecodeLayout::new(4, 4, 4, 4);
        let mut nv12 = vec![100; 16];
        nv12.extend([128, 128, 255, 107, 85, 255, 44, 21]);
        let mut rgb = vec![0; 48];
        layout.convert(&nv12, &mut rgb);
        for y in 0..4 {
            for x in 0..4 {
                let uv = 16 + (y / 2 * 2 + x / 2) * 2;
                assert_eq!(
                    &rgb[(y * 4 + x) * 3..][..3],
                    &rgb_pixel(100, nv12[uv], nv12[uv + 1])
                );
            }
        }
    }

    #[test]
    fn arguments_select_rust_scaling_or_ffmpeg_fallback() {
        use struckout_proto::VideoStreamConfig;
        for (video_width, video_height, scale, filter) in [
            (1280, 720, 2, false),
            (640, 360, 1, false),
            (1920, 1080, 1, true),
            (1000, 700, 1, true),
        ] {
            let settings = AnalysisSettings::from_config(&VideoStreamConfig {
                video_width,
                video_height,
                analysis_width: 640,
                analysis_height: 360,
                fx: 1.0,
                fy: 1.0,
                device_to_camera: vec![0.0; 9],
                foreground_persistence_frames: 1,
                ..Default::default()
            })
            .expect("有効な設定");
            let layout = DecodeLayout::from_settings(&settings);
            assert_eq!(layout.scale, scale);
            assert_eq!(layout.frame_bytes(), 640 * 360 * scale * scale * 3 / 2);
            let args = ffmpeg_args(&settings, Some("d3d11va"));
            assert_eq!(args.iter().any(|arg| arg == "-vf"), filter);
            if filter {
                assert!(args.iter().any(|arg| arg == "scale=640:360:flags=area"));
            }
            for pair in [
                ["-pix_fmt", "nv12"],
                ["-hwaccel", "d3d11va"],
                ["-threads", "1"],
                ["-flags", "low_delay"],
                ["-probesize", "32768"],
                ["-fps_mode", "passthrough"],
                ["-color_range", "pc"],
            ] {
                assert!(args.windows(2).any(|args| args == pair));
            }
        }
    }
}
