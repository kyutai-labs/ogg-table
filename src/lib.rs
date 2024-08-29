use anyhow::Result;

pub mod ogg;
pub mod ogg_vorbis;
pub mod vorbis;

pub use ogg_vorbis::OggVorbisReader;

#[derive(Debug, Clone)]
pub struct Entry {
    pub file_pos: u64,
    pub granule_position: u64,
}

#[derive(Debug, Clone)]
pub struct TableOfContent {
    pub entries: Vec<Entry>,
}

impl TableOfContent {
    /// Build a table of content from the headers of a ogg file.
    pub fn from_ogg_reader<R: std::io::Read + std::io::Seek>(rdr: &mut R) -> Result<Self> {
        let all_headers = ogg::all_headers(rdr)?;
        let entries: Vec<_> = all_headers
            .into_iter()
            .map(|(file_pos, hdr)| Entry { file_pos, granule_position: hdr.granule_position })
            .collect();
        Ok(Self { entries })
    }

    /// Read a table of content file.
    ///
    /// This uses a very simple serialization format.
    pub fn from_reader<R: std::io::Read>(rdr: &mut R) -> Result<Self> {
        use byteorder::{LittleEndian, ReadBytesExt};

        let mut entries = Vec::new();
        while let Ok(file_pos) = rdr.read_u64::<LittleEndian>() {
            let granule_position = rdr.read_u64::<LittleEndian>().unwrap();
            entries.push(Entry { file_pos, granule_position });
        }
        Ok(Self { entries })
    }

    pub fn write<W: std::io::Write>(&self, w: &mut W) -> Result<()> {
        use byteorder::{LittleEndian, WriteBytesExt};
        for entry in self.entries.iter() {
            w.write_u64::<LittleEndian>(entry.file_pos)?;
            w.write_u64::<LittleEndian>(entry.granule_position)?;
        }
        Ok(())
    }

    pub fn last_entry_before(&self, start_pos: u64) -> Option<&Entry> {
        let packet_idx = self.entries.partition_point(|entry| entry.granule_position < start_pos);
        let mut packet_idx = packet_idx.saturating_sub(1);
        while packet_idx < self.entries.len() && self.entries[packet_idx].granule_position == 0 {
            packet_idx += 1;
        }
        self.entries.get(packet_idx)
    }
}

/// Read a sample of data at the given start time and for the target duration from a file. This
/// uses a table file if available and otherwise fallsback to a linear scan.
pub fn read_ogg_vorbis_sample<P: AsRef<std::path::Path>>(
    p: P,
    start_time_sec: f64,
    duration_sec: f64,
) -> Result<(Vec<Vec<f32>>, u32)> {
    let mut ovr = {
        let file = std::fs::File::open(p.as_ref())?;
        let reader = std::io::BufReader::new(file);
        OggVorbisReader::new(reader)?
    };
    let sample_rate = ovr.sample_rate();
    let start_pos = (start_time_sec * sample_rate as f64) as u64;
    let duration = (duration_sec * sample_rate as f64) as u64;

    let table_path = p.as_ref().with_extension("ogg_table");
    let granule_pos = if table_path.is_file() {
        let table_file = std::fs::File::open(&table_path)?;
        let mut table_reader = std::io::BufReader::new(table_file);
        let toc = TableOfContent::from_reader(&mut table_reader)?;
        let entry = match toc.last_entry_before(start_pos) {
            None => anyhow::bail!("not enough audio packets in file"),
            Some(entry) => entry,
        };
        ovr.seek(entry.file_pos, true)?
    } else {
        ovr.seek_granule_position(start_pos, true)?
    };
    let to_skip = start_pos.saturating_sub(granule_pos);
    let data = ovr.decode(to_skip as usize, duration as usize)?;
    Ok((data, sample_rate))
}
