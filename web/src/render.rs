
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