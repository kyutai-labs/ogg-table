use anyhow::Result;
use ogg_table::OggVorbisReader;

fn main() -> Result<()> {
    let ogg_filename = "foo.ogg";
    let start_time_sec = 314.15;
    let duration_sec = 60.;
    let (all_data, sample_rate) = {
        let file = std::fs::File::open(ogg_filename)?;
        let rdr = std::io::BufReader::new(file);
        let mut ovr = OggVorbisReader::new(rdr)?;
        let data = ovr.decode(0, 1000000000000)?;
        (data, ovr.sample_rate())
    };
    let (selected_data, _sr) =
        ogg_table::read_ogg_vorbis_sample(ogg_filename, start_time_sec, duration_sec)?;
    let mut out_file = std::fs::File::create("foo1.wav")?;
    ogg_table::wav::write_wav(&mut out_file, &selected_data[0], sample_rate)?;

    let start_in_samples = (start_time_sec * sample_rate as f64) as usize;
    let len_in_samples = (duration_sec * sample_rate as f64) as usize;
    let data = &all_data[0][start_in_samples..start_in_samples + len_in_samples];
    let mut out_file = std::fs::File::create("foo2.wav")?;
    ogg_table::wav::write_wav(&mut out_file, &data, sample_rate)?;
    Ok(())
}
