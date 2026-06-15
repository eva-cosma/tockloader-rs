pub struct BoardSettings {
    pub arch: Option<String>,
    pub start_address: u64,
}

// TODO(eva-cosma): Does a default implementation make sense for this? Is a
// 'None' architechture a sane idea?
impl Default for BoardSettings {
    fn default() -> Self {
        Self {
            arch: None,
            start_address: 0x30000,
        }
    }
}
