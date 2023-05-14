use std::cell::Cell;

use stylist::style;
use sycamore::prelude::*;

#[derive(Prop)]
pub struct ModalWrapperProps<'a> {
    pub content: &'a ReadSignal<Option<View<DomNode>>>
}

#[component]
pub fn ModalWrapper<'a>(cx: Scope<'a>, props: ModalWrapperProps<'a>) -> View<DomNode> {
    let base_style = style! { r#"
        position: fixed;
        z-index: 1;
        padding-top: 100px;
        left: 0;
        top: 0;
        width: 100%;
        height: 100%;
        overflow: auto;
        background-color: rgba(0,0,0,0.4);
    "#
    }.expect("CSS should work");
    let hidden_style = style! { r#"
        display: none;
    "#
    }.expect("CSS should work");
    let shown_style = style! { r#"
        display: block;
    "#
    }.expect("CSS should work");
    let content_style = style! { r#"
        margin: auto;
        width: 80%;
        border-style: solid;
        background-color: rgb(255, 255, 255);
        padding-top: 1rem;
        padding-left: 1rem;
        padding-right: 1rem;
        padding-bottom: 1rem;
        border-radius: 1rem;
    "#}.expect("CSS should work");


    let content = props.content;
    let root = create_node_ref::<DomNode>(cx);
    let result = view! { cx,
        div(ref=root) {
            div(class=content_style.get_class_name()) {
                (
                    content.get().as_ref().as_ref().map_or_else(View::empty, Clone::clone)
                )
            }
        }
    };
    create_effect(cx, move || {
        let node = root.get_raw();
        if content.get().is_none() {
            node.set_class_name(&format!("{} {}", base_style.get_class_name(), hidden_style.get_class_name()));
        } else {
            node.set_class_name(&format!("{} {}", base_style.get_class_name(), shown_style.get_class_name()));
        }
    });
    return result;
}

#[derive(Clone)]
pub struct ModalHandler {
    content: RcSignal<(u64, Option<View<DomNode>>)>
}
impl ModalHandler {
    pub fn set_modal<'a>(&self, cx: Scope<'a>, f: impl Fn() -> View<DomNode> + 'a) {
        let index = self.content.get().0 + 1;
        self.content.set((index, Some(View::new_dyn(cx, f))));
        let content = self.content.clone();
        on_cleanup(cx, move || {
            // Remove the modal when the context is deleted.
            if content.get().0 == index {
                content.set((0, None));
            }
        });
    }
    pub fn clear_modal<'a>(&self) {
        self.content.set((0, None));
    }
}

pub fn install_modal_handler<'a>(cx: Scope<'a>) -> View<DomNode> {
    let content = create_rc_signal((0, None));
    provide_context(cx, ModalHandler { content: content.clone() });
    let signal = create_memo(cx, move || content.get().1.clone());
    view! { cx,
        ModalWrapper(content=signal)
    }
}
pub fn use_modal_handler<'a>(cx: Scope<'a>) -> &'a ModalHandler {
    use_context(cx)
}