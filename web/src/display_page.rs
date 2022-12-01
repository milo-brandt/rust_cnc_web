use std::{rc::Rc, cell::RefCell, cmp::{min, max}};

use js_sys::Math::{sin, cos};
use quaternion_core::{Quaternion, QuaternionOps};
use stylist::style;
use sycamore::prelude::*;
use wasm_bindgen::{JsCast, JsValue, prelude::Closure};
use web_sys::{WebGl2RenderingContext, MouseEvent};

use crate::render::{compile_shader, link_program, add_loop_callback};

#[derive(Prop)]
pub struct DisplayProps<'a> {
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

#[component]
pub fn DisplayPage<'a>(cx: Scope<'a>, props: DisplayProps<'a>) -> View<DomNode> {
    let css_style = style! { r#"
        width: 100vw;
        height: 70vh;
    "#
    }.expect("CSS should work");

    let base: DomNode = node! { cx, 
        canvas(class=css_style.get_class_name(), width=500, height=500) {}
    };


    let canvas: web_sys::HtmlCanvasElement = base.inner_element().dyn_into::<web_sys::HtmlCanvasElement>().unwrap();

    let context = canvas
        .get_context("webgl2").unwrap()
        .unwrap()
        .dyn_into::<WebGl2RenderingContext>().unwrap();

    let vert_shader = compile_shader(
        &context,
        WebGl2RenderingContext::VERTEX_SHADER,
        r##"#version 300 es
        uniform mat3x3 transformation;
        uniform vec3 offset;
        uniform vec2 scale;
        in vec3 position;

        void main() {
            vec3 true_position = transformation * position + offset;
            gl_Position = vec4(vec3(scale, 0.5) * (transformation * position + offset), true_position.z);
        }
        "##,
    ).unwrap();

    let frag_shader = compile_shader(
        &context,
        WebGl2RenderingContext::FRAGMENT_SHADER,
        r##"#version 300 es
    
        precision highp float;
        out vec4 outColor;
        
        void main() {
            outColor = vec4(1, 1, 1, 1);
        }
        "##,
    ).unwrap();
    let program = link_program(&context, &vert_shader, &frag_shader).unwrap();
    let position_attribute_location = context.get_attrib_location(&program, "position");
    let buffer = context.create_buffer().ok_or("Failed to create buffer").unwrap();

    let mat_location = context.get_uniform_location(&program, "transformation").unwrap();
    let offset_location = context.get_uniform_location(&program, "offset").unwrap();
    let scale_location = context.get_uniform_location(&program, "scale").unwrap();

    let current_position: Rc<RefCell<Quaternion<f32>>> = Rc::new(RefCell::new((1.0, [0.0, 0.0, 0.0])));

    let mut last_t = None;

    {
        let current_position = current_position.clone();
        let mouse_closure: Closure<dyn FnMut(MouseEvent)> = Closure::new(move |value: MouseEvent| {
            if value.buttons() & 1 == 1 {
                let x_dif = value.movement_x() as f32 * 0.001;
                let y_dif = value.movement_y() as f32 * 0.001;
                let transformation: Quaternion<f32> = quaternion_core::exp([y_dif, x_dif, 0.0]);
                if transformation.0 != 1.0 {
                    let mut value = current_position.borrow_mut();
                    *value = quaternion_core::mul(*value, transformation).normalize();
                    log::debug!("Multiply by {:?} => {:?}", transformation, value);
                }
            }
        });
        canvas.set_onmousemove(Some(mouse_closure.as_ref().unchecked_ref()));

        mouse_closure.forget();    
    }

    let ref_signal = create_rc_signal_from_rc(props.positions.get());
    let ref_signal_copy = ref_signal.clone();
    create_effect(cx, move || ref_signal_copy.set_rc(props.positions.get()));


    add_loop_callback(move |t| {
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
        log::debug!("{:?} {:?} {}", bounds, center, max_dif);
        let scale_factor = 1.0 / max_dif;

        let mut vertices: Vec<f32> = ref_signal.get().iter().flatten().cloned().collect();

        let aspect = (width as f32) / (height as f32);

        let c = cos(t * 0.0017) as f32;
        let s = sin(t * 0.0017) as f32;
        let s2 = sin(t*0.001) as f32 * 0.2;

        if let Some(last_t) = last_t {
            // let dif = (t - last_t) as f32;
            let mut value = current_position.borrow_mut();
            // *value = quaternion_core::mul(*value, quaternion_core::from_rotation_vector([0.0, 0.0, dif*0.001])).normalize();
        }
        last_t = Some(t);

        let true_center = quaternion_core::point_rotation(quaternion_core::conj(*current_position.borrow()), center);
        
        let dcm = quaternion_core::to_dcm(*current_position.borrow());

        // log::debug!("{:?}", dcm);

        context.uniform_matrix3fv_with_f32_array(Some(&mat_location), false, 
            &[dcm[0][0] * scale_factor, dcm[0][1] * scale_factor, dcm[0][2] * scale_factor,
              dcm[1][0] * scale_factor, dcm[1][1] * scale_factor, dcm[1][2] * scale_factor,
              dcm[2][0] * scale_factor, dcm[2][1] * scale_factor, dcm[2][2] * scale_factor]
        );

        context.uniform3fv_with_f32_array(Some(&offset_location), &[-true_center[0] * scale_factor, -true_center[1]  * scale_factor, -true_center[2]  * scale_factor + 2.0]);
        context.uniform2fv_with_f32_array(Some(&scale_location), &[1.5 / aspect, 1.5]);

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

        let vao = context
            .create_vertex_array()
            .ok_or("Could not create vertex array object").unwrap();
        context.bind_vertex_array(Some(&vao));

        context.vertex_attrib_pointer_with_i32(
            position_attribute_location as u32,
            3,
            WebGl2RenderingContext::FLOAT,
            false,
            0,
            0,
        );
        context.enable_vertex_attrib_array(position_attribute_location as u32);

        context.bind_vertex_array(Some(&vao));

        let vert_count = (vertices.len() / 3) as i32;
        context.clear_color(0.0, 0.0, 0.0, 1.0);
        context.clear(WebGl2RenderingContext::COLOR_BUFFER_BIT);
    
        context.draw_arrays(WebGl2RenderingContext::LINE_STRIP, 0, vert_count);
    
    });

    View::new_node(base)
}