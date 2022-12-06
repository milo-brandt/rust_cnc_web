use std::{rc::Rc, cell::RefCell, cmp::{min, max}, fmt::Display};

use common::api;
use js_sys::Math::{sin, cos};
use quaternion_core::{Quaternion, QuaternionOps};
use stylist::style;
use sycamore::{prelude::*, futures::spawn_local_scoped};
use wasm_bindgen::{JsCast, JsValue, prelude::Closure};
use web_sys::{WebGl2RenderingContext, MouseEvent, WheelEvent};

use crate::{render::{compile_shader, link_program, add_loop_callback}, request::{self, HttpMethod}};

#[derive(Prop)]
pub struct InteractiveDisplayProps<'a> {
    positions: &'a ReadSignal<Vec<[f32; 3]>>
}

#[derive(Debug)]
struct MinMax {
    min: [f32; 3],
    max: [f32; 3],
}
fn enlarge_to(old: MinMax, new: &[f32; 3]) -> MinMax {
    MinMax {
        min: [
            old.min[0].min(new[0]),
            old.min[1].min(new[1]),
            old.min[2].min(new[2]),
        ],
        max: [
            old.max[0].max(new[0]),
            old.max[1].max(new[1]),
            old.max[2].max(new[2]),
        ]
    }
}
fn max_bounds_of(r: &MinMax) -> f32 {
    (r.max[0] - r.min[0]).max(r.max[1] - r.min[1]).max(r.max[2] - r.min[2])
}
fn center_of(r: &MinMax) -> [f32; 3] {
    [
        (r.max[0] + r.min[0]) * 0.5,
        (r.max[1] + r.min[1]) * 0.5,
        (r.max[2] + r.min[2]) * 0.5,
    ]
}

pub fn distance(a: &[f32; 3], b: &[f32; 3]) -> f32 {
    ((a[0]-b[0])*(a[0]-b[0]) + (a[1]-b[1])*(a[1]-b[1]) + (a[2]-b[2])*(a[2]-b[2])).sqrt()
}

#[component]
pub fn InteractiveDisplay<'a>(cx: Scope<'a>, props: InteractiveDisplayProps<'a>) -> View<DomNode> {
    let css_style = style! { r#"
        width: 100vw;
        height: 70vh;
    "#
    }.expect("CSS should work");

    let base: DomNode = node! { cx, 
        canvas(class=css_style.get_class_name(), width=500, height=500) {}
    };

    let css_style_slider = style! { r#"
        width: 90vw;
    "#
    }.expect("CSS should work");


    let canvas: web_sys::HtmlCanvasElement = base.inner_element().dyn_into::<web_sys::HtmlCanvasElement>().unwrap();

    let context = canvas
        .get_context("webgl2").unwrap()
        .unwrap()
        .dyn_into::<WebGl2RenderingContext>().unwrap();

    context.enable(WebGl2RenderingContext::BLEND);
    context.blend_func(WebGl2RenderingContext::SRC_ALPHA, WebGl2RenderingContext::ONE_MINUS_SRC_ALPHA);

    let vert_shader = compile_shader(
        &context,
        WebGl2RenderingContext::VERTEX_SHADER,
        r##"#version 300 es
        uniform mat3x3 transformation;
        uniform vec3 offset;
        uniform vec2 scale;

        in vec3 position;
        in float distance;

        out float depth;
        out float frag_distance;

        void main() {
            vec3 true_position = transformation * position + offset;
            gl_Position = vec4(vec3(scale, 0.5) * (transformation * position + offset), true_position.z);
            depth = position.z;
            frag_distance = distance;
        }
        "##,
    ).unwrap();

    let frag_shader = compile_shader(
        &context,
        WebGl2RenderingContext::FRAGMENT_SHADER,
        r##"#version 300 es
    


        precision highp float;
        in float depth;
        in float frag_distance;
        out vec4 outColor;

        uniform float depth_cutoff;
        
        void main() {
            outColor = vec4(1, frag_distance, 1, frag_distance < depth_cutoff ? 1.0 : 0.1);
        }
        "##,
    ).unwrap();
    let program = link_program(&context, &vert_shader, &frag_shader).unwrap();
    let position_attribute_location = context.get_attrib_location(&program, "position");
    let distance_attribute_location = context.get_attrib_location(&program, "distance");
    let buffer = context.create_buffer().ok_or("Failed to create buffer").unwrap();

    let mat_location = context.get_uniform_location(&program, "transformation").unwrap();
    let offset_location = context.get_uniform_location(&program, "offset").unwrap();
    let scale_location = context.get_uniform_location(&program, "scale").unwrap();
    let depth_cutoff_location = context.get_uniform_location(&program, "depth_cutoff").unwrap();

    let current_position: Rc<RefCell<Quaternion<f32>>> = Rc::new(RefCell::new((0.0, [0.0, 1.0, 0.0])));
    let current_zoom: Rc<RefCell<f32> > = Rc::new(RefCell::new(1.0));

    {
        let current_position = current_position.clone();
        let mouse_closure: Closure<dyn FnMut(MouseEvent)> = Closure::new(move |value: MouseEvent| {
            if value.buttons() & 1 == 1 {
                let x_dif = value.movement_x() as f32 * 0.001;
                let y_dif = value.movement_y() as f32 * 0.001;
                let transformation: Quaternion<f32> = quaternion_core::exp([-y_dif, -x_dif, 0.0]);
                if transformation.0 != 1.0 {
                    let mut value = current_position.borrow_mut();
                    *value = quaternion_core::mul(transformation, *value).normalize();
                    log::debug!("Multiply by {:?} => {:?}", transformation, value);
                }
            }
        });
        canvas.set_onmousemove(Some(mouse_closure.as_ref().unchecked_ref()));

        mouse_closure.forget();    
    }
    {
        let current_zoom = current_zoom.clone();
        let closure: Closure<dyn FnMut(WheelEvent)> = Closure::new(move |value: WheelEvent| {
            value.prevent_default();
            let delta_y = value.delta_y();
            let mut zoom_level = current_zoom.borrow_mut();
            if delta_y > 0.0 {
                *zoom_level *= 0.8;
            } else if delta_y < 0.0 {
                *zoom_level *= 1.0/0.8;
            }
        });
        canvas.set_onwheel(Some(closure.as_ref().unchecked_ref()));
        closure.forget();
    }
    let slider_value = create_signal(cx, "100".to_string());

    let ref_signal = create_rc_signal_from_rc(props.positions.get());
    let ref_signal_copy = ref_signal.clone();
    create_effect(cx, move || ref_signal_copy.set_rc(props.positions.get()));
    let progress_value = create_rc_signal(100.0);
    let progress_value_copy = progress_value.clone();
    create_effect(cx, move || match (*slider_value.get()).parse::<f32>() {
        Ok(value) => progress_value_copy.set(value * 0.01001 - 0.00001),  // a little more than 1% to make sure 100% is okay
        _ => ()
    });

    let depth_shown = create_rc_signal("???".to_string());
    let depth_shown_copy = depth_shown.clone();

    add_loop_callback(move |_| {
        let width = canvas.client_width() as u32;
        let height = canvas.client_height() as u32;
        canvas.set_width(width);
        canvas.set_height(height);
        context.viewport(0, 0, width as i32, height as i32);

        context.use_program(Some(&program));

        // let verticality = sin(t * 0.001);

        //let vertices: [f32; 9] = [-1.0, 1.0, 0.0,    verticality as f32, -1.0, 0.0,     1.0, 1.0, 0.0];

        let vertices_vec = &*ref_signal.get();
        if vertices_vec.is_empty() {
            return;
        }
        let first = vertices_vec[0];
        let bounds = vertices_vec.iter().fold(MinMax{ min: first, max: first }, enlarge_to);

        let max_dif = max_bounds_of(&bounds).max(0.001);
        let center = center_of(&bounds);
        // log::debug!("{:?} {:?} {}", bounds, center, max_dif);
        let scale_factor = 1.0 / max_dif;

        let (vertex_distances, mut total_distance) = 'vertex_distances: {
            let vec = ref_signal.get();
            let mut iterator = vec.iter();
            let mut result = Vec::new();
            let mut last_position = match iterator.next() {
                Some(position) => *position,
                None => break 'vertex_distances (result, 0.0)
            };
            let mut accumulator = 0.0;
            result.push(0.0);
            for position in iterator {
                let distance = distance(&last_position, position);
                accumulator += distance as f64;
                result.push(accumulator as f32);
                last_position = *position;
            }
            (result, accumulator)
        };
        total_distance += 0.01;

        let vertices: Vec<f32> = ref_signal.get().iter()
            .zip(vertex_distances)
            .flat_map(|(pos, distance)| [pos[0], pos[1], pos[2], distance / (total_distance as f32)])
            .collect();

        let aspect = (width as f32) / (height as f32);

        let true_center = quaternion_core::point_rotation(*current_position.borrow(), center);
        
        let dcm = quaternion_core::to_dcm(*current_position.borrow());

        // log::debug!("{:?}", dcm);

        context.uniform_matrix3fv_with_f32_array(Some(&mat_location), true, 
            &[dcm[0][0] * scale_factor, dcm[0][1] * scale_factor, dcm[0][2] * scale_factor,
              dcm[1][0] * scale_factor, dcm[1][1] * scale_factor, dcm[1][2] * scale_factor,
              dcm[2][0] * scale_factor, dcm[2][1] * scale_factor, dcm[2][2] * scale_factor]
        );

        let scale = *current_zoom.borrow();

        context.uniform3fv_with_f32_array(Some(&offset_location), &[-true_center[0] * scale_factor, -true_center[1]  * scale_factor, -true_center[2]  * scale_factor + 2.0]);
        context.uniform2fv_with_f32_array(Some(&scale_location), &[scale * 1.5 / aspect, scale * 1.5]);

        let progress_value = *progress_value.get();
        let cutoff = progress_value;
        //let cutoff = bounds.min[2] * (1.0 - progress_value) + bounds.max[2] * progress_value;
        depth_shown_copy.set(format!("DEPTH: {} mm", cutoff));

        context.uniform1f(Some(&depth_cutoff_location), cutoff);

        context.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&buffer));

        // Note that `Float32Array::view` is somewhat dangerous (hence the
        // `unsafe`!). This is creating a raw view into our module's
        // `WebAssembly.Memory` buffer, but if we allocate more pages for ourself
        // (aka do a memory allocation in Rust) it'll cause the buffer to change,
        // causing the `Float32Array` to be invalid.
        //
        // As a result, after `Float32Array::view` we have to be very careful not to
        // do any memory allocations before it's dropped.
        unsafe {
            let positions_array_buf_view = js_sys::Float32Array::view(&vertices);

            context.buffer_data_with_array_buffer_view(
                WebGl2RenderingContext::ARRAY_BUFFER,
                &positions_array_buf_view,
                WebGl2RenderingContext::STATIC_DRAW,
            );
        }

        //log::debug!("LOOPING!");

        let vao = context
            .create_vertex_array()
            .ok_or("Could not create vertex array object").unwrap();
        context.bind_vertex_array(Some(&vao));

        context.vertex_attrib_pointer_with_i32(
            position_attribute_location as u32,
            3,
            WebGl2RenderingContext::FLOAT,
            false,
            4 * 4,
            0,
        );
        context.vertex_attrib_pointer_with_i32(
            distance_attribute_location as u32,
            1,
            WebGl2RenderingContext::FLOAT,
            false,
            4 * 4,
            3 * 4,
        );
        context.enable_vertex_attrib_array(position_attribute_location as u32);
        context.enable_vertex_attrib_array(distance_attribute_location as u32);

        context.bind_vertex_array(Some(&vao));

        let vert_count = (vertices.len() / 4) as i32;
        context.clear_color(0.0, 0.0, 0.0, 1.0);
        context.clear(WebGl2RenderingContext::COLOR_BUFFER_BIT);
    
        context.draw_arrays(WebGl2RenderingContext::LINE_STRIP, 0, vert_count);
    
    });

    let canvas_view = View::new_node(base);
    view! { cx,
        (canvas_view)
        br {}
        input(type="range", min=0, max=100, value=100, step=0.01, bind:value=slider_value, class=css_style_slider.get_class_name()) {}
        br {}
        (depth_shown.get())
    }
}

#[derive(Prop)]
pub struct DisplayPageProps {
    name: String
}

#[component]
pub fn DisplayPage(cx: Scope, props: DisplayPageProps) -> View<DomNode> {
    let value = create_signal(cx, vec![]);
    spawn_local_scoped(cx, async {
        let result = request::request_with_json(
            HttpMethod::Post,
            api::EXAMINE_LINES_IN_GCODE_FILE,
            &api::ExamineGcodeFile {
                path: props.name.into()
            }
        ).await.unwrap();
        let result: Vec<[f32; 3]> = result.json().await.unwrap();
        // let mut old_value = (*value.get()).clone();
        // old_value.push([0.0, 0.0, 0.0]);
        value.set(result);
    });
    view! { cx,
        InteractiveDisplay(positions=value)
        br{}
        a(href="/send_gcode") { "Back!" }
    }
}
