//! Drawing formulas. index.html loads KaTeX before the app starts, so `katex`
//! is always available here.
use js_sys::{Object, Reflect};
use leptos::prelude::*;
use mocktest_domain::math::{Piece, pieces};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = katex, js_name = renderToString, catch)]
    fn render_to_string(source: &str, options: &JsValue) -> Result<String, JsValue>;
}

/// The HTML for one formula. Fails when KaTeX cannot draw it.
fn draw(source: &str, display: bool) -> Result<String, JsValue> {
    let options = Object::new();
    Reflect::set(&options, &JsValue::from_str("displayMode"), &JsValue::from_bool(display))?;
    Reflect::set(&options, &JsValue::from_str("throwOnError"), &JsValue::TRUE)?;
    let options: JsValue = options.into();
    render_to_string(source, &options)
}

/// Text that may contain formulas. Plain words appear as written; formulas are
/// drawn. Put it inside an element that already has the right white-space rule.
#[component]
pub fn Rich(text: String) -> impl IntoView {
    pieces(&text)
        .into_iter()
        .map(|piece| match piece {
            Piece::Text(words) => words.into_any(),
            Piece::Math { display, source } => formula(source, display),
        })
        .collect_view()
}

/// One formula. If KaTeX cannot draw it, the source is shown as written so the
/// student still sees the text.
fn formula(source: String, display: bool) -> AnyView {
    match draw(&source, display) {
        Ok(html) => {
            let class = if display { "block my-2 overflow-x-auto" } else { "" };
            view! { <span class=class inner_html=html></span> }.into_any()
        }
        Err(_) => {
            let delimiter = if display { "$$" } else { "$" };
            view! {
                <span class="text-bad" title="This formula could not be drawn">
                    {format!("{delimiter}{source}{delimiter}")}
                </span>
            }
            .into_any()
        }
    }
}

/// Messages for formulas KaTeX cannot draw, for the check made when a quiz is saved.
pub fn broken_formulas(text: &str) -> Vec<String> {
    pieces(text)
        .into_iter()
        .filter_map(|piece| match piece {
            Piece::Math { display, source } => draw(&source, display).err().map(|_| {
                let delimiter = if display { "$$" } else { "$" };
                format!("the formula {delimiter}{source}{delimiter} cannot be drawn. Check its LaTeX.")
            }),
            Piece::Text(_) => None,
        })
        .collect()
}
