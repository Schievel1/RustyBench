use std::path::{PathBuf, Path};

pub mod ui;

pub struct Teddyfile {
    path: PathBuf,
    tag: String,
}

impl Teddyfile {
    pub fn new(path: PathBuf, tag: String) -> Self {
        Self {
            path,
            tag,
        }
    }
}

pub fn populate_folder(path: &Path, files: &mut Vec<Teddyfile>) {
    let file = Teddyfile::new("bblabla".into(), "E0ABCDE".into());
    files.push(file);
    let file = Teddyfile::new("blubblubb".into(), "E0FGHIJ".into());
    files.push(file);
    // TODO wander through all subfolders in path to get the files
    // TODO use get_tag_id to get the ids
}

pub fn get_tag_id(path: &Path) {
    todo!("Get the tag id using the path");
}

pub fn convert_from_ogg(path: &Path) {
    todo!("convert an ogg to teddybox file");
}

pub fn extract_to_ogg(file: &Teddyfile) {
    todo!("convert a teddybox file to ogg");
}

pub fn change_tag_id(file: &Teddyfile) {
    todo!("change the tag id of a file");
}
