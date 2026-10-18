//! カメラアプリの `ObjectTracker` (struckoutCameraApp*/.../camera/ObjectTracker.kt) を移したもの。
//!
//! 背景との差分で動く物体を見つけ、見つけた領域を、平均色が背景からどれだけ色みを変えたかで絞り込む。
//! 手順と既定値はアプリと同じにしてある。アプリ側を変えたらこちらも合わせること。
//!
//! OpenCV を使わずに書いているので、アプリと次の点が少し違う。
//! - 背景を 8 ビットに直すときの丸め (OpenCV は偶数への丸め、こちらは四捨五入)
//! - 色みの判定に使う領域。アプリは外側の輪郭を塗りつぶした多角形、こちらは領域とその穴を合わせた画素
//!
//! 120fps のカメラ 2 台を、それぞれ 1 本のスレッドで解析する (1 フレーム 8.33 ms の予算)。長く動かすと PC が熱で
//! 遅くなるので、どの手順も画像全体をなるべく少ない回数だけなぞるようにしてある。
//! - 背景との差は、背景を 8 ビットに直しながら 1 回でなぞり、前景かどうかを 1 画素 1 ビットに詰める
//! - 形の処理は、1 行を 64 画素ずつの整数として、ビット演算で行う
//! - 領域の番号付けと背景の更新は、前景の画素だけを見る (背景はまとめて更新し、残す画素だけ元に戻す)

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
    /// 前景が続いているフレーム数。前のフレームで前景だった画素だけが 0 でない
    foreground_age: Vec<u16>,
    /// 前景 (形の処理をしたあと)。前のフレームのものは、番号と前景の続いた数を消すのに使う
    clean: BitImage,
    previous_clean: BitImage,
    // 作業用。フレームごとに確保しない
    binary: BitImage,
    scratch: BitImage,
    inverted: BitImage,
    deltas: Vec<u8>,
    row: Vec<u8>,
    rows: [Vec<u64>; 3],
    /// 領域の番号。前景の画素だけに付ける (それ以外は 0)
    labels: Vec<u32>,
    label_scratch: LabelScratch,
    color_scratch: ColorScratch,
}

impl ObjectTracker {
    pub fn new(params: TrackerParams) -> Self {
        Self {
            params,
            width: 0,
            height: 0,
            background: Vec::new(),
            foreground_age: Vec::new(),
            clean: BitImage::new(0, 0),
            previous_clean: BitImage::new(0, 0),
            binary: BitImage::new(0, 0),
            scratch: BitImage::new(0, 0),
            inverted: BitImage::new(0, 0),
            deltas: Vec::new(),
            row: Vec::new(),
            rows: [Vec::new(), Vec::new(), Vec::new()],
            labels: Vec::new(),
            label_scratch: LabelScratch::default(),
            color_scratch: ColorScratch::default(),
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
            self.clean = BitImage::new(width, height);
            self.previous_clean = BitImage::new(width, height);
            self.binary = BitImage::new(width, height);
            self.scratch = BitImage::new(width, height);
            self.inverted = BitImage::new(width, height);
            self.deltas = vec![0; width * 3];
            self.row = vec![0; width];
            self.rows = [
                vec![0; self.clean.words_per_row],
                vec![0; self.clean.words_per_row],
                vec![0; self.clean.words_per_row],
            ];
            self.labels = vec![0; pixels];
            return Vec::new();
        }

        // 背景との差を色ごとに取り、最も大きく変わったチャンネルで判定する
        threshold(
            frame,
            &self.background,
            self.params.binary_threshold,
            &mut self.deltas,
            &mut self.row,
            &mut self.binary,
        );

        // 小さな点を消してから (オープニング 1 回)、穴を埋める (クロージング 2 回)
        let (rows, inverted) = (&mut self.rows, &mut self.inverted);
        erode_ellipse5(&self.binary, &mut self.scratch, inverted, rows);
        dilate_ellipse5(&self.scratch, &mut self.binary, rows);
        dilate_ellipse5(&self.binary, &mut self.scratch, rows);
        dilate_ellipse5(&self.scratch, &mut self.binary, rows);
        erode_ellipse5(&self.binary, &mut self.scratch, inverted, rows);
        std::mem::swap(&mut self.previous_clean, &mut self.clean);
        erode_ellipse5(&self.scratch, &mut self.clean, inverted, rows);

        // 領域を探す。背景はまだ更新していないので、色みの判定はこのフレームの前の背景と比べる
        let (w, h) = (width, height);
        for i in self.previous_clean.ones() {
            self.labels[i] = 0;
        }
        let mut rects = Vec::new();
        let components = label_components(&self.clean, &mut self.labels, &mut self.label_scratch);
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
                    &self.background,
                    &self.labels,
                    w,
                    component.label,
                    rect,
                    &mut self.color_scratch,
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

        // 止まった前景は、しばらくしたら背景に取り込む。64 画素 (1 つの整数) ずつ、前景の続いた数を更新し、
        // 背景に取り込まない画素 (前景で、続いた数がまだ足りない) を除いて背景を更新する。前景はふつう画像の
        // ごく一部なので、ほとんどの 64 画素は除くものがなく、まとめて更新できる (ベクトル命令が使える)
        let persistence = self.params.foreground_persistence_frames;
        let weight = self.params.accumulate_weight as f32;
        let keep = 1.0 - weight;
        let n = self.clean.words_per_row;
        for y in 0..h {
            for k in 0..n {
                let (now, before) = (
                    self.clean.words[y * n + k],
                    self.previous_clean.words[y * n + k],
                );
                let x0 = k * 64;
                let base = y * w + x0;
                // 前景の続いた数は、前のフレームで前景だった画素のほかは 0 のまま
                for bit in Ones(before & !now) {
                    self.foreground_age[base + bit] = 0;
                }
                let mut hold = 0u64;
                for bit in Ones(now) {
                    let age = &mut self.foreground_age[base + bit];
                    *age = age.saturating_add(1);
                    if u32::from(*age) < persistence {
                        hold |= 1 << bit;
                    }
                }
                let count = (w - x0).min(64);
                let values = base * 3..(base + count) * 3;
                let background = &mut self.background[values.clone()];
                let frame = &frame[values];
                if hold == 0 {
                    for (bg, &value) in background.iter_mut().zip(frame) {
                        *bg = keep * *bg + weight * f32::from(value);
                    }
                } else {
                    for (i, (bg, value)) in background
                        .as_chunks_mut::<3>()
                        .0
                        .iter_mut()
                        .zip(frame.as_chunks::<3>().0)
                        .enumerate()
                    {
                        if hold >> i & 1 == 0 {
                            for c in 0..3 {
                                bg[c] = keep * bg[c] + weight * f32::from(value[c]);
                            }
                        }
                    }
                }
            }
        }
        rects
    }
}

/// 背景の値を 8 ビットに直す。`value.abs().round().clamp(0.0, 255.0) as u8` と同じ結果。
///
/// `f32::round` は x86_64 の既定の命令 (SSE2) では 1 つずつの関数呼び出しになり、画像全体では遅い。
/// 0.5 を足して切り捨てると、0.5 のすぐ下の値 (0.49999997) だけが 1 に繰り上がるので、それを直す。
#[inline]
fn to_u8(value: f32) -> u8 {
    let value = value.abs();
    // 255 を超えるところは飽和し、NaN は 0 になる
    let rounded = (value + 0.5) as u8;
    rounded - u8::from(value < f32::from(rounded) - 0.5)
}

/// 背景との差がしきい値を超えた画素を前景とする。`deltas` と `row` は 1 行ぶんの作業用 (幅 x 3 と、幅)。
fn threshold(
    frame: &[u8],
    background: &[f32],
    threshold: f64,
    deltas: &mut [u8],
    row: &mut [u8],
    out: &mut BitImage,
) {
    // 差は整数なので、「差 > しきい値」は「差 > しきい値の切り捨て」と同じ。しきい値が負なら全部、255 以上なら
    // どれも超えない (NaN も超えない)
    let limit: i16 = if threshold.is_nan() {
        255
    } else {
        threshold.floor().clamp(-1.0, 255.0) as i16
    };
    let w = out.width;
    for y in 0..out.height {
        let values = y * w * 3..(y + 1) * w * 3;
        // チャンネルを区別せずに並びのまま差を取る (ベクトル命令が使える)。画素ごとに最も大きな差で判定する
        for ((d, &f), &b) in deltas
            .iter_mut()
            .zip(&frame[values.clone()])
            .zip(&background[values])
        {
            *d = f.abs_diff(to_u8(b));
        }
        for (value, d) in row.iter_mut().zip(deltas.as_chunks::<3>().0) {
            *value = u8::from(i16::from(d[0].max(d[1]).max(d[2])) > limit);
        }
        out.set_row(y, row);
    }
}

// --- 1 画素 1 ビットの画像 ---

/// 前景かどうかを 1 画素 1 ビットで持つ画像。1 行を 64 画素ずつの整数に詰める (画素 x は x / 64 番目の
/// 整数の x % 64 ビット目)。行の右端の余りのビットはいつも 0。
#[derive(Debug, Clone, PartialEq, Eq)]
struct BitImage {
    width: usize,
    height: usize,
    words_per_row: usize,
    words: Vec<u64>,
}

impl BitImage {
    fn new(width: usize, height: usize) -> Self {
        let words_per_row = width.div_ceil(64);
        Self {
            width,
            height,
            words_per_row,
            words: vec![0; words_per_row * height],
        }
    }

    fn row(&self, y: usize) -> &[u64] {
        &self.words[y * self.words_per_row..(y + 1) * self.words_per_row]
    }

    fn get(&self, x: usize, y: usize) -> bool {
        self.words[y * self.words_per_row + x / 64] >> (x % 64) & 1 != 0
    }

    /// 1 行を、画素ごとの 0 / 1 から詰める。
    fn set_row(&mut self, y: usize, values: &[u8]) {
        let words = &mut self.words[y * self.words_per_row..(y + 1) * self.words_per_row];
        for (word, chunk) in words.iter_mut().zip(values.chunks(64)) {
            let mut packed = 0u64;
            for (i, eight) in chunk.chunks(8).enumerate() {
                let mut bytes = [0u8; 8];
                bytes[..eight.len()].copy_from_slice(eight);
                packed |= pack8(u64::from_le_bytes(bytes)) << (i * 8);
            }
            *word = packed;
        }
    }

    /// 1 の画素の通し番号を、ラスタ順に返す。
    fn ones(&self) -> impl Iterator<Item = usize> + '_ {
        self.words.iter().enumerate().flat_map(move |(k, &word)| {
            let (y, x0) = (k / self.words_per_row, k % self.words_per_row * 64);
            let base = y * self.width + x0;
            Ones(word).map(move |bit| base + bit)
        })
    }

    /// 行の右端の余りのビット (画像の外) を 0 にする。
    fn mask_padding(&self, row: &mut [u64]) {
        let tail = self.width % 64;
        if tail != 0
            && let Some(last) = row.last_mut()
        {
            *last &= (1u64 << tail) - 1;
        }
    }
}

/// 8 バイト (どれも 0 か 1) を 8 ビットに詰める。i 番目のバイトが i ビット目になる。
///
/// 掛け算で各バイトを上の 8 ビットに集める。i 番目のバイトは 56 + i ビット目に来て、ほかの組み合わせは
/// 重ならない (繰り上がらない) 位置か、64 ビットの外に出る。
#[inline]
fn pack8(bytes: u64) -> u64 {
    (bytes & 0x0101_0101_0101_0101).wrapping_mul(0x0102_0408_1020_4080) >> 56
}

/// 整数の 1 のビットの位置を、小さいほうから返す。
struct Ones(u64);

impl Iterator for Ones {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        if self.0 == 0 {
            return None;
        }
        let bit = self.0.trailing_zeros() as usize;
        self.0 &= self.0 - 1;
        Some(bit)
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
// 収縮は、白黒を反転して膨張し、もう一度反転したものと同じ (核が上下左右に対称で、画像の外も反転する) なので、
// 膨張だけを書く。

/// 1 行を横に 2 画素ずつ広げる (横 5 の膨張)。画像の外は 0。
fn dilate_row5(src: &[u64], dst: &mut [u64]) {
    let n = src.len();
    for k in 0..n {
        let here = src[k];
        let before = if k > 0 { src[k - 1] } else { 0 };
        let after = if k + 1 < n { src[k + 1] } else { 0 };
        // 画素 x に x - 1, x - 2 (左) と x + 1, x + 2 (右) の値を重ねる
        let from_left = (here << 1 | before >> 63) | (here << 2 | before >> 62);
        let from_right = (here >> 1 | after << 63) | (here >> 2 | after << 62);
        dst[k] = here | from_left | from_right;
    }
}

/// 5x5 の楕円で膨張する。画像の外は 0。`rows` は 1 行ぶんの作業用 3 本 (上、今、下の行を横に広げたもの)。
fn dilate_ellipse5(src: &BitImage, dst: &mut BitImage, rows: &mut [Vec<u64>; 3]) {
    let (h, n) = (src.height, src.words_per_row);
    let empty = vec![0u64; n];
    let row_or_empty = |y: isize| {
        if y >= 0 && (y as usize) < h {
            src.row(y as usize)
        } else {
            empty.as_slice()
        }
    };
    // rows[0], rows[1], rows[2] に、y - 1, y, y + 1 の行を横に広げたものを置き、y を進めるたびに回す
    rows[0].iter_mut().for_each(|v| *v = 0);
    dilate_row5(row_or_empty(0), &mut rows[1]);
    for y in 0..h {
        dilate_row5(row_or_empty(y as isize + 1), &mut rows[2]);
        let (up2, up1, here, down1, down2) = (
            row_or_empty(y as isize - 2),
            row_or_empty(y as isize - 1),
            src.row(y),
            row_or_empty(y as isize + 1),
            row_or_empty(y as isize + 2),
        );
        let out = &mut dst.words[y * n..(y + 1) * n];
        for k in 0..n {
            let rect = rows[0][k] | rows[1][k] | rows[2][k];
            let line = up2[k] | up1[k] | here[k] | down1[k] | down2[k];
            out[k] = rect | line;
        }
        src.mask_padding(out);
        rows.rotate_left(1);
    }
}

/// 5x5 の楕円で収縮する。画像の外は 1。`inverted` は src と同じ大きさの作業用。
fn erode_ellipse5(
    src: &BitImage,
    dst: &mut BitImage,
    inverted: &mut BitImage,
    rows: &mut [Vec<u64>; 3],
) {
    // 反転して膨張し、もう一度反転する
    let n = src.words_per_row;
    for y in 0..src.height {
        let row = &mut inverted.words[y * n..(y + 1) * n];
        for (v, &s) in row.iter_mut().zip(src.row(y)) {
            *v = !s;
        }
        src.mask_padding(row);
    }
    dilate_ellipse5(inverted, dst, rows);
    for y in 0..dst.height {
        let row = &mut dst.words[y * n..(y + 1) * n];
        row.iter_mut().for_each(|v| *v = !*v);
        src.mask_padding(row);
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

/// 前景の横の連なり (1 行のうち、続いている前景の画素)。`x1` も連なりに含む。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Run {
    y: usize,
    x0: usize,
    x1: usize,
}

/// 番号付けの作業用。フレームごとに確保しない。
#[derive(Default)]
struct LabelScratch {
    runs: Vec<Run>,
    /// 連なりどうしのつながり (union-find)。根が同じ連なりは同じ領域
    parent: Vec<usize>,
    /// 根の連なりに付けた番号 (0 はまだ付けていない)
    label_of: Vec<u32>,
}

/// 1 行の前景の連なりを、左から順に `runs` に足す。
fn push_runs(row: &[u64], y: usize, runs: &mut Vec<Run>) {
    // 前の整数から続いている連なりの始まり
    let mut start: Option<usize> = None;
    for (k, &word) in row.iter().enumerate() {
        let base = k * 64;
        // この整数のうち、`pos` より下のビットは見終わった
        let mut pos = 0;
        while pos < 64 {
            let rest = word >> pos;
            match start {
                None => {
                    if rest == 0 {
                        break;
                    }
                    pos += rest.trailing_zeros() as usize;
                    start = Some(base + pos);
                }
                Some(x0) => {
                    // `pos` から続く 1 の数。上から入る 0 は反転すると 1 になるので、64 - pos を超えない
                    pos += (!rest).trailing_zeros() as usize;
                    if pos < 64 {
                        runs.push(Run {
                            y,
                            x0,
                            x1: base + pos - 1,
                        });
                        start = None;
                    }
                }
            }
        }
    }
    // 行の右端まで続いた (右端の余りのビットは 0 なので、幅がちょうど 64 の倍数のときだけ)
    if let Some(x0) = start {
        runs.push(Run {
            y,
            x0,
            x1: row.len() * 64 - 1,
        });
    }
}

fn find(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

/// 8 近傍でつながった前景の領域に番号 (1 から) を付ける。`labels` は前景の画素がすべて 0 であること。
/// 番号は、ラスタ順で最初の画素が先に来る領域から振る (1 画素ずつ塗り広げたときと同じ番号になる)。
///
/// 1 画素ずつ塗り広げると、人が写ったときのような大きな領域で遅いので、横の連なりを単位にする。
/// 上の行の連なりと、横に 1 画素広げて重なれば (斜めも含めて隣り合えば) 同じ領域。
fn label_components(
    binary: &BitImage,
    labels: &mut [u32],
    scratch: &mut LabelScratch,
) -> Vec<Component> {
    let w = binary.width;
    let LabelScratch {
        runs,
        parent,
        label_of,
    } = scratch;
    runs.clear();
    parent.clear();
    let mut previous = 0..0;
    for y in 0..binary.height {
        let first = runs.len();
        push_runs(binary.row(y), y, runs);
        let current = first..runs.len();
        parent.extend(current.clone());
        // 上の行の連なりは左から並んでいるので、重なりうる最初のものを順に進めながら探す
        let mut j = previous.start;
        for i in current.clone() {
            let Run { x0, x1, .. } = runs[i];
            while j < previous.end && runs[j].x1 + 1 < x0 {
                j += 1;
            }
            let mut k = j;
            while k < previous.end && runs[k].x0 <= x1 + 1 {
                let (a, b) = (find(parent, i), find(parent, k));
                if a != b {
                    parent[a.max(b)] = a.min(b);
                }
                k += 1;
            }
        }
        previous = current;
    }

    label_of.clear();
    label_of.resize(runs.len(), 0);
    let mut components: Vec<Component> = Vec::new();
    for (i, &run) in runs.iter().enumerate() {
        let root = find(parent, i);
        if label_of[root] == 0 {
            // 連なりはラスタ順に並んでいるので、領域で最初に出てきた連なりの左端が、領域の最初の画素
            let label = components.len() as u32 + 1;
            components.push(Component {
                label,
                start: (run.x0, run.y),
                min_x: run.x0,
                min_y: run.y,
                max_x: run.x1,
                max_y: run.y,
            });
            label_of[root] = label;
        }
        let label = label_of[root];
        let component = &mut components[label as usize - 1];
        component.min_x = component.min_x.min(run.x0);
        component.max_x = component.max_x.max(run.x1);
        component.max_y = component.max_y.max(run.y);
        labels[run.y * w + run.x0..=run.y * w + run.x1].fill(label);
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

/// 色みの判定の作業用。フレームごとに確保しない。
#[derive(Default)]
struct ColorScratch {
    outside: Vec<bool>,
    stack: Vec<(usize, usize)>,
}

/// 領域 (穴を含む) の平均色が、背景の同じ場所の平均色から色みをどれだけ変えたか (8 ビットの階調)。
///
/// 次の 2 つの残りの小さいほう。アプリの ObjectTracker.colorShift と同じ式。
/// 1. 明るさだけが何倍かになったとみなしたときの残り
/// 2. 全チャンネルが同じ量だけ増減したとみなしたときの残り
///
/// 背景は、8 ビットに直してから平均する ([`to_u8`])。アプリが 8 ビットの背景と比べるのに合わせる。
fn color_shift(
    frame: &[u8],
    background: &[f32],
    labels: &[u32],
    w: usize,
    label: u32,
    rect: Rect,
    scratch: &mut ColorScratch,
) -> f64 {
    let (rx, ry, rw, rh) = (
        rect.x as usize,
        rect.y as usize,
        rect.width as usize,
        rect.height as usize,
    );
    // 四角形の縁から 4 近傍でたどれる「領域でない画素」が外。それ以外 (領域とその穴) を平均する。
    // 大きな領域でも速いよう、横の連なりごとに塗り広げる
    let ColorScratch { outside, stack } = scratch;
    outside.clear();
    outside.resize(rw * rh, false);
    stack.clear();
    let in_region = |x: usize, y: usize| labels[(ry + y) * w + rx + x] == label;
    for x in 0..rw {
        stack.push((x, 0));
        stack.push((x, rh - 1));
    }
    for y in 0..rh {
        stack.push((0, y));
        stack.push((rw - 1, y));
    }
    while let Some((x, y)) = stack.pop() {
        let open = |outside: &[bool], x: usize, y: usize| !outside[y * rw + x] && !in_region(x, y);
        if !open(outside, x, y) {
            continue;
        }
        let (mut left, mut right) = (x, x);
        while left > 0 && open(outside, left - 1, y) {
            left -= 1;
        }
        while right + 1 < rw && open(outside, right + 1, y) {
            right += 1;
        }
        outside[y * rw + left..=y * rw + right].fill(true);
        // 上下の行で、この連なりの真上と真下にある「外になりうる」連なりごとに 1 つずつ種を置く
        for ny in [y.wrapping_sub(1), y + 1] {
            if ny >= rh {
                continue;
            }
            let mut nx = left;
            while nx <= right {
                if open(outside, nx, ny) {
                    stack.push((nx, ny));
                    while nx <= right && open(outside, nx, ny) {
                        nx += 1;
                    }
                } else {
                    nx += 1;
                }
            }
        }
    }
    // 合計は整数で取る (浮動小数で 1 つずつ足しても、2^53 に届かないので同じ値になる)
    let mut now = [0u64; 3];
    let mut before = [0u64; 3];
    let mut count = 0u64;
    for y in 0..rh {
        for x in 0..rw {
            if outside[y * rw + x] {
                continue;
            }
            let p = (ry + y) * w + rx + x;
            for c in 0..3 {
                now[c] += u64::from(frame[p * 3 + c]);
                before[c] += u64::from(to_u8(background[p * 3 + c]));
            }
            count += 1;
        }
    }
    if count == 0 {
        return 0.0;
    }
    let count = count as f64;
    let now = now.map(|v| v as f64 / count);
    let before = before.map(|v| v as f64 / count);
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

    /// テスト用の決まった乱数 (xorshift)。
    struct Random(u64);

    impl Random {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }

    fn bits(w: usize, h: usize, values: &[u8]) -> BitImage {
        let mut image = BitImage::new(w, h);
        for y in 0..h {
            image.set_row(y, &values[y * w..(y + 1) * w]);
        }
        image
    }

    fn unpack(image: &BitImage) -> Vec<u8> {
        (0..image.height)
            .flat_map(|y| (0..image.width).map(move |x| u8::from(image.get(x, y))))
            .collect()
    }

    /// 5x5 の楕円の形の処理を、画素ごとに素直に計算したもの。画像の外は、膨張では 0、収縮では 1。
    fn reference_morphology(src: &[u8], w: usize, h: usize, dilate: bool) -> Vec<u8> {
        let kernel: Vec<(i32, i32)> = (-2i32..=2)
            .flat_map(|dy| (-2i32..=2).map(move |dx| (dx, dy)))
            .filter(|&(dx, dy)| dy.abs() <= 1 || dx == 0)
            .collect();
        let mut out = vec![0u8; w * h];
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                let values = kernel.iter().map(|&(dx, dy)| {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        u8::from(!dilate)
                    } else {
                        src[ny as usize * w + nx as usize]
                    }
                });
                let value = if dilate { values.max() } else { values.min() };
                out[y as usize * w + x as usize] = value.expect("kernel is not empty");
            }
        }
        out
    }

    #[test]
    fn ellipse_kernel_matches_opencv() {
        // 1 画素を膨張すると、OpenCV の 5x5 の楕円の形になる
        let (w, h) = (9, 9);
        let mut values = vec![0u8; w * h];
        values[4 * w + 4] = 1;
        let mut dst = BitImage::new(w, h);
        dilate_ellipse5(
            &bits(w, h, &values),
            &mut dst,
            &mut [vec![0; 1], vec![0; 1], vec![0; 1]],
        );
        let dst = unpack(&dst);
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
        let mut dst = BitImage::new(w, h);
        let mut inverted = BitImage::new(w, h);
        erode_ellipse5(
            &bits(w, h, &vec![1u8; w * h]),
            &mut dst,
            &mut inverted,
            &mut [vec![0; 1], vec![0; 1], vec![0; 1]],
        );
        assert!(unpack(&dst).iter().all(|&v| v == 1));
    }

    #[test]
    fn morphology_matches_a_per_pixel_reference() {
        let mut random = Random(0x9e37_79b9_7f4a_7c15);
        // 64 の倍数とその前後 (整数の境目)、1 画素、細長い画像
        for (w, h) in [
            (1, 1),
            (5, 3),
            (63, 7),
            (64, 6),
            (65, 9),
            (130, 11),
            (640, 4),
            (3, 40),
        ] {
            for density in [2, 8, 64] {
                let values: Vec<u8> = (0..w * h)
                    .map(|_| u8::from(random.next() % 128 < density))
                    .collect();
                let src = bits(w, h, &values);
                let n = src.words_per_row;
                let mut rows = [vec![0; n], vec![0; n], vec![0; n]];
                let mut dst = BitImage::new(w, h);
                dilate_ellipse5(&src, &mut dst, &mut rows);
                assert_eq!(
                    unpack(&dst),
                    reference_morphology(&values, w, h, true),
                    "dilate {w}x{h} density {density}"
                );
                let mut inverted = BitImage::new(w, h);
                erode_ellipse5(&src, &mut dst, &mut inverted, &mut rows);
                assert_eq!(
                    unpack(&dst),
                    reference_morphology(&values, w, h, false),
                    "erode {w}x{h} density {density}"
                );
                // 右端の余りのビットは 0 のまま
                assert_eq!(dst, bits(w, h, &unpack(&dst)));
            }
        }
    }

    #[test]
    fn to_u8_matches_round_and_clamp() {
        let reference = |v: f32| v.abs().round().clamp(0.0, 255.0) as u8;
        let edges = [
            0.0,
            -0.0,
            0.25,
            f32::from_bits(0.5f32.to_bits() - 1),
            0.5,
            0.75,
            1.0,
            1.5,
            2.5,
            f32::from_bits(1.5f32.to_bits() - 1),
            126.5,
            f32::from_bits(127.5f32.to_bits() - 1),
            254.5,
            254.6,
            255.0,
            255.49,
            255.5,
            256.0,
            300.0,
            1e10,
            f32::INFINITY,
            -0.5,
            -1.5,
            -254.5,
            -300.0,
            f32::NAN,
        ];
        for v in edges {
            assert_eq!(to_u8(v), reference(v), "{v} ({:#x})", v.to_bits());
        }
        // 背景の値は、整数を重み付きで平均したもの (2 進の小数)。いろいろな桁の値で確かめる
        let mut random = Random(42);
        for _ in 0..1_000_000 {
            let v = (random.next() % (300 << 20)) as f32 / (1 << 20) as f32;
            assert_eq!(to_u8(v), reference(v), "{v}");
            let bits = 0x3e80_0000 + (random.next() % 0x0c00_0000) as u32; // 0.25 から 300 ほどまでの全てのビットの並び
            let v = f32::from_bits(bits);
            assert_eq!(to_u8(v), reference(v), "{v} ({bits:#x})");
        }
    }

    #[test]
    fn threshold_matches_a_per_pixel_reference() {
        let (w, h) = (70, 5);
        let mut random = Random(7);
        let frame: Vec<u8> = (0..w * h * 3).map(|_| random.next() as u8).collect();
        let background: Vec<f32> = (0..w * h * 3)
            .map(|_| (random.next() % (256 << 4)) as f32 / 16.0)
            .collect();
        for limit in [
            15.0,
            15.5,
            0.0,
            -0.5,
            -10.0,
            254.9,
            255.0,
            300.0,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ] {
            let mut out = BitImage::new(w, h);
            threshold(
                &frame,
                &background,
                limit,
                &mut vec![0; w * 3],
                &mut vec![0; w],
                &mut out,
            );
            let expected: Vec<u8> = (0..w * h)
                .map(|p| {
                    let delta = (0..3)
                        .map(|c| {
                            frame[p * 3 + c].abs_diff(
                                background[p * 3 + c].abs().round().clamp(0.0, 255.0) as u8,
                            )
                        })
                        .max()
                        .expect("3 channels");
                    u8::from(f64::from(delta) > limit)
                })
                .collect();
            assert_eq!(unpack(&out), expected, "threshold {limit}");
        }
    }

    #[test]
    fn ones_lists_set_pixels_in_raster_order() {
        let (w, h) = (70, 3);
        let mut values = vec![0u8; w * h];
        for i in [0, 63, 64, 69, 70, 139, 140, 209] {
            values[i] = 1;
        }
        assert_eq!(
            bits(w, h, &values).ones().collect::<Vec<_>>(),
            vec![0, 63, 64, 69, 70, 139, 140, 209]
        );
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
    fn a_ball_that_moves_on_leaves_no_trace_behind() {
        // 前のフレームの前景は背景に取り込まず、番号も消すので、ボールが通り過ぎた跡は検出しない
        let (w, h) = (160, 90);
        let background = [120u8, 110, 100];
        let mut tracker = ObjectTracker::new(TrackerParams::APP_DEFAULT);
        tracker.next_frame(&image(w, h, background), w, h);
        for (i, cx) in [40.0, 60.0, 80.0].into_iter().enumerate() {
            let mut frame = image(w, h, background);
            fill_disc(&mut frame, w, cx, 40.0, 8.0, [220, 200, 40]);
            let rects = tracker.next_frame(&frame, w, h);
            assert_eq!(
                rects,
                vec![Rect {
                    x: cx as i32 - 7,
                    y: 32,
                    width: 15,
                    height: 17
                }],
                "frame {i}"
            );
        }
        assert!(
            tracker
                .next_frame(&image(w, h, background), w, h)
                .is_empty()
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
