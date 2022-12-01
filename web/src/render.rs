
use std::collections::HashMap;

use js_sys::Math::sin;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{WebGl2RenderingContext, WebGlProgram, WebGlShader};

struct RunLoop {
    entrypoint: Closure<dyn Fn(f64)>,
    callbacks: HashMap<u64, Box<dyn FnMut(f64)>>,
    next_index: u64,
}



static mut RUN_LOOP: Option<RunLoop> = None;

fn mutate_run_loop<T>(f: impl FnOnce(&mut RunLoop) -> T) -> T {
    unsafe {
        if RUN_LOOP.is_none() {
            RUN_LOOP = Some(RunLoop {
                entrypoint: Closure::new(run_loop_callback),
                callbacks: Default::default(),
                next_index: 0,
            });
            let window = web_sys::window().unwrap();
            window.request_animation_frame(RUN_LOOP.as_mut().unwrap().entrypoint.as_ref().unchecked_ref()).unwrap();
        }
        f(RUN_LOOP.as_mut().unwrap())
    }
}

fn run_loop_callback(value: f64) {
    mutate_run_loop(|run_loop| {
        for (_, callback) in &mut run_loop.callbacks {
            callback(value)
        }
    });
    let window = web_sys::window().unwrap();
    unsafe {
        window.request_animation_frame(RUN_LOOP.as_mut().unwrap().entrypoint.as_ref().unchecked_ref()).unwrap();
    }
}

pub fn add_loop_callback(f: impl FnMut(f64) + 'static) {
    mutate_run_loop(|run_loop| {
        run_loop.callbacks.insert(run_loop.next_index, Box::new(f));
        run_loop.next_index += 1;
    });
}

pub fn start() -> Result<(), JsValue> {
    /*
    let window = web_sys::window().unwrap();

    let document = window.document().unwrap();
    let canvas = document.get_element_by_id("canvas").unwrap();
    let canvas: web_sys::HtmlCanvasElement = canvas.dyn_into::<web_sys::HtmlCanvasElement>()?;

    let context = canvas
        .get_context("webgl2")?
        .unwrap()
        .dyn_into::<WebGl2RenderingContext>()?;

    let vert_shader = compile_shader(
        &context,
        WebGl2RenderingContext::VERTEX_SHADER,
        r##"#version 300 es
 
        in vec4 position;

        void main() {
        
            gl_Position = position;
        }
        "##,
    )?;

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
    )?;
    let program = link_program(&context, &vert_shader, &frag_shader)?;
    add_loop_callback(move |t| {
        let width = canvas.client_width() as u32;
        let height = canvas.client_height() as u32;
        canvas.set_width(width);
        canvas.set_height(height);
        log::debug!("{} x {}", width, height);
        context.viewport(0, 0, width as i32, height as i32);

        context.use_program(Some(&program));

        let verticality = sin(t * 0.001);

        let vertices: [f32; 9] = [-1.0, 1.0, 0.0,    verticality as f32, -1.0, 0.0,     1.0, 1.0, 0.0];

        let position_attribute_location = context.get_attrib_location(&program, "position");
        let buffer = context.create_buffer().ok_or("Failed to create buffer").unwrap();
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
    
        context.draw_arrays(WebGl2RenderingContext::TRIANGLES, 0, vert_count);
    
    });
    */
    Ok(())
}


pub fn compile_shader(
    context: &WebGl2RenderingContext,
    shader_type: u32,
    source: &str,
) -> Result<WebGlShader, String> {
    let shader = context
        .create_shader(shader_type)
        .ok_or_else(|| String::from("Unable to create shader object"))?;
    context.shader_source(&shader, source);
    context.compile_shader(&shader);

    if context
        .get_shader_parameter(&shader, WebGl2RenderingContext::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(shader)
    } else {
        Err(context
            .get_shader_info_log(&shader)
            .unwrap_or_else(|| String::from("Unknown error creating shader")))
    }
}

pub fn link_program(
    context: &WebGl2RenderingContext,
    vert_shader: &WebGlShader,
    frag_shader: &WebGlShader,
) -> Result<WebGlProgram, String> {
    let program = context
        .create_program()
        .ok_or_else(|| String::from("Unable to create shader object"))?;

    context.attach_shader(&program, vert_shader);
    context.attach_shader(&program, frag_shader);
    context.link_program(&program);

    if context
        .get_program_parameter(&program, WebGl2RenderingContext::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(program)
    } else {
        Err(context
            .get_program_info_log(&program)
            .unwrap_or_else(|| String::from("Unknown error creating program object")))
    }
}