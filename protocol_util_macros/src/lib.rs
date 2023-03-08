use std::{future::Future, io, fs, process::Command};

use proc_macro::*;
use syn::{spanned::{Spanned}, parse::{ParseStream, Parse}, Ident, ItemStruct, ItemEnum, Block, Token, parse, parse_macro_input, Type, Visibility, Generics, Path, parenthesized, token, punctuated::Punctuated, braced, PathArguments};
use quote::{quote, format_ident, ToTokens};
use tempfile::tempdir;

mod input;
mod output;

fn pretty_print<S: ToString>(code: S) -> io::Result<()> {
    let tmp_dir = tempdir().unwrap();
    let file_path = tmp_dir.path().join("rendering.rs");

    fs::write(&file_path, code.to_string())?;

    Command::new("rustfmt")
        .arg(&file_path)
        .spawn()?
        .wait()?;
    
    println!("{}", fs::read_to_string(&file_path)?);

    Ok(())
}

#[proc_macro_attribute]
pub fn protocol_type(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Printing seems to upset rust_analyzer?
    // println!("attr: \"{}\"", attr.to_string());
    // println!("item: \"{}\"", item.to_string());
    let input_type: input::Type = parse_macro_input!(item);
    let output_type = output::Type::from_input(input_type);
    // let coro_spec = CoroSpecification::from_definition(test.clone());

    // pretty_print(output_type.to_token_stream()).unwrap();

    // coro_spec.test_generate().into()
    output_type.to_token_stream().into()
}
