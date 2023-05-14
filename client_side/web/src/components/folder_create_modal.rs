use std::{ops::Deref, pin::Pin};

use futures::Future;
use itertools::Itertools;
use sycamore::{prelude::*, futures::spawn_local_scoped};
use web_sys::HtmlInputElement;

pub trait UploadCallback<'a> {
    fn call(&self, filename: String) -> Pin<Box<dyn Future<Output=Result<(), String>> + 'a>>;
}
impl<'a, F: 'a, Return> UploadCallback<'a> for F
where
    F: Fn(String) -> Return,
    Return: Future<Output=Result<(), String>> + 'a
{
    fn call(&self, filename: String) -> Pin<Box<dyn Future<Output=Result<(), String>> + 'a>> {
        Box::pin(self(filename))
    }
}

#[derive(Prop)]
pub struct UploadModalProps<'a> {
    on_upload: &'a dyn UploadCallback<'a>,
    on_close: Box<dyn Fn() + 'a>,
}

#[derive(Clone)]
enum UploadState {
    AwaitingInput,
    Waiting,
    Error(String),
    Success
}

#[component]
pub fn FolderCreateModal<'a>(cx: Scope<'a>, props: UploadModalProps<'a>) -> View<DomNode> {
    let props = create_ref(cx, props);
    let input_ref = create_node_ref(cx);
    // Have the node persist, inserted as needed.
    let input_component = create_ref(cx, view! { cx,
        input(ref=input_ref, type="text") {}
    });
    let state = create_signal(cx, UploadState::AwaitingInput);
    let run_callback = create_ref(cx, move |_| {
        spawn_local_scoped(cx, async move {
            let node: DomNode = input_ref.get_raw();
            let input_node: HtmlInputElement = node.unchecked_into();
            state.set(UploadState::Waiting);
            match props.on_upload.call(input_node.value()).await {
                Ok(()) => state.set(UploadState::Success),
                Err(err) => state.set(UploadState::Error(err)),
            }
        });
    });
    let back = create_ref(cx, move |_| state.set(UploadState::AwaitingInput));
    let content = View::new_dyn(cx, move || {
        log::debug!("DOING STUFF FOLDER!");
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
        h1 { "Create Directory" }
        (content)
    }
}