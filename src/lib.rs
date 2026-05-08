pub mod simulation;
pub mod rule_editor;
pub mod rule_meta;
pub mod palette;
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
                Box::new(|cc| Ok(Box::new(gui::CellularApp::new(cc, 2000, 2000, Some(
                    r#"{"m":"Single","r":[{"rule":{"l":[0,0,0,1,0,1,1,0,1,1,0,0,1,0,1,1,0,0,1,1,1,1,1,1,1,1,0,1,0,0,1,1,0,1,0,1,0,0,1,1,0,0,0,0,0,0,0,1,0,0,1,1,0,0,0,0,1,1,0,0,1,1,0,0,0,1,0,0,1,1,0,0,0,1,0,1,0,0,0,1,0,1,1,0,1,0,0,0,1,0,1,0,1,0,0,0,0,1,0,1,1,1,1,1,1,0,0,1,0,1,0,0,1,1,1,1,0,0,0,0,0,1,1,1,1,1,1,0],"w":3,"s":2},"noise":0.0,"seed":0}]}"#
                ))))),
            )
            .await
            .expect("failed to start eframe");
    });

    Ok(())
}
