//! カメラから受け取った動画を、あとで解析し直せるように保存する (`--video-record-dir`)。
//!
//! 接続ごとに 2 つのファイルを書く。名前はカメラの番号と、最初の設定を受け取った時刻 (この PC の時計)。
//! - `camera<番号>_<時刻>.vsp`: 受け取った [`VideoStreamPacket`] をそのままの順に。1 件ごとに
//!   「受け取った時刻 (UNIX epoch からの ns、i64)、長さ (u32)、Protobuf」で、数値はリトルエンディアン。
//!   設定 (途中で変わったものも)、SPS/PPS、フレームとそのセンサー時刻がすべて入る。[`read_recording`] で読む
//! - `camera<番号>_<時刻>.h264`: 動画だけを並べたもの (Annex B)。VLC や ffplay で見られる
//!
//! 書き込みは専用のスレッドで行い、受信を止めない。待ちデータが上限を超えると、その接続の録画だけを停止する。

use std::{
    fs::File,
    io::{BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self as std_mpsc, SyncSender, TrySendError},
    },
    thread,
};

use chrono::{DateTime, Utc};
use prost::Message;
use struckout_proto::{VideoStreamPacket, video_stream_packet};
use tracing::{info, warn};

/// 遅いディスクに合わせてメモリが増え続けないよう、書き込み中を含むデータ量を制限する。
const MAX_PENDING_BYTES: usize = 8 << 20;
/// 小さいパケットが続いても、キュー自体の管理領域を増やし続けない。
const MAX_PENDING_PACKETS: usize = 240;

pub struct Recorder {
    camera_id: u32,
    tx: Option<SyncSender<RecorderPacketData>>,
    state: Arc<RecordingState>,
    worker: Option<thread::JoinHandle<()>>,
}

#[derive(Default)]
struct RecordingState {
    pending_bytes: AtomicUsize,
    stopped: AtomicBool,
}

struct RecorderPacketData {
    received_ns: i64,
    video_packet: Vec<u8>,
    h264: Option<Vec<u8>>,
}

impl RecorderPacketData {
    fn byte_size(&self) -> usize {
        12 + self.video_packet.len() + self.h264.as_ref().map_or(0, Vec::len)
    }
}

impl Recorder {
    /// `dir` に、カメラ `camera_id` のファイルを作る。
    pub fn create(dir: &Path, camera_id: u32, started_at: DateTime<Utc>) -> std::io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let stem = format!("camera{camera_id}_{}", started_at.format("%Y%m%d-%H%M%S"));
        let packets_path = dir.join(format!("{stem}.vsp"));
        let video_path = dir.join(format!("{stem}.h264"));
        let mut packets = BufWriter::with_capacity(1 << 20, File::create(&packets_path)?);
        let mut video = BufWriter::with_capacity(1 << 20, File::create(&video_path)?);
        info!(camera_id, path = %packets_path.display(), "recording the video stream");
        let (tx, rx) = std_mpsc::sync_channel::<RecorderPacketData>(MAX_PENDING_PACKETS);
        let state = Arc::new(RecordingState::default());
        let worker_state = Arc::clone(&state);
        let worker = thread::Builder::new().name(format!("video-record-{camera_id}")).spawn(move || {
            while let Ok(data) = rx.recv() {
                let byte_size = data.byte_size();
                let RecorderPacketData { received_ns, video_packet: bytes, h264 } = data;
                let length = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
                let result = packets
                    .write_all(&received_ns.to_le_bytes())
                    .and_then(|()| packets.write_all(&length.to_le_bytes()))
                    .and_then(|()| packets.write_all(&bytes))
                    .and_then(|()| h264.map_or(Ok(()), |data| video.write_all(&data)));
                // 書き込み中のデータも予算に含め、書き終えたら受信側に返す。
                drop(bytes);
                worker_state.pending_bytes.fetch_sub(byte_size, Ordering::AcqRel);
                if let Err(e) = result {
                    if !worker_state.stopped.swap(true, Ordering::AcqRel) {
                        warn!(camera_id, error = ?e, "failed to record the video stream; stopped recording");
                    }
                    break;
                }
            }
            let packets_result = packets.flush();
            let video_result = video.flush();
            if let Err(e) = packets_result.and(video_result)
                && !worker_state.stopped.swap(true, Ordering::AcqRel)
            {
                warn!(camera_id, error = ?e, "failed to flush the video stream; stopped recording");
            }
            info!(camera_id, path = %packets_path.display(), "finished recording the video stream");
        })?;
        Ok(Self {
            camera_id,
            tx: Some(tx),
            state,
            worker: Some(worker),
        })
    }

    /// 受け取ったパケットを待たずに記録へ渡す。上限を超えた後も解析を続けられるよう、録画だけを停止する。
    pub fn record(&mut self, received_at: DateTime<Utc>, packet: &VideoStreamPacket) {
        if self.state.stopped.load(Ordering::Acquire) {
            self.tx = None;
            return;
        }
        let Some(tx) = &self.tx else { return };
        let h264 = match &packet.data {
            Some(video_stream_packet::Data::Frame(frame)) => Some(&frame.data),
            _ => None,
        };
        // 複製・符号化の前に予算を確保する。上限より大きな単独パケットも受け付けない。
        let byte_size = 12 + packet.encoded_len() + h264.map_or(0, Vec::len);
        if self
            .state
            .pending_bytes
            .try_update(Ordering::AcqRel, Ordering::Acquire, |pending| {
                pending
                    .checked_add(byte_size)
                    .filter(|total| *total <= MAX_PENDING_BYTES)
            })
            .is_err()
        {
            self.stop("recording byte limit reached");
            return;
        }
        let received_ns = received_at.timestamp_nanos_opt().unwrap_or_default();
        let result = tx.try_send(RecorderPacketData {
            received_ns,
            video_packet: packet.encode_to_vec(),
            h264: h264.cloned(),
        });
        if let Err(error) = result {
            self.state
                .pending_bytes
                .fetch_sub(byte_size, Ordering::AcqRel);
            self.stop(match error {
                TrySendError::Full(_) => "recording packet limit reached",
                TrySendError::Disconnected(_) => "recording worker disconnected",
            });
        }
    }

    fn stop(&mut self, reason: &'static str) {
        // 受理済みの連続した先頭部分だけを書き切る。欠けた H.264 を黙って記録し続けない。
        self.tx = None;
        if !self.state.stopped.swap(true, Ordering::AcqRel) {
            warn!(
                camera_id = self.camera_id,
                reason,
                max_pending_bytes = MAX_PENDING_BYTES,
                "stopped recording the video stream; analysis continues"
            );
        }
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        // 送り口を閉じると、書き込みのスレッドは残りを書いて終わる
        self.tx = None;
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// [`Recorder`] が書いた `.vsp` を、先頭から 1 件ずつ読む。受け取った時刻とパケットを、受け取った順に返す。
/// 1 時間で 1 台 10GB を超えるので、全体をメモリに読み込まない。
pub struct RecordingReader {
    reader: BufReader<File>,
}

impl RecordingReader {
    pub fn open(path: &Path) -> std::io::Result<Self> {
        Ok(Self {
            reader: BufReader::with_capacity(1 << 20, File::open(path)?),
        })
    }
}

impl Iterator for RecordingReader {
    type Item = std::io::Result<(DateTime<Utc>, VideoStreamPacket)>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut header = [0u8; 12];
        match self.reader.read_exact(&mut header) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return None,
            Err(e) => return Some(Err(e)),
        }
        let received_ns = i64::from_le_bytes(header[0..8].try_into().expect("8 bytes"));
        let length = u32::from_le_bytes(header[8..12].try_into().expect("4 bytes")) as usize;
        let mut bytes = vec![0u8; length];
        // 止めたときに書きかけだった最後の 1 件は捨てる
        self.reader.read_exact(&mut bytes).ok()?;
        Some(
            VideoStreamPacket::decode(bytes.as_slice())
                .map(|packet| (DateTime::from_timestamp_nanos(received_ns), packet))
                .map_err(std::io::Error::other),
        )
    }
}

/// `.vsp` をすべて読む (短い録画とテスト用)。
pub fn read_recording(path: &Path) -> std::io::Result<Vec<(DateTime<Utc>, VideoStreamPacket)>> {
    RecordingReader::open(path)?.collect()
}

/// `dir` にある `.vsp` をすべて返す。
pub fn recordings_in(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "vsp"))
        .collect();
    paths.sort();
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use struckout_proto::{VideoFrame, VideoStreamConfig};

    #[test]
    fn reads_back_what_was_recorded() {
        let dir = tempfile::tempdir().unwrap();
        let t0 = DateTime::from_timestamp_nanos(1_792_000_000_123_456_789);
        let config = VideoStreamPacket {
            data: Some(video_stream_packet::Data::Config(VideoStreamConfig {
                camera_id: 3,
                ..Default::default()
            })),
        };
        let frame = VideoStreamPacket {
            data: Some(video_stream_packet::Data::Frame(VideoFrame {
                data: vec![0, 0, 0, 1, 0x65, 7],
                key_frame: true,
                sensor_timestamp_ns: 42,
                ..Default::default()
            })),
        };
        {
            let mut recorder = Recorder::create(dir.path(), 3, t0).unwrap();
            recorder.record(t0, &config);
            recorder.record(t0 + chrono::TimeDelta::milliseconds(8), &frame);
        }
        let paths = recordings_in(dir.path()).unwrap();
        assert_eq!(paths.len(), 1);
        let packets = read_recording(&paths[0]).unwrap();
        assert_eq!(
            packets,
            vec![
                (t0, config),
                (t0 + chrono::TimeDelta::milliseconds(8), frame)
            ]
        );
        let h264 = std::fs::read(paths[0].with_extension("h264")).unwrap();
        assert_eq!(h264, vec![0, 0, 0, 1, 0x65, 7]);
    }
}
