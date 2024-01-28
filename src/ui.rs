use anyhow::anyhow;
use anyhow::Result;
use eframe::{
    egui::{self, RichText},
    epaint::Color32,
};
use egui_extras::{Column, TableBuilder};
use egui_modal::Modal;
use std::{fs, path::PathBuf};

use crate::{
    add_audio_file, add_note, change_tag_id, extract_all, extract_to_ogg, play_file,
    populate_table, Teddyfile,
};

pub struct RustyBench {
    pub picked_path: PathBuf,
    pub picked_file: PathBuf,
    pub files: Vec<Teddyfile>,
    pub selection: Option<usize>,
    pub show_id_popup: bool,
    pub tag_id: String,
    pub tag_id_valid: bool,
}

impl Default for RustyBench {
    fn default() -> Self {
        Self {
            picked_path: Default::default(),
            picked_file: Default::default(),
            files: vec![],
            selection: None,
            show_id_popup: false,
            tag_id: Default::default(),
            tag_id_valid: false,
        }
    }
}

impl RustyBench {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            ..Default::default()
        }
    }
    fn toggle_row_selection(&mut self, row_index: usize, row_response: &egui::Response) {
        if row_response.clicked() {
            self.selection = Some(row_index);
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
                        }
                        self.files.clear();
                        self.selection = None;
                        populate_table(&self.picked_path, &mut self.files)
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
                    .column(Column::auto().at_least(300.0).resizable(true))
                    .column(Column::auto().at_least(150.0).resizable(true))
                    .column(Column::remainder())
                    .sense(egui::Sense::click())
                    .header(20.0, |mut header| {
                        header.col(|ui| {
                            ui.heading("Filename");
                        });
                        header.col(|ui| {
                            ui.heading("Tag ID");
                        });
                        header.col(|ui| {
                            ui.heading("Note");
                        });
                    })
                    .body(|body| {
                        body.rows(30.0, self.files.len(), |mut row| {
                            let row_index = row.index();
                            row.set_selected(self.selection == Some(row_index));
                            row.col(|ui| {
                                ui.label(self.files[row_index].path.to_string_lossy());
                            });
                            row.col(|ui| {
                                ui.label(&self.files[row_index].tag);
                            });

                            self.toggle_row_selection(row_index, &row.response());
                        });
                    });
            });
        });
        egui::TopBottomPanel::bottom("Botom Panel").show(ctx, |ui| {
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
                    self.files.clear();
                    self.selection = None;
                    populate_table(&self.picked_path, &mut self.files)
                }
                if ui
                    .add_sized(
                        [120., 40.],
                        egui::Button::new("Add audio file").fill(Color32::BLUE),
                    )
                    .clicked()
                    && self.picked_path.exists()
                {
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        self.picked_file = path;
                        // self.show_id_popup = true;
                        // TODO ask for tag ID in menu
                        self.show_id_popup = true;
                    }
                }
                if ui
                    .add_sized(
                        [120., 40.],
                        egui::Button::new("Delete file").fill(Color32::RED),
                    )
                    .clicked()
                    && self.selection.is_some()
                {
                    let path = self.files[self.selection.unwrap()].path.clone();
                    let parent = path.parent().unwrap();
                    fs::remove_file(&path).unwrap();
                    if parent.read_dir().unwrap().next().is_none() {
                        fs::remove_dir(parent).unwrap();
                    }
                    self.files.clear();
                    populate_table(&self.picked_path, &mut self.files)
                }
                if ui
                    .add_sized(
                        [120., 40.],
                        egui::Button::new("Play file").fill(Color32::RED),
                    )
                    .clicked()
                    && self.selection.is_some()
                {
                    play_file(&self.files[self.selection.unwrap()]);
                }
                if ui
                    .add_sized(
                        [120., 40.],
                        egui::Button::new("Extract to .ogg").fill(Color32::BLUE),
                    )
                    .clicked()
                    && self.selection.is_some()
                {
                    if let Some(path) = rfd::FileDialog::new().save_file() {
                        extract_to_ogg(&self.files[self.selection.unwrap()], &path);
                    }
                }
                if ui
                    .add_sized(
                        [120., 40.],
                        egui::Button::new("Extract all").fill(Color32::BLUE),
                    )
                    .clicked()
                {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        extract_all(&self.files, &path);
                    }
                }
                if ui
                    .add_sized(
                        [120., 40.],
                        egui::Button::new("Change tag ID").fill(Color32::BLUE),
                    )
                    .clicked()
                    && self.selection.is_some()
                {
                    change_tag_id(&self.files[self.selection.unwrap()])
                }
                if ui
                    .add_sized(
                        [120., 40.],
                        egui::Button::new("Add note").fill(Color32::BLUE),
                    )
                    .clicked()
                    && self.selection.is_some()
                {
                    add_note(&self.files[self.selection.unwrap()])
                }
            });
        });
        if self.show_id_popup {
            egui::Window::new("Please provide a tag ID")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("tag ID: ");
                    let text_edit = egui::TextEdit::singleline(&mut self.tag_id);
                    let response = ui.add(text_edit);
                    if response.changed() {
                        // TODO color the box depending on the validity of the tag ID
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Ok").clicked() {
                            self.show_id_popup = false;
                            self.tag_id_valid = true;
                        }
                    });
                });
        }
        if self.tag_id_valid {
            println!("tag ID is valid");
            self.tag_id_valid = false;
            add_audio_file(&self.picked_path, &self.picked_file, &self.tag_id);
            self.files.clear();
            populate_table(&self.picked_path, &mut self.files)
        }
    }
}

fn show_id_popup(ctx: &egui::Context) -> Result<String> {
    let mut tag_id = String::new();
    let mut show = true;
    while show {
        egui::Window::new("Please provide a tag ID")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("tag ID: ");
                let text_edit = egui::TextEdit::singleline(&mut tag_id);
                let response = ui.add(text_edit);
                if response.changed() {
                    // TODO color the box depending on the validity of the tag ID
                }
                ui.horizontal(|ui| {
                    if ui.button("Ok").clicked() {
                        show = false
                    }
                });
            });
    }
    // TODO check if tag_id is valid
    Ok(tag_id)
}
