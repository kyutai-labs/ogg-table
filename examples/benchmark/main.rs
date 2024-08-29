use anyhow::Result;
use ogg_table::OggVorbisReader;

mod audio;

fn main() -> Result<()> {
    for i in 0..5 {
        let start_time = std::time::Instant::now();
        let file = std::fs::File::open("foo.ogg")?;
        let rdr = std::io::BufReader::new(file);
        let mut ovr = OggVorbisReader::new(rdr)?;
        let data = ovr.decode(0, 1000000000000)?;
        println!("{i} {} {:?}", data[0].len(), start_time.elapsed());
        if false {
            let mut out_file = std::fs::File::create("foo.wav")?;
            audio::write_wav(&mut out_file, &data[0], 24000)?;
        }
    }
    Ok(())
}
