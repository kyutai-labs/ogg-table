use anyhow::Result;
use ogg_table::OggVorbisReader;

fn main() -> Result<()> {
    for i in 0..5 {
        let start_time = std::time::Instant::now();
        let file = std::fs::File::open("foo.ogg")?;
        let rdr = std::io::BufReader::new(file);
        let mut ovr = OggVorbisReader::new(rdr)?;
        let data = ovr.decode(0, 1000000000000)?;
        println!("{i} {} {:?}", data[0].len(), start_time.elapsed());
    }
    Ok(())
}
