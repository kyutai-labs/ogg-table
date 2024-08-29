use anyhow::Result;

// https://xiph.org/vorbis/doc/Vorbis_I_spec.html#x1-630004.2.2
#[derive(Debug, Clone)]
pub struct IdentificationHeader {
    pub audio_channels: u8,
    pub audio_sample_rate: u32,
    pub bitrate_maximum: u32,
    pub bitrate_nominal: u32,
    pub bitrate_minimum: u32,
    pub blocksizes: u8,
}

impl IdentificationHeader {
    pub fn from_reader<R: std::io::Read>(rdr: &mut R) -> Result<Self> {
        use byteorder::{LittleEndian, ReadBytesExt};
        let packet_type = rdr.read_u8()?;
        if packet_type != 1 {
            anyhow::bail!("unexpected packet type for the vorbis ident header {packet_type}");
        }
        let mut magic = [0u8; 6];
        rdr.read_exact(&mut magic)?;
        if magic != *b"vorbis" {
            anyhow::bail!("unexpected vorbis magic in header {magic:?}")
        }
        let vorbis_version = rdr.read_u32::<LittleEndian>()?;
        if vorbis_version != 0 {
            anyhow::bail!("unexpected vorbis version {vorbis_version}")
        }
        let audio_channels = rdr.read_u8()?;
        let audio_sample_rate = rdr.read_u32::<LittleEndian>()?;
        let bitrate_maximum = rdr.read_u32::<LittleEndian>()?;
        let bitrate_nominal = rdr.read_u32::<LittleEndian>()?;
        let bitrate_minimum = rdr.read_u32::<LittleEndian>()?;
        let blocksizes = rdr.read_u8()?;
        Ok(Self {
            audio_channels,
            audio_sample_rate,
            bitrate_maximum,
            bitrate_nominal,
            bitrate_minimum,
            blocksizes,
        })
    }
}
