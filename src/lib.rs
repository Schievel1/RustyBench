use anyhow::Result;
use log::{error, info};
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use toniefile::Toniefile;
use std::env;
use symphonia::core::errors::Error;
use std::path::{Path, PathBuf};
use prost::Message;
use std::fs::{self, DirEntry, File};
use std::io::{BufReader, Read};
use std::io::Cursor;
use anyhow::anyhow;

use crate::resampler::Resampler;

pub mod ui;
pub mod resampler;

pub struct Teddyfile {
    path: PathBuf,
    is_valid: bool,
    hash: Vec<u8>,
    length: u32,
    timestamp: u32,
    chapter_pages: Vec<u32>,
    tag: String,
}

impl Teddyfile {
    pub fn new(
        path: PathBuf,
        is_valid: bool,
        hash: Vec<u8>,
        length: u32,
        timestamp: u32,
        chapter_pages: Vec<u32>,
        tag: String,
    ) -> Self {
        Self {
            path,
            is_valid,
            hash,
            length,
            timestamp,
            chapter_pages,
            tag,
        }
    }
}

pub mod tonie {
    include!(concat!(env!("OUT_DIR"), "/tonie.rs"));
}

fn deserialize_header(len: usize, buf: &[u8]) -> Result<tonie::TonieHeader, prost::DecodeError> {
    if len + 4 > buf.len() {
        return Err(prost::DecodeError::new(
            "header length is longer than buffer",
        ));
    }

    tonie::TonieHeader::decode(&mut Cursor::new(buf[4..len + 4].to_vec()))
}

pub fn add_audio_file(dest: &Path, path: &Path, tag: &str) {
    let (filename, dirname) = tag.split_at(8);
    dbg!(&filename);
    dbg!(&dirname);
    let (filename, dirname) = (filename.to_string().to_ascii_uppercase(), dirname.to_string().to_ascii_uppercase());
    let dest = dest.join(rotate_bytewise(&dirname));
    let _ = fs::create_dir(&dest);
    let destfile = File::create(dest.join(rotate_bytewise(&filename))).unwrap();

    let src = File::open(path).unwrap();
    // Create the media source stream.
    let mss = MediaSourceStream::new(Box::new(src), Default::default());

    // if the input file has an extension, use it as a hint for the media format.
    let mut hint = Hint::new();
    if let Some(ext) = path.extension() {
        if let Some(ext) = ext.to_str() {
            hint.with_extension(ext);
        }
    }

    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();
    // Probe the media source.
    let probed = symphonia::default::get_probe().format(&hint, mss, &fmt_opts, &meta_opts).unwrap();

    // Get the instantiated format reader.
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .unwrap();
        // .ok_or(anyhow::anyhow!("No supported audio track found"));

    // Create a decoder for the track.
    let dec_opts: DecoderOptions = Default::default();
    let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &dec_opts).unwrap();

    // Store the track identifier, it will be used to filter packets.
    let track_id = track.id;

    // create a Toniefile to write to later
    let mut toniefile = Toniefile::new_simple(destfile).unwrap();

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

    while let Ok(packet) = format.next_packet() {
        let progress = packet.ts * 100 / tracklen;
        info!("Progress: {}%", progress);
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
                        decoded.capacity() as u64 / 2,
                    ));
                }
                if let Some(resampled) = resampler.as_mut().unwrap().resample(decoded.clone()) {
                    toniefile.encode(resampled).unwrap();
                }
            }
            Err(Error::IoError(_)) => {
                // The packet failed to decode due to an IO error, skip the packet.
                error!("IO error");
                continue;
            }
            Err(Error::DecodeError(_)) => {
                // The packet failed to decode due to invalid data, skip the packet.
                error!("Decode error");
                continue;
            }
            Err(err) => {
                // An unrecoverable error occured, halt decoding.
                panic!("{}", err);
            }
        }
    }
    toniefile.finalize().unwrap();
    info!("File done");
}

fn read_header_len(buf: &[u8]) -> usize {
    buf[3] as usize | (buf[2] as usize) << 8
}

fn read_header_from_file(path: &Path) -> Result<tonie::TonieHeader> {
    let f = File::open(path)?;
    let mut reader = BufReader::with_capacity(4096, f);
    let mut buf = [0u8; 4096];

    reader.read_exact(&mut buf)?;
    let header_len = read_header_len(&buf);
    let header = deserialize_header(header_len, &buf)?;
    // if path.parent().unwrap().ends_with("A33D9C12") {
        // println!("{}", path.display());
        // println!("{:x?}", &header);
    // }
    Ok(header)
}

fn write_table_entry(entry: DirEntry, files: &mut Vec<Teddyfile>) {
    match read_header_from_file(&entry.path()) {
        Ok(header) => {
            files.push(Teddyfile::new(
                entry.path(),
                true,
                header.data_hash,
                header.data_length,
                header.timestamp,
                header.chapter_pages,
                get_tag_id(&entry.path()),
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
                get_tag_id(&entry.path()),
            ));
        }
    }
}

pub fn populate_table(path: &Path, files: &mut Vec<Teddyfile>) {
    for entry in path.read_dir().unwrap().flatten() {
        if entry.path().is_dir()
            && !entry
                .path()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("000000")
        {
            for entry in entry.path().read_dir().unwrap().flatten() {
                if entry.path().is_file() {
                    write_table_entry(entry, files);
                }
            }
        }
    }
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

pub fn get_tag_id(path: &Path) -> String {
    let mut ancestors = path.ancestors();
    let firsthalf = ancestors
        .next()
        .unwrap()
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();
    let secondhalf = ancestors
        .next()
        .unwrap()
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();
    let firsthalf = rotate_bytewise(firsthalf);
    let secondhalf = rotate_bytewise(secondhalf);
    format!("{}{}", firsthalf, secondhalf)
}

fn read_audio_from_file(file: &Teddyfile) -> Result<Vec<u8>> {
    let f = File::open(&file.path)?;
    let mut reader = BufReader::new(f);
    let mut buf: Vec<u8> = Vec::new();

    reader.read_to_end(&mut buf)?;
    let header_len = read_header_len(&buf);

    Ok(buf[header_len + 4..].to_vec())
}

pub fn extract_to_ogg(file: &Teddyfile, dest: &Path) {
    if !file.is_valid {
        log::error!("file {} has an invalid header", file.path.display());
    }
    let audio = read_audio_from_file(file).unwrap();
    fs::write(dest.join(dest).with_extension("ogg"), audio).unwrap();
}

pub fn change_tag_id(picked_path: &Path, file: &Teddyfile, tag: &str) {
    let (filename, dirname) = tag.split_at(8);
    dbg!(&filename);
    dbg!(&dirname);
    let (filename, dirname) = (filename.to_string().to_ascii_uppercase(), dirname.to_string().to_ascii_uppercase());
    let dest = picked_path.join(rotate_bytewise(&dirname));
    let _ = fs::create_dir(&dest);
    let _ = fs::copy(&file.path, dest.join(rotate_bytewise(&filename)));
    let _ = fs::remove_file(&file.path);
    let _ = fs::remove_dir(file.path.parent().unwrap());
}

pub fn extract_all(files: &[Teddyfile], path: &Path) {
    for file in files {
        dbg!(&file.path);
        extract_to_ogg(file, &path.join(&file.tag));
    }
}

pub fn play_file(file: &Teddyfile) {
    let dir = env::temp_dir();
    let path = dir
        .join(file.path.file_name().unwrap())
        .with_extension("ogg");
    extract_to_ogg(file, &path);
    open::that(path).unwrap();
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
