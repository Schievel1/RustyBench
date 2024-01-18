use anyhow::Result;
use sha1::{Digest, Sha1};
use std::env;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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
use std::fs::{self, DirEntry, File};
use std::io::{BufReader, Read};
use std::io::{BufWriter, Cursor, Write};
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
fn serialize_header(header: &tonie::TonieHeader) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.reserve(header.encoded_len());

    header.encode(&mut buf).unwrap();
    buf
}
pub fn create_tonie_header(audio: &[u8], pad_len: usize) -> tonie::TonieHeader {
    let mut hasher = Sha1::new();
    hasher.update(audio);
    let hash = hasher.finalize();
    let start = SystemTime::now();
    let timestamp = (start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs() as u32) - 0x1000;
    // debug
    // let timestamp = 0x15a45007;
    // let data_length = 0xb3198f;

    tonie::TonieHeader {
        data_hash: hash.to_vec(),
        data_length: audio.len() as u32,
        timestamp,
        chapter_pages: vec![0],
        padding: vec![0; pad_len],
    }
}
pub fn convert_from_ogg(dest: &Path, path: &Path, tag: &str) {
    if tag.len() != 16 {
        log::error!("error: tag must be 16 characters long");
        return;
    }

    // TODO this is some example code when we want to read different audio files and then convert them to opus
    // let f_i = File::open(path).unwrap();
    // let mut reader = BufReader::new(f_i);
    // let mut audio = vec![];
    // let _ = reader.read_to_end(&mut audio);
    // let (raw, _header) = ogg_opus::decode::<_,16000>(Cursor::new(audio)).unwrap();
    // let mut opus = ogg_opus::encode::<16000, 1>(&raw).unwrap();
    let f = File::open(path).unwrap();
    let mut reader = BufReader::new(f);
    let mut audio = vec![];
    let _ = reader.read_to_end(&mut audio);

    // HACK: to calculate the heading (the complete header must be 0xffc in size), we add a padding of 0x100
    let header = create_tonie_header(&audio, 0x100);
    // then serialize and take its length
    let tmp_header_len = serialize_header(&header).len();
    // then create the header again with the correct padding
    let header = create_tonie_header(&audio, 0xffc - tmp_header_len + 0x100);

    println!("{:x?}", &header);

    let (filename, dirname) = tag.split_at(8);
    // dbg!(&filename);
    // dbg!(&dirname);
    let dest = dest.join(rotate_bytewise(dirname));
    let _ = fs::create_dir(&dest);

    let mut outbuf = vec![0, 0, 0x0f, 0xfc];
    outbuf.append(&mut serialize_header(&header));
    outbuf.resize(0x1000, 0);
    outbuf.append(&mut audio);

    let outfile = File::create(dest.join(rotate_bytewise(filename))).unwrap();
    let mut writer = BufWriter::new(outfile);
    let _ = writer.write_all(&outbuf);
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
            log::error!("error reading header from file {}", entry.path().display());
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

pub fn change_tag_id(file: &Teddyfile) {
    todo!("change the tag id of a file");
}

pub fn add_note(file: &Teddyfile) {
    todo!("add a note to a file");
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
