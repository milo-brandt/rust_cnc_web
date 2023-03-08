use crate::input;
use proc_macro2::{TokenStream, Span};
use quote::{quote, TokenStreamExt, ToTokens, format_ident};
use convert_case::{Casing, Case};
use syn::token::Struct;

#[derive(Debug)]
pub struct Structure {
    pub name: syn::Ident,
    pub rx_name: syn::Ident,
    pub tx_name: syn::Ident,
    pub members: Vec<(syn::Ident, syn::Type)>,
}

#[derive(Debug)]
pub struct Enum {
    pub name: syn::Ident,
    pub rx_name: syn::Ident,
    pub tx_name: syn::Ident,
    pub variants: Vec<(syn::Ident, syn::Type)>,
}
#[derive(Debug)]
pub enum Type {
    Structure(Structure),
    Enum(Enum)
}

impl Structure {
    pub fn from_input(value: input::Structure) -> Self {
        let rx_name = format_ident!("{}Rx", value.name);
        let tx_name = format_ident!("{}Tx", value.name);
        Self {
            name: value.name,
            rx_name,
            tx_name,
            members: value.members
        }
    }
    /*
        Generation functions...
    */
    fn generate_base_struct(&self) -> TokenStream {
        let name = &self.name;
        let members = self.members.iter().map(|(name, ty)| {
            quote! {
                pub #name: #ty
            }
        });
        quote! {
            #[derive(::serde::Serialize, ::serde::Deserialize)]
            pub struct #name {
                #(#members),*
            }
        }
    }
    fn generate_received_struct(&self) -> TokenStream {
        let name = &self.rx_name;
        let members = self.members.iter().map(|(name, ty)| {
            quote! {
                pub #name: <#ty as ::protocol_util::generic::Receivable>::ReceivedAs
            }
        });
        quote! {
            pub struct #name {
                #(#members),*
            }
        }
    }
    fn generate_received_impl(&self) -> TokenStream {
        let name = &self.name;
        let rx_name = &self.rx_name;
        let members = self.members.iter().map(|(name, _)| {
            quote! {
                #name: self.#name.receive_in_context(context)
            }
        });
        quote! {
            impl ::protocol_util::generic::Receivable for #name {
                type ReceivedAs = #rx_name;

                fn receive_in_context(self, context: &::protocol_util::communication_context::Context) -> Self::ReceivedAs {
                    #rx_name {
                        #(#members),*
                    }
                }
            }
        }
    }
    fn generate_sent_struct(&self) -> TokenStream {
        let name = &self.tx_name;
        let (members, type_params): (Vec<_>, Vec<_>) = self.members.iter().map(|(name, _ty)| {
            let type_name = syn::Ident::new(&name.to_string().from_case(Case::Snake).to_case(Case::UpperCamel), Span::call_site());
            (quote! {
                pub #name: #type_name
            }, quote! {
                #type_name
            })
        }).unzip();
        quote! {
            pub struct #name <#(#type_params),*> {
                #(#members),*
            }
        }
    }
    fn generate_sent_impl(&self) -> TokenStream {
        let name = &self.name;
        let tx_name = &self.tx_name;
        let (members, rest): (Vec<_>, Vec<_>) = self.members.iter().map(|(name, ty)| {
            let type_name = syn::Ident::new(&name.to_string().from_case(Case::Snake).to_case(Case::UpperCamel), Span::call_site());
            (quote! {
                #name: self.#name.prepare_in_context(context)
            }, (quote! {
                #type_name: ::protocol_util::generic::SendableAs<#ty>
            }, type_name))
        }).unzip();
        let (generics_bounded, generics): (Vec<_>, Vec<_>) = rest.into_iter().unzip();
        quote! {
            impl<#(#generics_bounded),*> ::protocol_util::generic::SendableAs<#name> for #tx_name<#(#generics),*> {
                fn prepare_in_context(self, context: &::protocol_util::communication_context::DeferingContext) -> #name {
                    #name {
                        #(#members),*
                    }
                }
            }
        }
    }
}

impl Enum {
    pub fn from_input(value: input::Enum) -> Self {
        let rx_name = format_ident!("{}Rx", value.name);
        let tx_name = format_ident!("{}Tx", value.name);
        Self {
            name: value.name,
            rx_name,
            tx_name,
            variants: value.variants
        }
    }
    /*
        Generation functions...
    */
    fn generate_base_struct(&self) -> TokenStream {
        let name = &self.name;
        let variants = self.variants.iter().map(|(variant, ty)| {
            quote! {
                #variant(#ty)
            }
        });
        quote! {
            #[derive(::serde::Serialize, ::serde::Deserialize)]
            pub enum #name {
                #(#variants),*
            }
        }
    }
    fn generate_received_struct(&self) -> TokenStream {
        let name = &self.rx_name;
        let variants = self.variants.iter().map(|(variant, ty)| {
            quote! {
                #variant(<#ty as ::protocol_util::generic::Receivable>::ReceivedAs)
            }
        });
        quote! {
            pub enum #name {
                #(#variants),*
            }
        }
    }
    fn generate_received_impl(&self) -> TokenStream {
        let name = &self.name;
        let rx_name = &self.rx_name;
        let variants = self.variants.iter().map(|(variant, _)| {
            quote! {
                #name::#variant(value) => #rx_name::#variant(value.receive_in_context(context))
            }
        });
        quote! {
            impl ::protocol_util::generic::Receivable for #name {
                type ReceivedAs = #rx_name;

                fn receive_in_context(self, context: &::protocol_util::communication_context::Context) -> Self::ReceivedAs {
                    match self {
                        #(#variants),*
                    }
                }
            }
        }
    }
    fn generate_sent_struct(&self) -> TokenStream {
        let name = &self.tx_name;
        let (variants, type_params): (Vec<_>, Vec<_>) = self.variants.iter().map(|(variant, _ty)| {
            let type_name = syn::Ident::new(&variant.to_string(), Span::call_site());
            (quote! {
                #variant(#type_name)
            }, quote! {
                #type_name
            })
        }).unzip();
        quote! {
            pub enum #name <#(#type_params),*> {
                #(#variants),*
            }
        }
    }
    fn generate_sent_impl(&self) -> TokenStream {
        let name = &self.name;
        let tx_name = &self.tx_name;
        let (variants, rest): (Vec<_>, Vec<_>) = self.variants.iter().map(|(variant, ty)| {
            let type_name = syn::Ident::new(&variant.to_string(), Span::call_site());
            (quote! {
                #tx_name::#variant(value) => #name::#variant(value.prepare_in_context(context))
            }, (quote! {
                #type_name: ::protocol_util::generic::SendableAs<#ty>
            }, type_name))
        }).unzip();
        let (generics_bounded, generics): (Vec<_>, Vec<_>) = rest.into_iter().unzip();
        quote! {
            impl<#(#generics_bounded),*> ::protocol_util::generic::SendableAs<#name> for #tx_name<#(#generics),*> {
                fn prepare_in_context(self, context: &::protocol_util::communication_context::DeferingContext) -> #name {
                    match self {
                        #(#variants),*
                    }
                }
            }
        }
    }
    fn generate_sent_aliases(&self) -> TokenStream {
        let tx_name = &self.tx_name;
        let aliases = (0..self.variants.len()).map(|index| {
            let variant = &self.variants[index].0;
            let name = format_ident!("{}{}", self.tx_name, variant.to_string());
            let output_generics = (0..self.variants.len()).map(|inner_index| {
                if index == inner_index {
                    quote!{ #variant }
                } else {
                    quote! { ::protocol_util::generic::Infallible }
                }
            });
            quote! {
                type #name<#variant> = #tx_name<#(#output_generics),*>
            }
        });
        quote! {
            #(#aliases;)*
        }
    }
}
impl Type {
    pub fn from_input(value: input::Type) -> Self {
        match value {
            input::Type::Structure(structure) => Type::Structure(Structure::from_input(structure)),
            input::Type::Enum(enumeration) => Type::Enum(Enum::from_input(enumeration)),
        }
    }
}

impl ToTokens for Structure {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.append_all(self.generate_base_struct());
        tokens.append_all(self.generate_received_struct());
        tokens.append_all(self.generate_sent_struct());
        tokens.append_all(self.generate_received_impl());
        tokens.append_all(self.generate_sent_impl());
    }
}
impl ToTokens for Enum {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.append_all(self.generate_base_struct());
        tokens.append_all(self.generate_received_struct());
        tokens.append_all(self.generate_sent_struct());
        tokens.append_all(self.generate_received_impl());
        tokens.append_all(self.generate_sent_impl());
        tokens.append_all(self.generate_sent_aliases());
    }
}
impl ToTokens for Type {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            Type::Structure(structure) => structure.to_tokens(tokens),
            Type::Enum(enumeration) => enumeration.to_tokens(tokens),
        }
    }
}