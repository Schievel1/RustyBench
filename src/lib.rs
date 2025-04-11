use anyhow::anyhow;
use anyhow::Result;
use crossbeam::channel::Sender;
use log::{error, info};
use std::env;
use std::fs::{self, DirEntry, File};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use toniefile::Toniefile;
use tonielist::Tonie;
use ui::Action;

#[macro_use]
extern crate lazy_static;

use crate::resampler::Resampler;
use crate::buffered_source::BufferedSource;

pub mod buffered_source;
pub mod resampler;
pub mod tonielist;
pub mod ui;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Teddyfile {
    path: PathBuf,
    is_valid: bool,
    hash: Vec<u8>,
    length: u64,
    audio_id: u32,
    chapter_pages: Vec<u32>,
    tag: String,
    info: Option<Tonie>,
}

#[allow(clippy::too_many_arguments)]
impl Teddyfile {
    pub fn new(
        path: PathBuf,
        is_valid: bool,
        hash: Vec<u8>,
        length: u64,
        audio_id: u32,
        chapter_pages: Vec<u32>,
        tag: String,
        info: Option<Tonie>,
    ) -> Self {
        Self {
            path,
            is_valid,
            hash,
            length,
            audio_id,
            chapter_pages,
            tag,
            info,
        }
    }
}

fn decode_encode(
    src: &Path,
    toniefile: &mut Toniefile<File>,
    write_tx: Sender<Action>,
) -> Result<()> {
    info!("Encoding input file: {}", src.display());
    let start_time = std::time::Instant::now();
    // if the input file has an extension, use it as a hint for the media format.
    let mut hint = Hint::new();
    if let Some(ext) = src.extension() {
        if let Some(ext) = ext.to_str() {
            hint.with_extension(ext);
        }
    }

    let src = std::fs::File::open(src)?;

    let bufsrc = Box::new(BufferedSource::new(src, 1024 * 1024 * 64));
    // Create the media source stream.
    let mss = MediaSourceStream::new(bufsrc, Default::default());

    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();

    // Probe the media source.
    let probed = symphonia::default::get_probe().format(&hint, mss, &fmt_opts, &meta_opts)?;

    // Get the instantiated format reader.
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or(anyhow::anyhow!("No supported audio track found (audio format is not supported by symphonia library)"))?;

    // Create a decoder for the track.
    let dec_opts: DecoderOptions = Default::default();
    let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &dec_opts)?;

    // Store the track identifier, it will be used to filter packets.
    let track_id = track.id;

    // create a resampler to convert to 48kHz
    let mut resampler: Option<Resampler<i16>> = None;

    let input_sample_rate = track.codec_params.sample_rate.unwrap_or_default();
    let input_channels = track.codec_params.channels.unwrap_or_default().count(); //TODO
    let tracklen = track.codec_params.n_frames.unwrap_or_default();

    // print some file info
    info!(
        "Input file: {} Hz, {} channels",
        input_sample_rate, input_channels,
    );

    info!("Track length: {} frames", tracklen);

    let mut progress = 0;
    while let Ok(packet) = format.next_packet() {
        let newprogress = packet.ts * 100 / tracklen;
        if packet.ts * 100 / tracklen != progress {
            progress = newprogress;
            info!("Progress: {}%", progress);
            write_tx.send(Action::Processing(progress))?;
        }

        // Consume any new metadata that has been read since the last packet.
        while !format.metadata().is_latest() {
            format.metadata().pop();
        }

        // If the packet does not belong to the selected track, skip over it.
        if packet.track_id() != track_id {
            continue;
        }

        // Decode the packet into audio samples.
        match decoder.decode(&packet) {
            Ok(decoded) => {
                // The packet was successfully decoded, process the audio samples.
                if resampler.is_none() {
                    resampler = Some(Resampler::new(
                        *decoded.spec(),
                        48000,
                        decoded.capacity() as u64 // / 2,
                    ));
                }
                if let Some(res) = resampler.as_mut() {
                    if let Some(resampled) = res.resample(decoded) {
                        toniefile.encode(resampled)?;
                    }
                }
            }
            Err(SymphoniaError::IoError(_)) => {
                // The packet failed to decode due to an IO error, skip the packet.
                info!("IO error");
                continue;
            }
            Err(SymphoniaError::DecodeError(_)) => {
                // The packet failed to decode due to invalid data, skip the packet.
                info!("Decode error");
                continue;
            }
            Err(err) => {
                // An unrecoverable error occured, halt decoding.
                return Err(err.into());
            }
        }
    }
    info!("Progress: 100%");
    info!("File done");
    info!("Time to decode: {} seconds", std::time::Instant::now().duration_since(start_time).as_secs());
    Ok(())
}

fn write_table_entry(
    entry: DirEntry,
    files: &mut Vec<Teddyfile>,
    tonielist: &Arc<Vec<Tonie>>,
) -> Result<()> {
    let mut f = File::open(entry.path())?;
    match Toniefile::parse_header(&mut f) {
        Ok(header) => {
            let info = tonielist::find_tonie_with_audio_id(&tonielist, header.audio_id);
            files.push(Teddyfile::new(
                entry.path(),
                true,
                header.sha1_hash,
                header.num_bytes,
                header.audio_id,
                header.track_page_nums,
                get_tag_id(&entry.path()).unwrap_or("invalid".into()),
                info,
            ));
        }
        Err(e) => {
            error!("error reading header from file {}", entry.path().display());
            error!("error: {}", e);
            files.push(Teddyfile::new(
                entry.path(),
                false,
                vec![],
                0,
                0,
                vec![],
                get_tag_id(&entry.path()).unwrap_or("invalid".into()),
                None,
            ));
        }
    }
    Ok(())
}

fn rotate_bytewise(input: &str) -> String {
    if input.len() != 8 {
        log::debug!("input {} is not 8 bytes long", input);
        return "INVALID".into();
    }
    format!(
        "{}{}{}{}{}{}{}{}",
        &input[6..=6],
        &input[7..=7],
        &input[4..=4],
        &input[5..=5],
        &input[2..=2],
        &input[3..=3],
        &input[0..=0],
        &input[1..=1]
    )
}

fn get_tag_id(path: &Path) -> Option<String> {
    let mut ancestors = path.ancestors();
    let firsthalf = ancestors.next()?.file_name()?.to_str()?;
    let secondhalf = ancestors.next()?.file_name()?.to_str()?;
    let firsthalf = rotate_bytewise(firsthalf);
    let secondhalf = rotate_bytewise(secondhalf);
    Some(format!("{}{}", firsthalf, secondhalf))
}

fn read_header_len(buf: &[u8]) -> Result<usize> {
    if buf.len() < 4 {
        return Err(anyhow!("file too short"));
    }
    Ok(buf[3] as usize | (buf[2] as usize) << 8)
}

fn read_audio_from_file(file: &Teddyfile) -> Result<Vec<u8>> {
    let f = File::open(&file.path)?;
    let mut reader = BufReader::new(f);
    let mut buf: Vec<u8> = Vec::new();

    reader.read_to_end(&mut buf)?;
    let header_len = read_header_len(&buf)?;

    Ok(buf[header_len + 4..].to_vec())
}

pub fn extract_to_ogg(file: &Teddyfile, dest: &Path, write_tx: Sender<Action>) -> Result<()> {
    if !file.is_valid {
        error!("file {} has an invalid header", file.path.display());
    }
    write_tx.send(Action::CurrentFile(file.path.to_string_lossy().to_string()))?;
    let audio = read_audio_from_file(file)?;
    fs::write(dest.join(dest).with_extension("ogg"), audio)?;
    write_tx.send(Action::CurrentFile("".to_string()))?;
    Ok(())
}

pub fn change_tag_id(picked_path: &Path, file: &Teddyfile, tag: &str) -> Result<()> {
    let (filename, dirname) = tag.split_at(8);
    let (filename, dirname) = (
        filename.to_string().to_ascii_uppercase(),
        dirname.to_string().to_ascii_uppercase(),
    );
    let dest = picked_path.join(rotate_bytewise(&dirname));
    fs::create_dir(&dest)?;
    fs::copy(&file.path, dest.join(rotate_bytewise(&filename)))?;
    fs::remove_file(&file.path)?;
    if let Some(parent) = file.path.parent() {
        fs::remove_dir(parent)?;
    }
    Ok(())
}

pub fn delete_file(file: &Teddyfile) -> Result<()> {
    fs::remove_file(&file.path)?;
    if let Some(parent) = file.path.parent() {
        fs::remove_dir(parent)?;
    }
    Ok(())
}

pub fn extract_all(files: &[Teddyfile], path: &Path, write_tx: Sender<Action>) -> Result<()> {
    for file in files {
        extract_to_ogg(file, &path.join(&file.tag), write_tx.clone()).unwrap_or_else(|e| {
            error!("error extracting file {}: {}", file.path.display(), e);
        });
    }
    Ok(())
}

pub fn play_file(file: &Teddyfile, write_tx: Sender<Action>) -> Result<()> {
    let dir = env::temp_dir();
    let path = dir
        .join(file.path.file_name().unwrap_or_default())
        .with_extension("ogg");
    extract_to_ogg(file, &path, write_tx)?;
    open::that(path)?;
    Ok(())
}

pub fn check_tag_id_validity(tag_id: &str) -> Result<()> {
    if tag_id.len() != 16 {
        return Err(anyhow!("tag ID must be 16 characters long"));
    }
    // tag id must be hex
    if tag_id.chars().any(|c| !c.is_ascii_hexdigit()) {
        return Err(anyhow!("tag ID must be hexadecimal"));
    }
    Ok(())
}

pub fn populate_table(
    path: &Path,
    files: &mut Vec<Teddyfile>,
    tonielist: &Arc<Vec<Tonie>>,
) -> Result<()> {
    for entry in path.read_dir()?.flatten() {
        if entry.path().is_dir() {
            if let Some(filename) = entry.path().file_name() {
                if !filename.to_string_lossy().starts_with("000000") {
                    for entry in entry.path().read_dir()?.flatten() {
                        if entry.path().is_file() {
                            write_table_entry(entry, files, &tonielist)?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn add_audio_file(
    dest: PathBuf,
    infiles: Vec<PathBuf>,
    tag: String,
    write_tx: Sender<Action>,
) -> Result<()> {
    let (filename, dirname) = tag.split_at(8);
    let (filename, dirname) = (
        filename.to_string().to_ascii_uppercase(),
        dirname.to_string().to_ascii_uppercase(),
    );
    let dest = dest.join(rotate_bytewise(&dirname));
    let _ = fs::create_dir(&dest);
    let destfile = File::create(dest.join(rotate_bytewise(&filename)))?;

    let mut toniefile = Toniefile::new_simple(destfile)?;

    let mut infiles_iter = infiles.iter();
    let first_path = infiles_iter.next().ok_or(anyhow!("no input files"))?;
    let mut i = 1;
    write_tx.send(Action::CurrentFileNo(i))?;
    decode_encode(first_path, &mut toniefile, write_tx.clone())?;
    for file in infiles_iter {
        i += 1;
        write_tx.send(Action::CurrentFileNo(i))?;
        toniefile.new_chapter()?;
        decode_encode(file, &mut toniefile, write_tx.clone())?;
    }
    info!("all files encoded, finalizing...");
    toniefile.finalize()?;
    write_tx.send(Action::PopulateTable)?;
    Ok(())
}
