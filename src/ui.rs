use anyhow::Error;
use anyhow::Result;
use crossbeam::channel::{Receiver, Sender};
use eframe::{
    egui::{self, RichText},
    epaint::Color32,
};
use egui::FontFamily::{self, Proportional};
use egui::FontId;
use egui::TextStyle::*;
use egui_extras::{Column, TableBuilder};
use log::{error, info};
use std::{ffi::OsStr, thread};
use std::{path::PathBuf, sync::Arc};

use crate::tonielist::get_tonie_list_from_file;
use crate::tonielist::get_tonie_list_online;
use crate::{
    add_audio_file, change_tag_id, check_tag_id_validity, delete_file, extract_all, extract_to_ogg,
    play_file, populate_table, tonielist::Tonie, Teddyfile,
};

#[derive(Debug, Clone)]
pub enum Action {
    None,
    AskAddAudioFile,
    AddAudioFile,
    AskChangeTagId,
    ChangeTagId,
    PopulateTable,
    ExtractToOgg,
    ExtractAll,
    PlayFile,
    DeleteFile,
    ShowFileData,
    Processing(u64),
    CurrentFileNo(usize),
    CurrentFile(String),
}

pub struct RustyBench {
    pub picked_path: PathBuf,
    pub picked_file: PathBuf,
    pub picked_files: Vec<PathBuf>,
    pub files: Vec<Teddyfile>,
    pub selection: Option<usize>,
    pub show_id_popup: bool,
    pub tag_id: String,
    pub tag_id_valid: bool,
    pub error: Option<Error>,
    pub action: Action,
    pub thread_receiver: Receiver<Action>,
    pub thread_sender: Sender<Action>,
    pub processed: u64,
    pub current_fileno: usize,
    pub current_file: String,
    pub joinhandles: Vec<thread::JoinHandle<Result<(), Error>>>,
    pub tonies: Arc<Vec<Tonie>>,
}

impl Default for RustyBench {
    fn default() -> Self {
        let (thread_sender, thread_receiver) = crossbeam::channel::unbounded::<Action>();
        let tonies = if let Ok(list) = get_tonie_list_online(None) {
            Arc::new(list)
        } else {
            Arc::new(vec![])
        };
        Self {
            picked_path: Default::default(),
            picked_file: Default::default(),
            picked_files: vec![],
            files: vec![],
            selection: None,
            show_id_popup: false,
            tag_id: "E0040350".to_string(),
            tag_id_valid: false,
            error: None,
            action: Action::None,
            thread_receiver,
            thread_sender,
            processed: 0,
            current_fileno: 0,
            current_file: "".to_string(),
            joinhandles: vec![],
            tonies,
        }
    }
}

impl RustyBench {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        RustyBench::setup_app(&cc.egui_ctx);
        Self {
            ..Default::default()
        }
    }
    fn setup_app(ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        style.text_styles = [
            (Heading, FontId::new(20.0, Proportional)),
            (Name("Heading2".into()), FontId::new(25.0, Proportional)),
            (Name("Context".into()), FontId::new(23.0, Proportional)),
            (Body, FontId::new(14.0, Proportional)),
            (Monospace, FontId::new(14.0, FontFamily::Monospace)),
            (Button, FontId::new(14.0, Proportional)),
            (Small, FontId::new(10.0, Proportional)),
        ]
        .into();
        ctx.set_style(style);
    }
    fn toggle_row_selection(&mut self, row_index: usize, row_response: &egui::Response) {
        if row_response.clicked() {
            self.selection = Some(row_index);
        }
        if row_response.double_clicked() {
            self.selection = Some(row_index);
            self.action = Action::ShowFileData;
        }
    }
    fn format_tag_id(&self, tag_id: &str) -> String {
        if tag_id.len() == 16 {
            let mut tag_id = tag_id.chars();
            format!(
                "{}{}:{}{}:{}{}:{}{}:{}{}:{}{}:{}{}:{}{}",
                tag_id.next().unwrap_or_default(),
                tag_id.next().unwrap_or_default(),
                tag_id.next().unwrap_or_default(),
                tag_id.next().unwrap_or_default(),
                tag_id.next().unwrap_or_default(),
                tag_id.next().unwrap_or_default(),
                tag_id.next().unwrap_or_default(),
                tag_id.next().unwrap_or_default(),
                tag_id.next().unwrap_or_default(),
                tag_id.next().unwrap_or_default(),
                tag_id.next().unwrap_or_default(),
                tag_id.next().unwrap_or_default(),
                tag_id.next().unwrap_or_default(),
                tag_id.next().unwrap_or_default(),
                tag_id.next().unwrap_or_default(),
                tag_id.next().unwrap_or_default()
            )
        } else {
            "invalid legnth".to_string()
        }
    }
}

impl eframe::App for RustyBench {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.set_enabled(!self.show_id_popup);
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    if ui.button("Choose CONTENT folder...").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                            self.picked_path = path;
                            self.action = Action::PopulateTable;
                        }
                    }
                    if ui.button("Load toniesV2.json file").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_file() {
                            let tonielist = match get_tonie_list_from_file(path) {
                                Ok(t) => Arc::new(t),
                                Err(e) => {
                                    self.error = Some(e);
                                    Arc::new(vec![])
                                }
                            };
                            self.tonies = tonielist;
                            self.action = Action::PopulateTable;
                        }
                    }
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.set_enabled(!self.show_id_popup);
            ui.horizontal(|ui| {
                ui.label("Folder: ");
                ui.add(
                    egui::Label::new(RichText::new(self.picked_path.to_string_lossy()).monospace())
                        .wrap(true),
                );
            });
            ui.group(|ui| {
                ui.vertical_centered_justified(|ui| {
                    ui.label("Files");
                });
                TableBuilder::new(ui)
                    .column(Column::auto().at_least(140.0).resizable(true))
                    .column(Column::auto().at_least(190.0).resizable(true))
                    .column(Column::remainder())
                    .sense(egui::Sense::click())
                    .striped(true)
                    .header(20.0, |mut header| {
                        header.col(|ui| {
                            ui.heading("Filename");
                        });
                        header.col(|ui| {
                            ui.heading("Tag ID");
                        });
                        header.col(|ui| {
                            ui.heading("Info");
                        });
                    })
                    .body(|body| {
                        body.rows(30.0, self.files.len(), |mut row| {
                            let row_index = row.index();
                            row.set_selected(self.selection == Some(row_index));
                            // column Filename
                            row.col(|ui| {
                                let mut parent = OsStr::new("");
                                if let Some(p) = self.files[row_index].path.parent() {
                                    if let Some(f) = p.file_name() {
                                        parent = f;
                                    }
                                }
                                let mut file = OsStr::new("");
                                if let Some(f) = self.files[row_index].path.file_name() {
                                    file = f;
                                }
                                let parent_and_file = String::from(parent.to_string_lossy())
                                    + "/"
                                    + &file.to_string_lossy();
                                ui.label(RichText::new(parent_and_file).monospace());
                            });
                            // column Tag ID
                            row.col(|ui| {
                                ui.label(
                                    egui::RichText::new(
                                        self.format_tag_id(&self.files[row_index].tag),
                                    )
                                    .monospace(),
                                );
                            });
                            // column Info
                            row.col(|ui| {
                                if let Some(t) = self.files[row_index].info.as_ref() {
                                    ui.label(format!(
                                        "{} - {}",
                                        &t.data[0].series.clone().unwrap_or_default(),
                                        &t.data[0].episode.clone().unwrap_or_default()
                                    ));
                                } else {
                                    ui.label("unknown");
                                }
                            });
                            self.toggle_row_selection(row_index, &row.response());
                        });
                    });
            });
        });

        egui::TopBottomPanel::bottom("Bottom Panel").show(ctx, |ui| {
            ui.set_enabled(!self.show_id_popup);
            ui.horizontal_centered(|ui| {
                if ui
                    .add_sized(
                        [120., 40.],
                        egui::Button::new("Choose folder\nCONTENT").fill(Color32::BLUE),
                    )
                    .clicked()
                {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.picked_path = path;
                    }
                    self.action = Action::PopulateTable;
                }
                if ui
                    .add_sized(
                        [120., 40.],
                        egui::Button::new("Add audio file").fill(Color32::BLUE),
                    )
                    .clicked()
                    && self.picked_path.exists()
                {
                    self.action = Action::AskAddAudioFile;
                }
                if ui
                    .add_sized(
                        [120., 40.],
                        egui::Button::new(egui::RichText::new("Delete file").color(Color32::BLACK))
                            .fill(Color32::RED),
                    )
                    .clicked()
                    && self.selection.is_some()
                {
                    self.action = Action::DeleteFile;
                }
                if ui
                    .add_sized(
                        [120., 40.],
                        egui::Button::new(egui::RichText::new("Play file").color(Color32::BLACK))
                            .fill(Color32::GREEN),
                    )
                    .clicked()
                    && self.selection.is_some()
                {
                    self.action = Action::PlayFile;
                }
                if ui
                    .add_sized(
                        [120., 40.],
                        egui::Button::new("Extract to .ogg").fill(Color32::BLUE),
                    )
                    .clicked()
                    && self.selection.is_some()
                {
                    self.action = Action::ExtractToOgg;
                }
                if ui
                    .add_sized(
                        [120., 40.],
                        egui::Button::new("Extract all\nto .ogg").fill(Color32::BLUE),
                    )
                    .clicked()
                {
                    self.action = Action::ExtractAll;
                }
                if ui
                    .add_sized(
                        [120., 40.],
                        egui::Button::new("Change tag ID").fill(Color32::BLUE),
                    )
                    .clicked()
                    && self.selection.is_some()
                {
                    self.action = Action::AskChangeTagId;
                }
            });
        });

        egui::TopBottomPanel::bottom("Messages Panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if self.processed > 0 {
                    ui.label(format!(
                        "File {} / {} ",
                        self.current_fileno,
                        self.picked_files.len()
                    ));
                    ui.label(format!("processed: {}%", self.processed));
                }
                if !self.current_file.is_empty() {
                    ui.label(format!("Extracting file: {}", self.current_file));
                }
            });
        });

        if self.show_id_popup {
            egui::Window::new("Please provide a tag ID")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("tag ID: ");
                    {
                        let text_edit = egui::TextEdit::singleline(&mut self.tag_id)
                            .char_limit(16)
                            .cursor_at_end(true)
                            .font(FontId::default());
                        let _output = text_edit.show(ui);
                    }
                    ui.horizontal(|ui| match check_tag_id_validity(&self.tag_id) {
                        Ok(_) => {
                            ui.label("Tag ID is valid");
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::BOTTOM),
                                |ui| {
                                    if ui.button("Cancel").clicked() {
                                        self.show_id_popup = false;
                                        self.tag_id = "E00403500".to_string();
                                        self.action = Action::None;
                                    }
                                    let ok_button = ui.button("Ok");
                                    ok_button.request_focus();
                                    if ok_button.clicked() {
                                        self.show_id_popup = false;
                                        self.tag_id_valid = true;
                                    }
                                },
                            );
                        }
                        Err(e) => {
                            ui.label(e.to_string());
                            self.tag_id_valid = false;
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::BOTTOM),
                                |ui| {
                                    if ui.button("Cancel").clicked() {
                                        self.show_id_popup = false;
                                        self.tag_id = "E00403500".to_string();
                                        self.action = Action::None;
                                    }
                                    ui.add_enabled(
                                        false,
                                        egui::Button::new("Ok").fill(Color32::GRAY),
                                    );
                                },
                            );
                        }
                    });
                });
        }

        if self.error.is_some() {
            egui::Window::new("Error")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Error: ");
                    if let Some(error) = &self.error {
                        ui.label(error.to_string());
                        error!("{}", error);
                    } else {
                        ui.label("unknown error");
                        error!("unknown error");
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Ok").clicked() {
                            self.error = None;
                        }
                    });
                });
        }
        // NOTE unwrapping self.selection is ok below here, because the buttons are disabled if
        // self.selection is None
        let thr = thread::Builder::new().name("action_thread".to_string());
        // let mut jh = None;
        match self.action {
            Action::None => {}
            Action::AskAddAudioFile => {
                self.action = Action::None;
                if let Some(files) = rfd::FileDialog::new().pick_files() {
                    self.picked_files = files;
                    self.show_id_popup = true;
                    self.action = Action::AddAudioFile;
                }
            }
            Action::AddAudioFile => {
                if self.tag_id_valid {
                    self.action = Action::None;
                    info!("adding audio file");
                    self.tag_id_valid = false;
                    let tag = self.tag_id.clone();
                    let path = self.picked_path.clone();
                    let files = self.picked_files.clone();
                    let add_audio_tx = self.thread_sender.clone();
                    let jh = thr
                        .spawn(move || add_audio_file(path, files, tag, add_audio_tx))
                        .unwrap();
                    self.joinhandles.push(jh);
                    self.tag_id = "E0040350".to_string();
                }
            }
            Action::AskChangeTagId => {
                self.show_id_popup = true;
                self.action = Action::ChangeTagId;
            }
            Action::ChangeTagId => {
                if self.tag_id_valid {
                    self.action = Action::None;
                    info!("changing tag id");
                    self.tag_id_valid = false;
                    change_tag_id(
                        &self.picked_path,
                        &self.files[self.selection.unwrap()],
                        &self.tag_id,
                    )
                    .unwrap_or_else(|e| self.error = Some(e));
                    self.action = Action::PopulateTable;
                    self.tag_id = "E0040350".to_string();
                }
            }
            Action::PopulateTable => {
                info!("populating table");
                self.processed = 0;
                self.action = Action::None;
                self.selection = None;
                self.files.clear();
                populate_table(&self.picked_path, &mut self.files, &self.tonies)
                    .unwrap_or_else(|e| self.error = Some(e));
            }
            Action::ExtractToOgg => {
                info!("extracting to ogg");
                self.action = Action::None;
                if let Some(path) = rfd::FileDialog::new().set_file_name(".ogg").save_file() {
                    let sel = self.files[self.selection.unwrap()].clone();
                    let add_audio_tx = self.thread_sender.clone();
                    let jh = thr
                        .spawn(move || extract_to_ogg(&sel, &path, add_audio_tx))
                        .unwrap();
                    self.joinhandles.push(jh);
                }
            }
            Action::ExtractAll => {
                info!("extracting all");
                self.action = Action::None;
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    let files = self.files.clone();
                    let add_audio_tx = self.thread_sender.clone();
                    let jh = thr
                        .spawn(move || extract_all(&files, &path, add_audio_tx))
                        .unwrap();
                    self.joinhandles.push(jh);
                }
            }
            Action::PlayFile => {
                info!("playing file");
                self.action = Action::None;
                let sel = self.files[self.selection.unwrap()].clone();
                let add_audio_tx = self.thread_sender.clone();
                let jh = thr.spawn(move || play_file(&sel, add_audio_tx)).unwrap();
                self.joinhandles.push(jh);
            }
            Action::DeleteFile => {
                info!("deleting file");
                if rfd::MessageDialog::new()
                    .set_description("Do you really want to delete this file?")
                    .set_buttons(rfd::MessageButtons::YesNo)
                    .show()
                    == rfd::MessageDialogResult::Yes
                {
                    delete_file(&self.files[self.selection.unwrap()])
                        .unwrap_or_else(|e| self.error = Some(e));
                }
                self.action = Action::PopulateTable;
            }
            Action::ShowFileData => {
                info!("showing file data");
                self.action = Action::None;
                let file = &self.files[self.selection.unwrap()];
                let _ = rfd::MessageDialog::new()
                    .set_description(format!(
                        "File info:\nPath: {}\nTag ID: {}\nAudio ID: {}\nAudio size: {} kbyte\nAudio tracks page addresses: {:?}\nSHA1 hash: {:x?}",
                        file.path.to_string_lossy(),
                        self.format_tag_id(&file.tag),
                        file.audio_id,
                        file.length / 1024,
                        file.chapter_pages,
                        file.hash,
                    ))
                    .set_buttons(rfd::MessageButtons::Ok)
                    .show();
            }
            Action::Processing(_) => {}
            Action::CurrentFileNo(_) => {}
            Action::CurrentFile(_) => {}
        }
        if let Ok(action) = self.thread_receiver.try_recv() {
            info!("recvd thread action: {:?}", action);
            ctx.request_repaint();
            match action {
                Action::PopulateTable => {
                    self.action = Action::PopulateTable;
                }
                Action::Processing(p) => {
                    self.processed = p;
                }
                Action::CurrentFileNo(n) => {
                    self.current_fileno = n;
                }
                Action::CurrentFile(f) => {
                    self.current_file = f;
                }
                _ => {}
            }
        }
        // join any finished threads
        for i in 0..self.joinhandles.len() {
            if self.joinhandles[i].is_finished() {
                let jh = self.joinhandles.remove(i);
                if let Err(e) = jh.join().unwrap() {
                    self.error = Some(e);
                }
            }
        }
    }
}
