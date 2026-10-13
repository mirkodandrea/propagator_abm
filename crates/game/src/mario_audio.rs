//! Bounded PCM handoff from the main-thread native engine to Bevy's audio sink.
use bevy::{audio::{Decodable, Source}, prelude::*};
use std::{collections::VecDeque, sync::{Arc, Mutex}, time::Duration};

const MAX_SAMPLES: usize = libsm64_native::AUDIO_RATE as usize * 2 / 5;
const BLOCK_SAMPLES: usize = 512;

#[derive(Default)]
struct Queue {
    samples: VecDeque<i16>,
    active: bool,
}

#[derive(Asset, TypePath, Clone, Default)]
pub(super) struct NativeAudio {
    queue: Arc<Mutex<Queue>>,
}

impl NativeAudio {
    pub fn start(&self) {
        let mut q = self.queue.lock().unwrap();
        q.samples.clear();
        q.active = true;
    }
    pub fn stop(&self) {
        let mut q = self.queue.lock().unwrap();
        q.active = false;
        q.samples.clear();
    }
    pub fn clear(&self) { self.queue.lock().unwrap().samples.clear(); }
    pub fn queued_frames(&self) -> usize { self.queue.lock().unwrap().samples.len() / 2 }
    pub fn push(&self, samples: &[i16]) {
        let mut q = self.queue.lock().unwrap();
        if !q.active { return; }
        q.samples.extend(samples);
        let excess = q.samples.len().saturating_sub(MAX_SAMPLES);
        q.samples.drain(..excess);
    }
}

pub(super) struct PcmDecoder {
    queue: Arc<Mutex<Queue>>,
    block: Vec<i16>,
    index: usize,
}

impl Iterator for PcmDecoder {
    type Item = i16;
    fn next(&mut self) -> Option<i16> {
        if self.index >= self.block.len() {
            let mut q = self.queue.lock().unwrap();
            if !q.active { return None; }
            self.block.clear();
            let count = q.samples.len().min(BLOCK_SAMPLES);
            self.block.extend(q.samples.drain(..count));
            // Never block on native synthesis in the real-time callback.
            if self.block.is_empty() { self.block.resize(BLOCK_SAMPLES, 0); }
            self.index = 0;
        }
        let sample = self.block[self.index];
        self.index += 1;
        Some(sample)
    }
}

impl Source for PcmDecoder {
    fn current_frame_len(&self) -> Option<usize> { None }
    fn channels(&self) -> u16 { 2 }
    fn sample_rate(&self) -> u32 { libsm64_native::AUDIO_RATE }
    fn total_duration(&self) -> Option<Duration> { None }
}

impl Decodable for NativeAudio {
    type DecoderItem = i16;
    type Decoder = PcmDecoder;
    fn decoder(&self) -> PcmDecoder {
        PcmDecoder { queue: self.queue.clone(), block: Vec::with_capacity(BLOCK_SAMPLES), index: 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pcm_stereo_order_underflow_and_stop() {
        let source = NativeAudio::default();
        source.start();
        source.push(&[10, -10, 20, -20]);
        let mut decoder = source.decoder();
        assert_eq!(decoder.by_ref().take(4).collect::<Vec<_>>(), [10, -10, 20, -20]);
        assert_eq!(decoder.next(), Some(0));
        source.stop();
        for _ in 1..BLOCK_SAMPLES { assert_eq!(decoder.next(), Some(0)); }
        assert_eq!(decoder.next(), None);
        source.start();
        assert_eq!(source.queued_frames(), 0);
    }
    #[test]
    fn a_missing_audio_device_cannot_accumulate_unbounded_latency() {
        let source = NativeAudio::default();
        source.start();
        source.push(&vec![1; MAX_SAMPLES]);
        source.push(&[2, 3]);
        assert_eq!(source.queued_frames(), MAX_SAMPLES / 2);
        let samples: Vec<_> = source.decoder().take(MAX_SAMPLES).collect();
        assert_eq!(&samples[MAX_SAMPLES - 2..], &[2, 3]);
    }
}
