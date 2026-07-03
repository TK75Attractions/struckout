#[derive(Debug)]
pub struct FrameEntity {
    pub timestamp: i64,
    pub session_id: String,
    pub data: Vec<u8>,
}
