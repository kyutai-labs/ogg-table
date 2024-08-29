use crate::{ogg, vorbis};
use anyhow::Result;

pub struct OggVorbisReader<R: std::io::Read> {
    decoder: symphonia_codec_vorbis::VorbisDecoder,
    packet_reader: ogg::PacketReader<R>,
    ident: vorbis::IdentificationHeader,
}

impl<R: std::io::Read + std::io::Seek> OggVorbisReader<R> {
    /// Seek to an absolute position specified as a number of bytes in the file.
    pub fn seek(&mut self, header_bytes_pos: u64, move_to_last_segment: bool) -> Result<u64> {
        self.packet_reader.seek(header_bytes_pos, move_to_last_segment)
    }

    pub fn seek_granule_position(
        &mut self,
        target_granule_pos: u64,
        move_to_last_segment: bool,
    ) -> Result<u64> {
        self.packet_reader.seek_granule_position(target_granule_pos, move_to_last_segment)
    }
}

impl<R: std::io::Read> OggVorbisReader<R> {
    pub fn new(rdr: R) -> Result<Self> {
        use symphonia_core::codecs::Decoder;
        let mut packet_reader = ogg::PacketReader::new(rdr)?;
        let ident_header = match packet_reader.next_packet()? {
            None => anyhow::bail!("missing ident header"),
            Some(packet) => packet,
        };
        let ident =
            vorbis::IdentificationHeader::from_reader(&mut std::io::Cursor::new(&ident_header))?;
        if packet_reader.next_packet()?.is_none() {
            anyhow::bail!("missing comment header")
        }
        let setup_header = match packet_reader.next_packet()? {
            None => anyhow::bail!("missing setup header"),
            Some(packet) => packet,
        };
        let buf = [ident_header, setup_header].concat();
        let mut params = symphonia_core::codecs::CodecParameters::new();
        params
            .for_codec(symphonia_core::codecs::CODEC_TYPE_VORBIS)
            .with_sample_rate(ident.audio_sample_rate)
            .with_time_base(symphonia_core::units::TimeBase::new(1, ident.audio_sample_rate))
            .with_extra_data(Box::from(buf));
        let decoder = symphonia_codec_vorbis::VorbisDecoder::try_new(
            &params,
            &symphonia_core::codecs::DecoderOptions { verify: true },
        )?;

        Ok(Self { ident, packet_reader, decoder })
    }

    pub fn channels(&self) -> u8 {
        self.ident.audio_channels
    }

    pub fn sample_rate(&self) -> u32 {
        self.ident.audio_sample_rate
    }

    pub fn decode(&mut self, mut to_skip: usize, len_in_samples: usize) -> Result<Vec<Vec<f32>>> {
        use symphonia_core::codecs::Decoder;

        let max_len = usize::min(len_in_samples, 200_000_000);
        let mut all_data = vec![Vec::with_capacity(max_len); self.channels() as usize];
        self.decoder.reset();

        while let Some(packet) = self.packet_reader.next_packet()? {
            let packet =
                symphonia_core::formats::Packet::new_from_slice(42, 299792458, 13371337, &packet);
            let data = self.decoder.decode(&packet)?;
            let data = match data {
                symphonia_core::audio::AudioBufferRef::F32(v) => v,
                _ => unreachable!(),
            };
            let data = data.planes();
            let data = data.planes();
            let data_len = data[0].len();
            if data_len <= to_skip {
                to_skip -= data_len
            } else {
                for (all_data, data) in all_data.iter_mut().zip(data.iter()) {
                    all_data.extend_from_slice(&data[to_skip..])
                }
                to_skip = 0
            }
            if all_data[0].len() >= len_in_samples {
                for data in all_data.iter_mut() {
                    data.truncate(len_in_samples)
                }
                break;
            }
        }
        Ok(all_data)
    }
}
