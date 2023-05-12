use crate::input;
use proc_macro2::{TokenStream, Span};
use quote::{quote, TokenStreamExt, ToTokens, format_ident};
use convert_case::{Casing, Case};
use syn::{token::Struct, punctuated::Punctuated, WherePredicate};

#[derive(Debug)]
pub struct Structure {
    pub generics: syn::Generics,
    pub name: syn::Ident,
    pub rx_name: syn::Ident,
    pub tx_name: syn::Ident,
    pub members: Vec<(syn::Ident, syn::Type)>,
}

#[derive(Debug)]
pub struct Enum {
    pub generics: syn::Generics,
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
            generics: value.generics,
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
        let generics = &self.generics;
        let where_clause = &generics.where_clause;
        quote! {
            #[derive(::serde::Serialize, ::serde::Deserialize)]
            pub struct #name #generics #where_clause {
                #(#members),*
            }
        }
    }
    fn received_where_predicates(&self) -> Vec<WherePredicate> {
        self.members.iter().map(|(_, ty)| {
            syn::parse2(quote! { #ty: ::protocol_util::generic::Receivable }).unwrap()
        }).collect()
    }
    fn generate_received_struct(&self) -> TokenStream {
        let name = &self.rx_name;
        let members = self.members.iter().map(|(name, ty)| {
            quote! {
                pub #name: <#ty as ::protocol_util::generic::Receivable>::ReceivedAs
            }
        });
        let mut generics = self.generics.clone();
        let mut where_clause = generics.make_where_clause().clone();
        where_clause.predicates.extend(self.received_where_predicates());
        quote! {
            pub struct #name #generics #where_clause {
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
        let (impl_generics, ty_generics, _) = self.generics.split_for_impl();
        let mut where_clause = self.generics.clone().make_where_clause().clone();
        where_clause.predicates.extend(self.received_where_predicates());
        where_clause.predicates.push(syn::parse2(quote! {
            #name #ty_generics: ::serde::de::DeserializeOwned
        }).unwrap());
        quote! {
            impl #impl_generics ::protocol_util::generic::Receivable for #name #ty_generics #where_clause {
                type ReceivedAs = #rx_name #ty_generics;

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
        let mut members = Vec::new();
        let mut sent_generics = syn::Generics {
            lt_token: None,
            params: Punctuated::new(),
            gt_token: None,
            where_clause: None,
        };
        for (name, _) in &self.members {
            let type_name = syn::Ident::new(&name.to_string().from_case(Case::Snake).to_case(Case::UpperCamel), Span::call_site());
            members.push(quote!{
                pub #name: #type_name
            });
            sent_generics.params.push(syn::GenericParam::Type(syn::TypeParam {
                attrs: Vec::new(),
                ident: type_name,
                colon_token: None,
                bounds: Punctuated::new(),
                eq_token: None,
                default: None,
            }));
        }
        quote! {
            pub struct #name #sent_generics {
                #(#members),*
            }
        }
    }
    fn generate_sent_impl(&self) -> TokenStream {
        let name = &self.name;
        let tx_name = &self.tx_name;
        let mut members = Vec::new();
        let (_, type_generics, _) = self.generics.split_for_impl();
        let mut full_generics = self.generics.clone();
        let mut sent_generics = syn::Generics {
            lt_token: None,
            params: Punctuated::new(),
            gt_token: None,
            where_clause: None,
        };
        let mut where_clause = self.generics.clone().make_where_clause().clone();
        for (name, ty) in &self.members {
            let type_name = syn::Ident::new(&name.to_string().from_case(Case::Snake).to_case(Case::UpperCamel), Span::call_site());
            full_generics.params.push(syn::parse2(quote! {
                #type_name: ::protocol_util::generic::SendableAs<#ty>
            }).unwrap());
            sent_generics.params.push(syn::GenericParam::Type(syn::TypeParam {
                attrs: Vec::new(),
                ident: type_name,
                colon_token: None,
                bounds: Punctuated::new(),
                eq_token: None,
                default: None,
            }));
            members.push(quote! {
                #name: self.#name.prepare_in_context(context)
            });
            where_clause.predicates.push(syn::parse2(quote! {
                #ty: ::serde::Serialize
            }).unwrap());
        }
        where_clause.predicates.push(syn::parse2(quote! {
            #name #type_generics: ::serde::Serialize
        }).unwrap());
        quote! {
            impl #full_generics ::protocol_util::generic::SendableAs<#name #type_generics> for #tx_name #sent_generics #where_clause {
                fn prepare_in_context(self, context: &::protocol_util::communication_context::DeferingContext) -> #name #type_generics {
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
            generics: value.generics,
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
        let generics = &self.generics;
        let where_clause = &generics.where_clause;
        quote! {
            #[derive(::serde::Serialize, ::serde::Deserialize)]
            pub enum #name #generics #where_clause {
                #(#variants),*
            }
        }
    }
    fn received_where_predicates(&self) -> Vec<WherePredicate> {
        self.variants.iter().map(|(_, ty)| {
            syn::parse2(quote! { #ty: ::protocol_util::generic::Receivable }).unwrap()
        }).collect()
    }
    fn generate_received_struct(&self) -> TokenStream {
        let name = &self.rx_name;
        let variants = self.variants.iter().map(|(variant, ty)| {
            quote! {
                #variant(<#ty as ::protocol_util::generic::Receivable>::ReceivedAs)
            }
        });
        let mut generics = self.generics.clone();
        let mut where_clause = generics.make_where_clause().clone();
        where_clause.predicates.extend(self.received_where_predicates());
        quote! {
            pub enum #name #generics #where_clause {
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
        let (impl_generics, ty_generics, _) = self.generics.split_for_impl();
        let mut where_clause = self.generics.clone().make_where_clause().clone();
        where_clause.predicates.extend(self.received_where_predicates());
        where_clause.predicates.push(syn::parse2(quote! {
            #name #ty_generics: ::serde::de::DeserializeOwned
        }).unwrap());
        quote! {
            impl #impl_generics ::protocol_util::generic::Receivable for #name #ty_generics #where_clause {
                type ReceivedAs = #rx_name #ty_generics;

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
        let mut variants = Vec::new();
        let mut sent_generics = syn::Generics {
            lt_token: None,
            params: Punctuated::new(),
            gt_token: None,
            where_clause: None,
        };
        for (variant, _) in &self.variants {
            let type_name = syn::Ident::new(&variant.to_string(), Span::call_site());
            variants.push(quote!{
                #variant(#type_name)
            });
            sent_generics.params.push(syn::GenericParam::Type(syn::TypeParam {
                attrs: Vec::new(),
                ident: type_name,
                colon_token: None,
                bounds: Punctuated::new(),
                eq_token: None,
                default: None,
            }));
        }
        quote! {
            pub enum #name #sent_generics {
                #(#variants),*
            }
        }
    }
    fn generate_sent_impl(&self) -> TokenStream {
        let name = &self.name;
        let tx_name = &self.tx_name;
        let mut variants = Vec::new();
        let (_, type_generics, _) = self.generics.split_for_impl();
        let mut full_generics = self.generics.clone();
        let mut sent_generics = syn::Generics {
            lt_token: None,
            params: Punctuated::new(),
            gt_token: None,
            where_clause: None,
        };
        let mut where_clause = self.generics.clone().make_where_clause().clone();
        for (variant, ty) in &self.variants {
            let type_name = syn::Ident::new(&variant.to_string(), Span::call_site());
            full_generics.params.push(syn::parse2(quote! {
                #type_name: ::protocol_util::generic::SendableAs<#ty>
            }).unwrap());
            sent_generics.params.push(syn::GenericParam::Type(syn::TypeParam {
                attrs: Vec::new(),
                ident: type_name,
                colon_token: None,
                bounds: Punctuated::new(),
                eq_token: None,
                default: None,
            }));
            variants.push(quote! {
                #tx_name::#variant(value) => #name::#variant(value.prepare_in_context(context))
            });
            where_clause.predicates.push(syn::parse2(quote! {
                #ty: ::serde::Serialize
            }).unwrap());
        }
        where_clause.predicates.push(syn::parse2(quote! {
            #name #type_generics: ::serde::Serialize
        }).unwrap());
        quote! {
            impl #full_generics ::protocol_util::generic::SendableAs<#name #type_generics> for #tx_name #sent_generics #where_clause {
                fn prepare_in_context(self, context: &::protocol_util::communication_context::DeferingContext) -> #name #type_generics {
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