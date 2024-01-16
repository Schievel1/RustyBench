use eframe::{run_native, egui};
use rusty_bench::ui::RustyBench;

fn main() {
    env_logger::init();
    let _app: RustyBench = Default::default();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([240.0, 320.0]),
        ..Default::default()
    };
    let _ = run_native(
        "rustyBench",
        options,
        Box::new(|cc| Box::new(RustyBench::new(cc))),
    );
}
