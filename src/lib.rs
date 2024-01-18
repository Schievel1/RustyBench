use anyhow::Result;
use std::path::{Path, PathBuf};

pub mod ui;

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

use prost::Message;
use std::fs::{File, self};
use std::io::Cursor;
use std::io::{BufReader, Read};
pub mod tonie {
    include!(concat!(env!("OUT_DIR"), "/tonie.rs"));
}
fn deserialize_header(len: usize, buf: &[u8]) -> Result<tonie::TonieHeader, prost::DecodeError> {
    if len + 4 > buf.len() {
        return Err(prost::DecodeError::new("header length is longer than buffer"));
    }

    tonie::TonieHeader::decode(&mut Cursor::new(buf[4..len + 4].to_vec()))
}
fn serialize_header(header: &tonie::TonieHeader) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.reserve(header.encoded_len());

    header.encode(&mut buf).unwrap();
    buf
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
    Ok(header)
}

pub fn populate_folder(path: &Path, files: &mut Vec<Teddyfile>) {
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
                            log::error!(
                                "error reading header from file {}",
                                entry.path().display()
                            );
                            log::error!("error: {}", e);
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

pub fn convert_from_ogg(path: &Path) {
    todo!("convert an ogg to teddybox file");
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
    fs::write(
        dest.join(dest).with_extension("ogg"),
        audio,
    ).unwrap();
}

pub fn change_tag_id(file: &Teddyfile) {
    todo!("change the tag id of a file");
}
