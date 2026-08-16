fn main() -> eframe::Result<()> {
    let initial_path = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([960.0, 640.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Evograph",
        options,
        Box::new(move |_cc| Ok(Box::new(evograph::EvographApp::new(initial_path)))),
    )
}
