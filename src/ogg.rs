use anyhow::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeaderTypeFlag {
    Continuation,
    Bos,
    Eos,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct HeaderType(u8);

impl HeaderType {
    pub fn has_flag(&self, flag: HeaderTypeFlag) -> bool {
        let flag = match flag {
            HeaderTypeFlag::Continuation => 0x01,
            HeaderTypeFlag::Bos => 0x02,
            HeaderTypeFlag::Eos => 0x04,
        };
        (flag & self.0) != 0
    }
}

impl std::fmt::Debug for HeaderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.has_flag(HeaderTypeFlag::Continuation) {
            write!(f, "Cont")?
        }
        if self.has_flag(HeaderTypeFlag::Bos) {
            write!(f, "Bos")?
        }
        if self.has_flag(HeaderTypeFlag::Eos) {
            write!(f, "Eos")?
        }
        if self.0 == 0 {
            write!(f, "None")?
        }
        Ok(())
    }
}

// https://en.wikipedia.org/wiki/Ogg#Page_structure
// https://xiph.org/ogg/doc/framing.html
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub header_type: HeaderType,
    pub granule_position: u64,
    pub bitstream_serial_number: u32,
    pub page_sequence_number: u32,
    pub segment_table: Vec<u8>,
}

impl Header {
    pub fn from_reader<R: std::io::Read>(rdr: &mut R) -> Result<Option<Self>> {
        use byteorder::{LittleEndian, ReadBytesExt};

        let mut capture_pattern = [0u8; 4];
        if rdr.read_exact(&mut capture_pattern).is_err() {
            return Ok(None);
        };
        if capture_pattern != *b"OggS" {
            anyhow::bail!("unexpected capture pattern {capture_pattern:?}")
        }
        let version = rdr.read_u8()?;
        if version != 0 {
            anyhow::bail!("unexpected version {version}")
        }
        let header_type = HeaderType(rdr.read_u8()?);
        let granule_position = rdr.read_u64::<LittleEndian>()?;
        let bitstream_serial_number = rdr.read_u32::<LittleEndian>()?;
        let page_sequence_number = rdr.read_u32::<LittleEndian>()?;
        let _checksum = rdr.read_u32::<LittleEndian>()?;
        let segments = rdr.read_u8()?;
        let mut segment_table = vec![0u8; segments as usize];
        rdr.read_exact(&mut segment_table)?;
        Ok(Some(Self {
            header_type,
            granule_position,
            bitstream_serial_number,
            page_sequence_number,
            segment_table,
        }))
    }
}

pub fn all_headers<R: std::io::Read + std::io::Seek>(rdr: &mut R) -> Result<Vec<(u64, Header)>> {
    rdr.seek(std::io::SeekFrom::Start(0))?;
    let mut headers = vec![];
    loop {
        let pos = rdr.stream_position()?;
        let header = match Header::from_reader(rdr)? {
            None => break,
            Some(header) => header,
        };
        let to_skip = header.segment_table.iter().map(|v| *v as i64).sum::<i64>();
        headers.push((pos, header));
        rdr.seek(std::io::SeekFrom::Current(to_skip))?;
    }
    Ok(headers)
}

pub struct PacketReader<R: std::io::Read> {
    // The reader is always positioned at the beginning of the data for `segment_idx`.
    reader: R,
    current_header: Header,
    segment_idx: usize,
}

impl<R: std::io::Read + std::io::Seek> PacketReader<R> {
    pub fn seek(&mut self, header_pos: u64, move_to_last_segment: bool) -> Result<u64> {
        self.reader.seek(std::io::SeekFrom::Start(header_pos))?;
        self.current_header = match Header::from_reader(&mut self.reader)? {
            None => anyhow::bail!("no data left"),
            Some(header) => header,
        };
        if self
            .current_header
            .header_type
            .has_flag(HeaderTypeFlag::Continuation)
        {
            anyhow::bail!("continuations are not handled properly when seeking")
        }
        if move_to_last_segment {
            // Skip to the last unfinished packet.
            let mut to_skip = 0usize;
            let mut n_to_skip = 0usize;
            let mut is_finished = false;
            for &l in self.current_header.segment_table.iter().rev() {
                if l != 255 {
                    is_finished = true
                }
                if is_finished {
                    to_skip += l as usize;
                    n_to_skip += 1;
                }
            }
            self.segment_idx = n_to_skip;
            self.reader
                .seek(std::io::SeekFrom::Current(to_skip as i64))?;
        }
        Ok(self.current_header.granule_position)
    }

    pub fn seek_granule_position(
        &mut self,
        target_granule_pos: u64,
        move_to_last_segment: bool,
    ) -> Result<u64> {
        self.reader.seek(std::io::SeekFrom::Start(0))?;
        let mut last_header_pos = 0;
        loop {
            let header_pos = self.reader.stream_position()?;
            let header = match Header::from_reader(&mut self.reader)? {
                None => break,
                Some(header) => header,
            };
            if header.granule_position >= target_granule_pos && header.granule_position > 0 {
                break;
            }
            last_header_pos = header_pos;
            let to_skip = header.segment_table.iter().map(|v| *v as i64).sum::<i64>();
            self.reader.seek(std::io::SeekFrom::Current(to_skip))?;
        }
        self.seek(last_header_pos, move_to_last_segment)
    }
}

impl<R: std::io::Read> PacketReader<R> {
    pub fn new(mut reader: R) -> Result<Self> {
        let current_header = match Header::from_reader(&mut reader)? {
            None => anyhow::bail!("empty file"),
            Some(header) => header,
        };
        Ok(Self {
            reader,
            current_header,
            segment_idx: 0,
        })
    }

    pub fn next_packet(&mut self) -> Result<Option<Vec<u8>>> {
        let mut packet_data = Vec::new();
        loop {
            if self.segment_idx < self.current_header.segment_table.len() {
                let len = self.current_header.segment_table[self.segment_idx];
                self.segment_idx += 1;
                if len != 0 {
                    let mut data = vec![0u8; len as usize];
                    self.reader.read_exact(&mut data)?;
                    packet_data.push(data);
                }
                if len != 255 {
                    let packet_data = packet_data.concat();
                    return Ok(Some(packet_data));
                }
            } else {
                match Header::from_reader(&mut self.reader)? {
                    None => {
                        if packet_data.is_empty() {
                            return Ok(None);
                        }
                        let packet_data = packet_data.concat();
                        return Ok(Some(packet_data));
                    }
                    Some(header) => {
                        self.segment_idx = 0;
                        self.current_header = header
                    }
                }
            }
        }
    }

    pub fn into_inner(self) -> R {
        self.reader
    }
}
