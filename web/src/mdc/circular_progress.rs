use std::io::Read;

use sycamore::prelude::*;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/src/mdc/circular_progress.js")]
extern "C" {
    fn register_circular_progress(node: web_sys::Node) -> wasm_bindgen::JsValue;
    fn set_determinate(mdc_text_field: wasm_bindgen::JsValue, determinate: bool);
    fn set_progress(mdc_text_field: wasm_bindgen::JsValue, progress: f32);
    fn deregister_circular_progress(mdc_text_field: wasm_bindgen::JsValue);
}

#[derive(Prop)]
pub struct CircularProgressProps<'a> {
    density: f32,
    determinate: &'a ReadSignal<bool>,
    progress: &'a ReadSignal<f32>,
}

#[component]
pub fn CircularProgress<'a>(cx: Scope<'a>, props: CircularProgressProps<'a>) -> View<DomNode> {
// https://github.com/material-components/material-web/blob/mwc/packages/circular-progress/mwc-circular-progress-base.ts 
    let side_length = 48.0 + props.density * 4.0;
    let center = side_length * 0.5;
    let circle_radius = if props.density >= -3.0 { 18.0 + props.density * 11.0 / 6.0 } else { 12. + (props.density + 3.0) * 5.0 / 4.0 };
    let stroke_width = if props.density >= -3.0 { 4.0 + props.density * (1.0 / 3.0) } else { 3.0 + (props.density + 3.0) * (1.0 / 6.0) };
    let gap_stroke_width = stroke_width * 0.8;

    let circumference = 2.0 * 3.1415926 * circle_radius;
    let half_circumference = circumference * 0.5;
    let view_box = create_ref(cx, format!("0 0 {} {}", side_length, side_length));
    let style_string = create_ref(cx, format!("width:{}px;height:{}px;", side_length, side_length));

    let base: DomNode = node! { cx, 
        div(class="mdc-circular-progress", style=style_string, role="progressbar", aria-label="Example Progress Bar", aria-valuemin="0", aria-valuemax="1") {
            div(class="mdc-circular-progress__determinate-container") {
                svg(class="mdc-circular-progress__determinate-circle-graphic", viewBox=view_box) {
                    circle(class="mdc-circular-progress__determinate-track", cx=center, cy=center, r=circle_radius, stroke-width=stroke_width) {}
                    circle(class="mdc-circular-progress__determinate-circle", cx=center, cy=center, r=circle_radius, stroke-dasharray=circumference, stroke-dashoffset=circumference, stroke-width=stroke_width) {}
                }
            }
            div(class="mdc-circular-progress__indeterminate-container") {
                div(class="mdc-circular-progress__spinner-layer") {
                    div(class="mdc-circular-progress__circle-clipper mdc-circular-progress__circle-left") {
                        svg(class="mdc-circular-progress__indeterminate-circle-graphic", viewBox=view_box) {
                            circle(cx=center, cy=center, r=circle_radius, stroke-dasharray=circumference, stroke-dashoffset=half_circumference, stroke-width=stroke_width) {}
                        }
                    }
                    div(class="mdc-circular-progress__gap-patch") {
                        svg(class="mdc-circular-progress__indeterminate-circle-graphic", viewBox=view_box) {
                            circle(cx=center, cy=center, r=circle_radius, stroke-dasharray=circumference, stroke-dashoffset=half_circumference, stroke-width=gap_stroke_width) {}
                        }
                    }
                    div(class="mdc-circular-progress__circle-clipper mdc-circular-progress__circle-right") {
                        svg(class="mdc-circular-progress__indeterminate-circle-graphic", viewBox=view_box) {
                            circle(cx=center, cy=center, r=circle_radius, stroke-dasharray=circumference, stroke-dashoffset=half_circumference, stroke-width=stroke_width) {}
                        }
                    }
                }
            }
        }
    };

    let mdc_ripple = create_ref(cx, register_circular_progress(base.inner_element()));
    create_effect(cx, || set_determinate(mdc_ripple.clone(), *props.determinate.get()));
    create_effect(cx, || set_progress(mdc_ripple.clone(), *props.progress.get()));
    on_cleanup(cx, || deregister_circular_progress(mdc_ripple.clone()));

    View::new_node(base)

}