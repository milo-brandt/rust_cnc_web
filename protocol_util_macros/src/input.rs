use itertools::Itertools;
use syn::{parse::{ParseStream, Parse}, Error};


#[derive(Debug)]
pub struct Structure {
    pub generics: syn::Generics,
    pub name: syn::Ident,
    pub members: Vec<(syn::Ident, syn::Type)>,
}
#[derive(Debug)]
pub struct Enum {
    pub generics: syn::Generics,
    pub name: syn::Ident,
    pub variants: Vec<(syn::Ident, syn::Type)>,
}
#[derive(Debug)]
pub enum Type {
    Structure(Structure),
    Enum(Enum)
}


impl Parse for Type {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let input_item: syn::Item = input.parse()?;
        match input_item {
            syn::Item::Struct(input_structure) => {
                let members = match input_structure.fields {
                    syn::Fields::Named(named_fields) => {
                        named_fields.named.into_iter().map(|field| {
                            (field.ident.expect("named fields should have ident"), field.ty)
                        }).collect_vec()
                    },
                    fields => return Err(Error::new_spanned(fields, "Can only handle named fields.")) 
                };
                Ok(Type::Structure(Structure {
                    generics: input_structure.generics,
                    name: input_structure.ident,
                    members
                }))
            },
            syn::Item::Enum(input_enum) => {
                let variants = input_enum.variants.into_iter().map(|variant| {
                    match variant.fields {
                        syn::Fields::Unnamed(unnamed_fields) => {
                            if unnamed_fields.unnamed.len() != 1 {
                                Err(Error::new_spanned(unnamed_fields, "Can only handle a single unnamed field."))
                            } else {
                                let field = unnamed_fields.unnamed.into_iter().next().expect("iterator should be non-empty");
                                Ok((variant.ident, field.ty))
                            }
                        },
                        fields => Err(Error::new_spanned(fields, "Can only handle a unnamed fields."))
                    }
                }).collect::<Result<Vec<_>, _>>()?;
                Ok(Type::Enum(Enum {
                    generics: input_enum.generics,
                    name: input_enum.ident,
                    variants
                }))
            },
            input_item => return Err(Error::new_spanned(input_item, "Can only handle structs and enums.")) 
        }
    }
}

