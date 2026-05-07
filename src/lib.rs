pub mod simulation;
pub mod rule_editor;
pub mod glance_view;
pub mod gui;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub async fn start() -> Result<(), wasm_bindgen::JsValue> {
    use wasm_bindgen::JsCast as _;

    console_error_panic_hook::set_once();

    let canvas = web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .get_element_by_id("the_canvas_id")
        .unwrap()
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .unwrap();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async move {
        eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(gui::CellularApp::new(cc, 2000, 2000, Some("2;7;00010110110010110011111111010011010100110000000100110000110011000100110001010001011010001010100001011111100101001111000001111110"))))),
            )
            .await
            .expect("failed to start eframe");
    });

    Ok(())
}
