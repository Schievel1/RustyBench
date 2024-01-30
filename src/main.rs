use eframe::{egui::ViewportBuilder, run_native};
use rusty_bench::ui::RustyBench;
use clap::Parser;

#[derive(Debug, Parser)]
struct Cli {
    #[command(flatten)]
    verbose: clap_verbosity_flag::Verbosity,
}

fn main() {
    // Parse CLI arguments.
    let cli = Cli::parse();
    // Initialize logging.
    env_logger::Builder::new()
    .filter_level(cli.verbose.log_level_filter())
    .init();

    // Run GUI
    let _app: RustyBench = Default::default();
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default().with_inner_size([900.0, 350.0]),
        ..Default::default()
    };
    let _ = run_native(
        "RustyBench",
        options,
        Box::new(|cc| Box::new(RustyBench::new(cc))),
    );
}
