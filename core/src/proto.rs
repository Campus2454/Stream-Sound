//! Wire format. Everything is little-endian UDP datagrams.
//!
//! Audio packet (kind 1):
//!   0..2   magic "SS"
//!   2      version (1)
//!   3      kind
//!   4      ttl: hops this packet may still be forwarded
//!   5      channels (1 or 2)
//!   6..8   frames in this packet
//!   8..12  stream id (random per sender session)
//!   12..16 sequence number
//!   16..20 sample rate
//!   20     codec (0 = PCM signed 16-bit)
//!   21..24 reserved
//!   24..   payload
//!
//! Stream info packet (kind 2): same header, payload is the UTF-8 stream name.

pub const MAGIC: [u8; 2] = *b"SS";
pub const VERSION: u8 = 1;
pub const HEADER_LEN: usize = 24;
pub const KIND_AUDIO: u8 = 1;
pub const KIND_INFO: u8 = 2;
pub const CODEC_PCM16: u8 = 0;
pub const DEFAULT_TTL: u8 = 8;
pub const DEFAULT_AUDIO_PORT: u16 = 47800;
pub const DISCOVERY_PORT: u16 = 47801;
/// Largest datagram we ever produce or accept.
pub const MAX_PACKET: usize = 1500;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    pub kind: u8,
    pub ttl: u8,
    pub channels: u8,
    pub frames: u16,
    pub stream_id: u32,
    pub seq: u32,
    pub sample_rate: u32,
    pub codec: u8,
}

impl Header {
    pub fn write(&self, out: &mut [u8]) {
        out[0..2].copy_from_slice(&MAGIC);
        out[2] = VERSION;
        out[3] = self.kind;
        out[4] = self.ttl;
        out[5] = self.channels;
        out[6..8].copy_from_slice(&self.frames.to_le_bytes());
        out[8..12].copy_from_slice(&self.stream_id.to_le_bytes());
        out[12..16].copy_from_slice(&self.seq.to_le_bytes());
        out[16..20].copy_from_slice(&self.sample_rate.to_le_bytes());
        out[20] = self.codec;
        out[21..24].fill(0);
    }

    pub fn parse(buf: &[u8]) -> Option<Header> {
        if buf.len() < HEADER_LEN || buf[0..2] != MAGIC || buf[2] != VERSION {
            return None;
        }
        let h = Header {
            kind: buf[3],
            ttl: buf[4],
            channels: buf[5],
            frames: u16::from_le_bytes([buf[6], buf[7]]),
            stream_id: u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]),
            seq: u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]),
            sample_rate: u32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]),
            codec: buf[20],
        };
        if h.kind == KIND_AUDIO {
            let need = HEADER_LEN + h.frames as usize * h.channels as usize * 2;
            if h.codec != CODEC_PCM16
                || !(1..=2).contains(&h.channels)
                || h.frames == 0
                || !(8_000..=192_000).contains(&h.sample_rate)
                || buf.len() < need
            {
                return None;
            }
        }
        Some(h)
    }
}

/// Decode PCM16 payload into f32 samples, appending to `out`.
pub fn decode_pcm16(payload: &[u8], out: &mut Vec<f32>) {
    out.extend(
        payload
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0),
    );
}

/// Encode f32 samples as PCM16 into `out` (which must be samples.len()*2 long).
pub fn encode_pcm16(samples: &[f32], out: &mut [u8]) {
    for (s, o) in samples.iter().zip(out.chunks_exact_mut(2)) {
        let v = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        o.copy_from_slice(&v.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_roundtrip() {
        let h = Header {
            kind: KIND_AUDIO,
            ttl: 5,
            channels: 2,
            frames: 240,
            stream_id: 0xdead_beef,
            seq: 42,
            sample_rate: 48_000,
            codec: CODEC_PCM16,
        };
        let mut buf = vec![0u8; HEADER_LEN + 240 * 4];
        h.write(&mut buf);
        assert_eq!(Header::parse(&buf), Some(h));
        assert_eq!(Header::parse(&buf[..100]), None, "truncated payload rejected");
    }

    #[test]
    fn pcm_roundtrip() {
        let src = [0.0f32, 0.5, -0.5, 1.0, -1.0];
        let mut bytes = vec![0u8; src.len() * 2];
        encode_pcm16(&src, &mut bytes);
        let mut back = Vec::new();
        decode_pcm16(&bytes, &mut back);
        for (a, b) in src.iter().zip(&back) {
            assert!((a - b).abs() < 1e-3);
        }
    }
}
