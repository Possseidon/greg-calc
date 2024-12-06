#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![cfg_attr(debug_assertions, allow(dead_code))]

pub mod math;
pub mod model;
mod ui;

use ui::app::GregCalc;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), eframe::Error> {
    env_logger::init();

    eframe::run_native(
        "GregCalc",
        Default::default(),
        Box::new(|creation_context| Ok(Box::new(GregCalc::new(creation_context)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("No window")
            .document()
            .expect("No document");

        let canvas = document
            .get_element_by_id("greg_calc")
            .expect("Failed to find greg_calc")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("greg_calc was not an HtmlCanvasElement");

        let start_result = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|creation_context| Ok(Box::new(GregCalc::new(creation_context)))),
            )
            .await;

        // Remove the loading text and spinner:
        if let Some(loading_text) = document.get_element_by_id("loading_text") {
            match start_result {
                Ok(_) => {
                    loading_text.remove();
                }
                Err(e) => {
                    loading_text.set_inner_html(
                        "<p> The app has crashed. See the developer console for details. </p>",
                    );
                    panic!("Failed to start eframe: {e:?}");
                }
            }
        }
    });
}
