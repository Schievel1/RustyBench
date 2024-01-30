use eframe::{egui::ViewportBuilder, run_native};
use env_logger::Env;
use rusty_bench::ui::RustyBench;

fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    let _app: RustyBench = Default::default();
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default().with_inner_size([900.0, 350.0]),
        ..Default::default()
    };
    let _ = run_native(
        "rustyBench",
        options,
        Box::new(|cc| Box::new(RustyBench::new(cc))),
    );
}
