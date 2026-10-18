//! カメラアプリの `ObjectTracker` (struckoutCameraApp*/.../camera/ObjectTracker.kt) を移したもの。
//!
//! 背景との差分で動く物体を見つけ、見つけた領域を、平均色が背景からどれだけ色みを変えたかで絞り込む。
//! 手順と既定値はアプリと同じにしてある。アプリ側を変えたらこちらも合わせること。
//!
//! OpenCV を使わずに書いているので、アプリと次の点が少し違う。
//! - 背景を 8 ビットに直すときの丸め (OpenCV は偶数への丸め、こちらは四捨五入)
//! - 色みの判定に使う領域。アプリは外側の輪郭を塗りつぶした多角形、こちらは領域とその穴を合わせた画素

/// 検出した領域を囲む四角形 (解析画像の px)。幅と高さは画素の数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// 検出 1 件。枠と、前景の画素の重心と、画像の端に接しているか。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Blob {
    pub rect: Rect,
    /// 前景の画素の重心 (画素の中心を整数の座標とする。枠の左上の画素の中心が (x, y))
    pub centroid: (f64, f64),
    /// 画像の端に接している (ボールの一部が画像の外にあり、枠や重心がボールの中心からずれている)
    pub touches_edge: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackerParams {
    /// 背景を更新するときの、新しいフレームの重み
    pub accumulate_weight: f64,
    /// 背景との差 (最も大きく変わったチャンネル、8 ビットの階調) がこれを超えた画素を前景とする
    pub binary_threshold: f64,
    /// 外側の輪郭の面積 (px²) の下限
    pub min_contour_area: f64,
    /// 前景のまま動かない画素を、背景に取り込み始めるまでのフレーム数
    pub foreground_persistence_frames: u32,
    /// 平均色が背景から色みを変えた量の下限 (8 ビットの階調)。0 のときは絞り込まない
    pub min_color_shift: f64,
}

impl TrackerParams {
    /// アプリ (高速撮影版、120fps) の既定値。
    pub const APP_DEFAULT: Self = Self {
        accumulate_weight: 0.5,
        binary_threshold: 15.0,
        min_contour_area: 80.0,
        foreground_persistence_frames: 60,
        min_color_shift: 10.0,
    };
}

pub struct ObjectTracker {
    params: TrackerParams,
    width: usize,
    height: usize,
    /// 背景 (チャンネルごと、浮動小数)。空なら未初期化
    background: Vec<f32>,
    /// 前景が続いているフレーム数
    foreground_age: Vec<u16>,
    // 作業用。フレームごとに確保しない
    background8: Vec<u8>,
    binary: Vec<u8>,
    scratch_a: Vec<u8>,
    scratch_b: Vec<u8>,
    scratch_c: Vec<u8>,
    scratch_d: Vec<u8>,
    scratch_e: Vec<u8>,
    labels: Vec<u32>,
}

impl ObjectTracker {
    pub fn new(params: TrackerParams) -> Self {
        Self {
            params,
            width: 0,
            height: 0,
            background: Vec::new(),
            foreground_age: Vec::new(),
            background8: Vec::new(),
            binary: Vec::new(),
            scratch_a: Vec::new(),
            scratch_b: Vec::new(),
            scratch_c: Vec::new(),
            scratch_d: Vec::new(),
            scratch_e: Vec::new(),
            labels: Vec::new(),
        }
    }

    pub fn params(&self) -> TrackerParams {
        self.params
    }

    /// 1 フレームを解析する。`frame` は 3 チャンネルで、画素ごとに並んだ画像 (チャンネルの並びは問わない)。
    /// 最初のフレーム (と、大きさが変わったフレーム) は背景にするだけで、何も返さない。
    pub fn next_frame(&mut self, frame: &[u8], width: usize, height: usize) -> Vec<Rect> {
        self.next_frame_blobs(frame, width, height)
            .into_iter()
            .map(|blob| blob.rect)
            .collect()
    }

    /// [`Self::next_frame`] と同じ処理で、枠に加えて重心と、画像の端に接しているかも返す。
    pub fn next_frame_blobs(&mut self, frame: &[u8], width: usize, height: usize) -> Vec<Blob> {
        let pixels = width * height;
        assert_eq!(frame.len(), pixels * 3, "frame must have 3 channels");
        if self.background.is_empty() || self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            self.background = frame.iter().map(|&v| f32::from(v)).collect();
            self.foreground_age = vec![0; pixels];
            self.background8 = vec![0; pixels * 3];
            self.binary = vec![0; pixels];
            self.scratch_a = vec![0; pixels];
            self.scratch_b = vec![0; pixels];
            self.scratch_c = vec![0; pixels];
            self.scratch_d = vec![0; pixels];
            self.scratch_e = vec![0; pixels];
            self.labels = vec![0; pixels];
            return Vec::new();
        }

        // 背景との差を色ごとに取り、最も大きく変わったチャンネルで判定する
        let threshold = self.params.binary_threshold;
        for (i, bg) in self.background.iter().enumerate() {
            self.background8[i] = bg.abs().round().clamp(0.0, 255.0) as u8;
        }
        for p in 0..pixels {
            let mut delta = 0u8;
            for c in 0..3 {
                delta = delta.max(frame[p * 3 + c].abs_diff(self.background8[p * 3 + c]));
            }
            self.binary[p] = u8::from(f64::from(delta) > threshold);
        }

        // 小さな点を消してから (オープニング 1 回)、穴を埋める (クロージング 2 回)
        let (w, h) = (width, height);
        let mut work = Work {
            tmp: &mut self.scratch_c,
            line: &mut self.scratch_d,
            line_tmp: &mut self.scratch_e,
        };
        erode_ellipse5(&self.binary, &mut self.scratch_a, &mut work, w, h);
        dilate_ellipse5(&self.scratch_a, &mut self.scratch_b, &mut work, w, h);
        dilate_ellipse5(&self.scratch_b, &mut self.scratch_a, &mut work, w, h);
        dilate_ellipse5(&self.scratch_a, &mut self.scratch_b, &mut work, w, h);
        erode_ellipse5(&self.scratch_b, &mut self.scratch_a, &mut work, w, h);
        erode_ellipse5(&self.scratch_a, &mut self.binary, &mut work, w, h);
        let clean = &self.binary;

        // 止まった前景は、しばらくしたら背景に取り込む
        let persistence = self.params.foreground_persistence_frames;
        let weight = self.params.accumulate_weight as f32;
        for p in 0..pixels {
            if clean[p] == 0 {
                self.foreground_age[p] = 0;
            } else {
                self.foreground_age[p] = self.foreground_age[p].saturating_add(1);
            }
            let update = clean[p] == 0 || u32::from(self.foreground_age[p]) >= persistence;
            if update {
                for c in 0..3 {
                    let i = p * 3 + c;
                    self.background[i] =
                        (1.0 - weight) * self.background[i] + weight * f32::from(frame[i]);
                }
            }
        }

        let mut rects = Vec::new();
        let components = label_components(clean, &mut self.labels, w, h);
        for component in components {
            let contour = trace_outer_contour(&self.labels, w, h, component.label, component.start);
            if polygon_area(&contour) < self.params.min_contour_area {
                continue;
            }
            let rect = Rect {
                x: component.min_x as i32,
                y: component.min_y as i32,
                width: (component.max_x - component.min_x + 1) as i32,
                height: (component.max_y - component.min_y + 1) as i32,
            };
            if self.params.min_color_shift > 0.0
                && color_shift(
                    frame,
                    &self.background8,
                    &self.labels,
                    w,
                    component.label,
                    rect,
                ) < self.params.min_color_shift
            {
                continue;
            }
            let (mut sx, mut sy, mut n) = (0.0, 0.0, 0.0);
            for y in component.min_y..=component.max_y {
                for x in component.min_x..=component.max_x {
                    if self.labels[y * w + x] == component.label {
                        sx += x as f64;
                        sy += y as f64;
                        n += 1.0;
                    }
                }
            }
            rects.push(Blob {
                rect,
                centroid: (sx / n, sy / n),
                touches_edge: component.min_x == 0
                    || component.min_y == 0
                    || component.max_x == w - 1
                    || component.max_y == h - 1,
            });
        }
        rects
    }
}

// --- 形の処理 (5x5 の楕円) ---
//
// OpenCV の getStructuringElement(MORPH_ELLIPSE, 5x5) は次の形。
//   0 0 1 0 0
//   1 1 1 1 1
//   1 1 1 1 1
//   1 1 1 1 1
//   0 0 1 0 0
// 横 5 x 縦 3 の長方形と、縦 5 の線を合わせた形なので、それぞれで処理して合わせる。
// 膨張は和、収縮は積になる。画像の外は、膨張では 0、収縮では 1 とみなす (OpenCV の BORDER_CONSTANT の既定)。

/// 形の処理の作業用の配列 (どれも画像と同じ大きさ)。
struct Work<'a> {
    tmp: &'a mut [u8],
    line: &'a mut [u8],
    line_tmp: &'a mut [u8],
}

fn dilate_ellipse5(src: &[u8], dst: &mut [u8], work: &mut Work, w: usize, h: usize) {
    rect_filter(src, dst, work.tmp, w, h, 2, 1, true);
    rect_filter(src, work.line, work.line_tmp, w, h, 0, 2, true);
    for (d, l) in dst.iter_mut().zip(work.line.iter()) {
        *d |= *l;
    }
}

fn erode_ellipse5(src: &[u8], dst: &mut [u8], work: &mut Work, w: usize, h: usize) {
    rect_filter(src, dst, work.tmp, w, h, 2, 1, false);
    rect_filter(src, work.line, work.line_tmp, w, h, 0, 2, false);
    for (d, l) in dst.iter_mut().zip(work.line.iter()) {
        *d &= *l;
    }
}

/// 横に `rx`、縦に `ry` 画素ずつ広げた長方形で、膨張 (`dilate`) か収縮をする。値は 0 か 1。
///
/// 1 画素ずつ窓を見るのではなく、行 (縦の処理では行どうし) をずらして重ねる。CPU のベクトル命令が使えるので速い。
/// 画像の外は、ずらしたときに重ならないだけなので、膨張では 0、収縮では 1 とみなしたことになる。
#[allow(clippy::too_many_arguments)]
fn rect_filter(
    src: &[u8],
    dst: &mut [u8],
    tmp: &mut [u8],
    w: usize,
    h: usize,
    rx: usize,
    ry: usize,
    dilate: bool,
) {
    let combine = |out: &mut [u8], other: &[u8]| {
        if dilate {
            out.iter_mut().zip(other).for_each(|(o, v)| *o |= *v);
        } else {
            out.iter_mut().zip(other).for_each(|(o, v)| *o &= *v);
        }
    };
    tmp.copy_from_slice(src);
    for y in 0..h {
        let row = y * w..(y + 1) * w;
        let source = &src[row.clone()];
        let target = &mut tmp[row];
        for d in 1..=rx.min(w.saturating_sub(1)) {
            combine(&mut target[d..], &source[..w - d]);
            combine(&mut target[..w - d], &source[d..]);
        }
    }
    dst.copy_from_slice(tmp);
    for d in 1..=ry.min(h.saturating_sub(1)) {
        for y in 0..h {
            let row = y * w..(y + 1) * w;
            if y >= d {
                combine(&mut dst[row.clone()], &tmp[(y - d) * w..(y - d + 1) * w]);
            }
            if y + d < h {
                combine(&mut dst[row], &tmp[(y + d) * w..(y + d + 1) * w]);
            }
        }
    }
}

// --- 領域と輪郭 ---

struct Component {
    label: u32,
    /// ラスタ順で最初の画素 (最も上の行の、最も左)
    start: (usize, usize),
    min_x: usize,
    min_y: usize,
    max_x: usize,
    max_y: usize,
}

/// 8 近傍でつながった前景の領域に番号 (1 から) を付ける。背景は 0。
fn label_components(binary: &[u8], labels: &mut [u32], w: usize, h: usize) -> Vec<Component> {
    labels.iter_mut().for_each(|l| *l = 0);
    let mut components = Vec::new();
    let mut stack = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if binary[i] == 0 || labels[i] != 0 {
                continue;
            }
            let label = components.len() as u32 + 1;
            let mut component = Component {
                label,
                start: (x, y),
                min_x: x,
                min_y: y,
                max_x: x,
                max_y: y,
            };
            labels[i] = label;
            stack.push((x, y));
            while let Some((cx, cy)) = stack.pop() {
                component.min_x = component.min_x.min(cx);
                component.max_x = component.max_x.max(cx);
                component.min_y = component.min_y.min(cy);
                component.max_y = component.max_y.max(cy);
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        let nx = cx as i32 + dx;
                        let ny = cy as i32 + dy;
                        if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                            continue;
                        }
                        let j = ny as usize * w + nx as usize;
                        if binary[j] != 0 && labels[j] == 0 {
                            labels[j] = label;
                            stack.push((nx as usize, ny as usize));
                        }
                    }
                }
            }
            components.push(component);
        }
    }
    components
}

/// 時計回りの 8 近傍 (画像の y は下向き)。西から始める。
const NEIGHBORS: [(i32, i32); 8] = [
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
];

/// 領域の外側の輪郭を、画素の中心の並びとして返す (ムーア近傍の追跡)。
/// `start` はラスタ順で最初の画素なので、その西は必ず領域の外。
fn trace_outer_contour(
    labels: &[u32],
    w: usize,
    h: usize,
    label: u32,
    start: (usize, usize),
) -> Vec<(i32, i32)> {
    let inside = |x: i32, y: i32| {
        x >= 0
            && y >= 0
            && x < w as i32
            && y < h as i32
            && labels[y as usize * w + x as usize] == label
    };
    let b0 = (start.0 as i32, start.1 as i32);
    let mut contour = vec![b0];
    // 次の画素を探す。`from` は b の近傍のうち、最後に見た外の画素の向き
    let next = |b: (i32, i32), from: usize| -> Option<((i32, i32), usize)> {
        for k in 1..=8 {
            let d = (from + k) % 8;
            let (dx, dy) = NEIGHBORS[d];
            if inside(b.0 + dx, b.1 + dy) {
                let n = (b.0 + dx, b.1 + dy);
                // n から見た、直前に見た外の画素 (b の近傍の d-1 番目) の向き
                let (pdx, pdy) = NEIGHBORS[(d + 7) % 8];
                let outside = (b.0 + pdx, b.1 + pdy);
                let back = NEIGHBORS
                    .iter()
                    .position(|&(ox, oy)| (n.0 + ox, n.1 + oy) == outside)
                    .unwrap_or((d + 4) % 8);
                return Some((n, back));
            }
        }
        None
    };
    let Some((b1, mut from)) = next(b0, 0) else {
        return contour; // 1 画素だけ
    };
    let mut b = b1;
    contour.push(b);
    // ジェイコブの停止条件: b0 に戻り、次が b1 になったら 1 周
    for _ in 0..4 * w * h {
        let Some((n, back)) = next(b, from) else {
            break;
        };
        if b == b0 && n == b1 {
            break;
        }
        contour.push(n);
        b = n;
        from = back;
    }
    // 最後に b0 に戻った点は始点と同じなので除く
    if contour.len() > 1 && contour.last() == Some(&b0) {
        contour.pop();
    }
    contour
}

/// 閉じた多角形の面積 (靴ひもの公式)。OpenCV の contourArea と同じ。
fn polygon_area(points: &[(i32, i32)]) -> f64 {
    if points.len() < 3 {
        return 0.0;
    }
    let mut twice = 0i64;
    for i in 0..points.len() {
        let (x0, y0) = points[i];
        let (x1, y1) = points[(i + 1) % points.len()];
        twice += i64::from(x0) * i64::from(y1) - i64::from(x1) * i64::from(y0);
    }
    (twice as f64).abs() / 2.0
}

/// 領域 (穴を含む) の平均色が、背景の同じ場所の平均色から色みをどれだけ変えたか (8 ビットの階調)。
///
/// 次の 2 つの残りの小さいほう。アプリの ObjectTracker.colorShift と同じ式。
/// 1. 明るさだけが何倍かになったとみなしたときの残り
/// 2. 全チャンネルが同じ量だけ増減したとみなしたときの残り
fn color_shift(
    frame: &[u8],
    background8: &[u8],
    labels: &[u32],
    w: usize,
    label: u32,
    rect: Rect,
) -> f64 {
    let (rx, ry, rw, rh) = (
        rect.x as usize,
        rect.y as usize,
        rect.width as usize,
        rect.height as usize,
    );
    // 四角形の縁からたどれる「領域でない画素」が外。それ以外 (領域とその穴) を平均する
    let mut outside = vec![false; rw * rh];
    let mut stack = Vec::new();
    let in_region = |x: usize, y: usize| labels[(ry + y) * w + rx + x] == label;
    for x in 0..rw {
        for y in [0, rh - 1] {
            if !in_region(x, y) && !outside[y * rw + x] {
                outside[y * rw + x] = true;
                stack.push((x, y));
            }
        }
    }
    for y in 0..rh {
        for x in [0, rw - 1] {
            if !in_region(x, y) && !outside[y * rw + x] {
                outside[y * rw + x] = true;
                stack.push((x, y));
            }
        }
    }
    while let Some((x, y)) = stack.pop() {
        for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if nx < 0 || ny < 0 || nx >= rw as i32 || ny >= rh as i32 {
                continue;
            }
            let (nx, ny) = (nx as usize, ny as usize);
            if !outside[ny * rw + nx] && !in_region(nx, ny) {
                outside[ny * rw + nx] = true;
                stack.push((nx, ny));
            }
        }
    }
    let mut now = [0f64; 3];
    let mut before = [0f64; 3];
    let mut count = 0f64;
    for y in 0..rh {
        for x in 0..rw {
            if outside[y * rw + x] {
                continue;
            }
            let p = (ry + y) * w + rx + x;
            for c in 0..3 {
                now[c] += f64::from(frame[p * 3 + c]);
                before[c] += f64::from(background8[p * 3 + c]);
            }
            count += 1.0;
        }
    }
    if count == 0.0 {
        return 0.0;
    }
    for c in 0..3 {
        now[c] /= count;
        before[c] /= count;
    }
    let dot: f64 = (0..3).map(|c| now[c] * before[c]).sum();
    let denominator: f64 = before.iter().map(|b| b * b).sum();
    let k = if denominator > 0.0 {
        dot / denominator
    } else {
        0.0
    };
    let scale_residual = (0..3)
        .map(|c| (now[c] - k * before[c]).powi(2))
        .sum::<f64>()
        .sqrt();
    let delta: Vec<f64> = (0..3).map(|c| now[c] - before[c]).collect();
    let mean = delta.iter().sum::<f64>() / 3.0;
    let offset_residual = delta.iter().map(|d| (d - mean).powi(2)).sum::<f64>().sqrt();
    scale_residual.min(offset_residual)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(w: usize, h: usize, color: [u8; 3]) -> Vec<u8> {
        let mut out = Vec::with_capacity(w * h * 3);
        for _ in 0..w * h {
            out.extend_from_slice(&color);
        }
        out
    }

    fn fill_disc(frame: &mut [u8], w: usize, cx: f64, cy: f64, r: f64, color: [u8; 3]) {
        let h = frame.len() / 3 / w;
        for y in 0..h {
            for x in 0..w {
                if (x as f64 - cx).powi(2) + (y as f64 - cy).powi(2) <= r * r {
                    frame[(y * w + x) * 3..(y * w + x) * 3 + 3].copy_from_slice(&color);
                }
            }
        }
    }

    #[test]
    fn ellipse_kernel_matches_opencv() {
        // 1 画素を膨張すると、OpenCV の 5x5 の楕円の形になる
        let (w, h) = (9, 9);
        let mut src = vec![0u8; w * h];
        src[4 * w + 4] = 1;
        let mut dst = vec![0u8; w * h];
        let (mut a, mut b, mut c) = (vec![0u8; w * h], vec![0u8; w * h], vec![0u8; w * h]);
        dilate_ellipse5(
            &src,
            &mut dst,
            &mut Work {
                tmp: &mut a,
                line: &mut b,
                line_tmp: &mut c,
            },
            w,
            h,
        );
        let expected = [
            [0, 0, 1, 0, 0],
            [1, 1, 1, 1, 1],
            [1, 1, 1, 1, 1],
            [1, 1, 1, 1, 1],
            [0, 0, 1, 0, 0],
        ];
        for y in 0..5 {
            for x in 0..5 {
                assert_eq!(dst[(y + 2) * w + x + 2], expected[y][x], "({x}, {y})");
            }
        }
        assert_eq!(dst.iter().map(|&v| u32::from(v)).sum::<u32>(), 17);
    }

    #[test]
    fn erosion_does_not_shrink_at_the_image_border() {
        let (w, h) = (6, 6);
        let src = vec![1u8; w * h];
        let mut dst = vec![0u8; w * h];
        let (mut a, mut b, mut c) = (vec![0u8; w * h], vec![0u8; w * h], vec![0u8; w * h]);
        erode_ellipse5(
            &src,
            &mut dst,
            &mut Work {
                tmp: &mut a,
                line: &mut b,
                line_tmp: &mut c,
            },
            w,
            h,
        );
        assert!(dst.iter().all(|&v| v == 1));
    }

    #[test]
    fn contour_area_matches_opencv_for_a_square() {
        // 4x4 の正方形の外側の輪郭は、画素の中心を結んだ 3x3 の正方形 (OpenCV の contourArea は 9)
        let (w, h) = (8, 8);
        let mut labels = vec![0u32; w * h];
        for y in 2..6 {
            for x in 2..6 {
                labels[y * w + x] = 1;
            }
        }
        let contour = trace_outer_contour(&labels, w, h, 1, (2, 2));
        assert_eq!(polygon_area(&contour), 9.0);
        assert_eq!(contour.len(), 12);
    }

    #[test]
    fn contour_of_a_single_pixel_has_no_area() {
        let (w, h) = (3, 3);
        let mut labels = vec![0u32; w * h];
        labels[4] = 1;
        let contour = trace_outer_contour(&labels, w, h, 1, (1, 1));
        assert_eq!(contour, vec![(1, 1)]);
        assert_eq!(polygon_area(&contour), 0.0);
    }

    #[test]
    fn contour_of_a_disc_encloses_most_of_its_pixels() {
        let (w, h) = (40, 40);
        let mut labels = vec![0u32; w * h];
        let mut count = 0;
        let mut start = None;
        for y in 0..h {
            for x in 0..w {
                if (x as f64 - 20.0).powi(2) + (y as f64 - 20.0).powi(2) <= 64.0 {
                    labels[y * w + x] = 1;
                    count += 1;
                    start.get_or_insert((x, y));
                }
            }
        }
        let area = polygon_area(&trace_outer_contour(&labels, w, h, 1, start.unwrap()));
        // 画素の中心を結ぶので、画素の数より外周の半分ほど小さい
        assert!(
            area < count as f64 && area > count as f64 * 0.8,
            "area {area}, pixels {count}"
        );
    }

    #[test]
    fn finds_a_moving_colored_ball_and_ignores_a_shadow() {
        let (w, h) = (160, 90);
        let background = [120u8, 110, 100];
        let mut tracker = ObjectTracker::new(TrackerParams::APP_DEFAULT);
        assert!(
            tracker
                .next_frame(&image(w, h, background), w, h)
                .is_empty()
        );

        // 黄色いボールと、背景を暗くしただけの影
        let mut frame = image(w, h, background);
        fill_disc(&mut frame, w, 40.0, 40.0, 8.0, [220, 200, 40]);
        fill_disc(&mut frame, w, 110.0, 50.0, 8.0, [60, 55, 50]);
        let rects = tracker.next_frame(&frame, w, h);
        assert_eq!(rects.len(), 1, "{rects:?}");
        // 円 (直径 17) の左右の先端 1 画素ずつは、横長の核のオープニングで削れる (OpenCV でも同じ)
        assert_eq!(
            rects[0],
            Rect {
                x: 33,
                y: 32,
                width: 15,
                height: 17
            }
        );
    }

    #[test]
    fn ignores_small_specks() {
        let (w, h) = (80, 60);
        let background = [100u8, 100, 100];
        let mut tracker = ObjectTracker::new(TrackerParams {
            min_color_shift: 0.0,
            ..TrackerParams::APP_DEFAULT
        });
        tracker.next_frame(&image(w, h, background), w, h);
        let mut frame = image(w, h, background);
        // 2x2 の点はオープニングで消え、輪郭の面積が 80 に満たない領域も捨てる
        for (x, y) in [(10, 10), (11, 10), (10, 11), (11, 11)] {
            frame[(y * w + x) * 3..(y * w + x) * 3 + 3].copy_from_slice(&[255, 255, 255]);
        }
        fill_disc(&mut frame, w, 50.0, 30.0, 4.0, [255, 255, 255]);
        assert!(tracker.next_frame(&frame, w, h).is_empty());
    }

    #[test]
    fn centroid_follows_the_ball_center_and_edge_balls_are_flagged() {
        let (w, h) = (160, 90);
        let background = [120u8, 110, 100];
        let mut tracker = ObjectTracker::new(TrackerParams::APP_DEFAULT);
        tracker.next_frame(&image(w, h, background), w, h);
        // 画素の境目にない中心 (40.3, 40.6) と、右の端で欠けたボール
        let mut frame = image(w, h, background);
        fill_disc(&mut frame, w, 40.3, 40.6, 8.0, [220, 200, 40]);
        fill_disc(&mut frame, w, 157.0, 60.0, 8.0, [220, 200, 40]);
        let mut blobs = tracker.next_frame_blobs(&frame, w, h);
        blobs.sort_by_key(|b| b.rect.x);
        assert_eq!(blobs.len(), 2, "{blobs:?}");
        let (cx, cy) = blobs[0].centroid;
        assert!(
            (cx - 40.3).abs() < 0.5 && (cy - 40.6).abs() < 0.5,
            "centroid {cx}, {cy}"
        );
        assert!(!blobs[0].touches_edge);
        assert!(blobs[1].touches_edge);
        // 欠けたボールの重心は、画像の内側に寄る
        assert!(blobs[1].centroid.0 < 157.0);
    }
}
