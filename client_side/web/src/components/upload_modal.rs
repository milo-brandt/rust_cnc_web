use std::ops::Deref;

use futures::Future;
use itertools::Itertools;
use sycamore::{prelude::*, futures::spawn_local_scoped};
use web_sys::HtmlInputElement;

#[derive(Prop)]
pub struct UploadModalProps<'a> {
    on_upload: Box<dyn Fn(Vec<web_sys::File>) -> Box<dyn Future<Output=Result<(), String>> + 'a> + 'a>,
    on_close: Box<dyn Fn() + 'a>,
}

fn get_files_from_file_input(input_node: HtmlInputElement) -> Vec<web_sys::File> {
    if let Some(files) = input_node.files() {
        (0..files.length()).into_iter().filter_map(|index| files.item(index)).collect_vec()
    } else {
        Vec::new()
    }
}

#[derive(Clone)]
enum UploadState {
    AwaitingInput,
    Waiting,
    Error(String),
    Success
}

#[component]
pub fn UploadModal<'a>(cx: Scope<'a>, props: UploadModalProps<'a>) -> View<DomNode> {
    let props = create_ref(cx, props);
    let input_ref = create_node_ref(cx);
    // Have the node persist, inserted as needed.
    let input_component = create_ref(cx, view! { cx,
        input(ref=input_ref, type="file", multiple=true) {}
    });
    let state = create_signal(cx, UploadState::AwaitingInput);
    let run_callback = create_ref(cx, move |_| {
        spawn_local_scoped(cx, async move {
            let node: DomNode = input_ref.get_raw();
            let input_node: HtmlInputElement = node.unchecked_into();
            state.set(UploadState::Waiting);
            match Box::into_pin((props.on_upload)(get_files_from_file_input(input_node))).await {
                Ok(()) => state.set(UploadState::Success),
                Err(err) => state.set(UploadState::Error(err)),
            }
        });
    });
    let back = create_ref(cx, move |_| state.set(UploadState::AwaitingInput));
    let content = View::new_dyn(cx, move || {
        log::debug!("DOING STUFF!");
        match state.get().as_ref().clone() {
            UploadState::AwaitingInput => view! { cx, 
                (input_component) br{} br{}
                button(on:click=run_callback) { "Upload" } button(on:click=|_| (props.on_close)()) { "Cancel" } 
            },
            UploadState::Waiting => view! { cx,
                "Uploading..."
            },
            UploadState::Error(err) => view! { cx,
                (err) br{}
                button(on:click=back) { "Back" } button(on:click=|_| (props.on_close)()) { "Close" } 
            },
            UploadState::Success => view! { cx,
                "Success!" br{} br{}
                button(on:click=|_| (props.on_close)()) { "Close" }
            }
        }
    });
    view! { cx,
        h1 { "Upload Files" }
        (content)
    }
}