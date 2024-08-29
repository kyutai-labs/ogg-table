use anyhow::Result;
use ogg_table::OggVorbisReader;

fn main() -> Result<()> {
    let ogg_filename = "foo.ogg";
    let (all_data, sample_rate) = {
        let file = std::fs::File::open(ogg_filename)?;
        let rdr = std::io::BufReader::new(file);
        let mut ovr = OggVorbisReader::new(rdr)?;
        let data = ovr.decode(0, 1000000000000)?;
        (data, ovr.sample_rate())
    };

    for with_ogg_table in [false, true] {
        let ogg_table = std::path::Path::new(ogg_filename).with_extension("ogg_table");
        if with_ogg_table {
            let file = std::fs::File::open(ogg_filename)?;
            let mut rdr = std::io::BufReader::new(file);
            let table = ogg_table::TableOfContent::from_ogg_reader(&mut rdr)?;
            let mut ogg_table = std::fs::File::create(ogg_table)?;
            table.write(&mut ogg_table)?
        } else {
            let _maybe_err = std::fs::remove_file(ogg_table);
        }
        for start_time_sec in [314.15, 13.37, 13.36, 999.9, 3999.9] {
            let duration_sec = 60.;
            let start_instant = std::time::Instant::now();
            let (selected_data, _sr) =
                ogg_table::read_ogg_vorbis_sample(ogg_filename, start_time_sec, duration_sec)?;
            let dt = start_instant.elapsed();
            println!("start-time: {start_time_sec} {dt:?}");
            let mut out_file = std::fs::File::create("foo1.wav")?;
            ogg_table::wav::write_wav(&mut out_file, &selected_data[0], sample_rate)?;

            let start_in_samples = (start_time_sec * sample_rate as f64) as usize;
            let len_in_samples = (duration_sec * sample_rate as f64) as usize;
            let data = &all_data[0][start_in_samples..start_in_samples + len_in_samples];
            let mut out_file = std::fs::File::create("foo2.wav")?;
            ogg_table::wav::write_wav(&mut out_file, data, sample_rate)?;

            for i in 0..1000 {
                if selected_data[0][i..i + 100] == data[..100] {
                    println!("offset: {i}")
                }
                if selected_data[0][..100] == data[i..i + 100] {
                    println!("offset -{i}")
                }
            }
        }
    }
    Ok(())
}
