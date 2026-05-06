// Native desktop entry point — the WASM entry is in lib.rs via #[wasm_bindgen(start)]

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("1D Cellular Automata")
            .with_inner_size([1200.0, 900.0]),
        ..Default::default()
    };

    eframe::run_native(
        "1D Cellular Automata",
        native_options,
        Box::new(|cc| Ok(Box::new(one_d_cellular_rust::gui::CellularApp::new(cc, 2000, 2000)))),
    )
}
