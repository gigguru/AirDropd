//! AWDL channel sequence management.

use crate::constants::{CHANSEQ_LENGTH, SOCIAL_CHANNELS};

#[derive(Debug, Clone)]
pub struct ChannelState {
    pub sequence: [(u8, u8); CHANSEQ_LENGTH],
    pub current_index: usize,
}

impl Default for ChannelState {
    fn default() -> Self {
        Self::new()
    }
}

impl ChannelState {
    pub fn new() -> Self {
        let mut sequence = [(0u8, 0u8); CHANSEQ_LENGTH];
        for (i, slot) in sequence.iter_mut().enumerate() {
            *slot = SOCIAL_CHANNELS[i % SOCIAL_CHANNELS.len()];
        }
        Self {
            sequence,
            current_index: 0,
        }
    }

    pub fn current(&self) -> (u8, u8) {
        self.sequence[self.current_index % CHANSEQ_LENGTH]
    }

    pub fn advance(&mut self) {
        self.current_index = (self.current_index + 1) % CHANSEQ_LENGTH;
    }
}
