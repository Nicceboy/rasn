#[macro_use]
extern crate quote;

mod asn_type;
mod component_constraints;
mod config;
mod decode;
mod encode;
mod r#enum;
mod ext;
mod tag;

use crate::ext::GenericsExt;
use config::Config;
use syn::{DataStruct, DeriveInput};

const CRATE_NAME: &str = "rasn";
const INNER_SUBTYPE_CONSTRAINT_ATTR: &str = "inner_subtype_constraint";

pub fn decode_derive_inner(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let config = Config::from_attributes(&input)?;
    let name = &input.ident;
    let mut generics = input.generics;
    let crate_root = &config.crate_root;
    generics.add_trait_bounds(crate_root, quote::format_ident!("Decode"));

    match input.data {
        // Unit structs are treated as ASN.1 NULL values.
        syn::Data::Struct(DataStruct {
            fields: syn::Fields::Unit,
            ..
        }) => Ok(quote! {
            impl #crate_root::Decode for #name {
                fn decode_with_tag_and_constraints<D: #crate_root::Decoder>(
                    decoder: &mut D,
                    tag: #crate_root::types::Tag,
                    _: #crate_root::prelude::Constraints,
                ) -> Result<Self, D::Error> {
                    decoder.decode_null(tag).map(|_| #name)
                }
            }
        }),
        syn::Data::Struct(v) => decode::derive_struct_impl(name, generics, v, &config),
        syn::Data::Enum(syn::DataEnum { variants, .. }) => r#enum::Enum {
            name,
            generics: &generics,
            variants: &variants,
            config: &config,
        }
        .impl_decode(),
        _ => Err(syn::Error::new(
            name.span(),
            "Union types are not supported.",
        )),
    }
}

pub fn encode_derive_inner(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let config = Config::from_attributes(&input)?;
    let name = &input.ident;
    let mut generics = input.generics;
    let crate_root = &config.crate_root;
    generics.add_trait_bounds(crate_root, quote::format_ident!("Encode"));

    Ok(match input.data {
        // Unit structs are treated as ASN.1 NULL values.
        syn::Data::Struct(DataStruct {
            fields: syn::Fields::Unit,
            ..
        }) => quote! {
            impl #crate_root::Encode for #name {
                fn encode_with_tag_and_constraints<'encoder, E: #crate_root::Encoder<'encoder>>(
                    &self,
                    encoder: &mut E,
                    tag: #crate_root::types::Tag,
                    constraints: #crate_root::prelude::Constraints,
                    identifier: #crate_root::types::Identifier,
                ) -> Result<(), E::Error> {
                    encoder.encode_null(tag, identifier).map(drop)
                }
            }
        },
        syn::Data::Struct(v) => encode::derive_struct_impl(name, generics, v, &config)?,
        syn::Data::Enum(syn::DataEnum { variants, .. }) => r#enum::Enum {
            name,
            generics: &generics,
            variants: &variants,
            config: &config,
        }
        .impl_encode()?,
        _ => {
            return Err(syn::Error::new(
                name.span(),
                "Union types are not supported.",
            ))
        }
    })
}

pub fn asn_type_derive_inner(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let config = Config::from_attributes(&input)?;
    let name = &input.ident;
    let mut generics = input.generics;
    let crate_root = &config.crate_root;
    for param in &mut generics.params {
        if let syn::GenericParam::Type(type_param) = param {
            type_param
                .bounds
                .push(syn::parse_quote!(#crate_root::AsnType));
        }
    }

    Ok(match input.data {
        syn::Data::Struct(v) => asn_type::derive_struct_impl(name, generics, v, &config)?,
        syn::Data::Enum(syn::DataEnum { variants, .. }) => r#enum::Enum {
            name,
            generics: &generics,
            variants: &variants,
            config: &config,
        }
        .impl_asntype()?,
        _ => {
            return Err(syn::Error::new(
                name.span(),
                "Union types are not supported.",
            ))
        }
    })
}

pub fn inner_subtype_constraint_derive_inner(
    input: DeriveInput,
) -> syn::Result<proc_macro2::TokenStream> {
    let mut inner_subtype_constraint = None;

    for attr in input.attrs {
        if let Ok(metalist) = attr.meta.require_list() {
            if let Some(ident) = &metalist.path.get_ident() {
                if *ident == INNER_SUBTYPE_CONSTRAINT_ATTR {
                    inner_subtype_constraint = Some(metalist.tokens.to_owned());
                }
            }
        }
    }
    if let Some(tokens) = inner_subtype_constraint {
        match input.data {
            syn::Data::Struct(_) => component_constraints::derive_inner_subtype_constraint_impl(
                &input.ident,
                &input.data,
                tokens,
            ),
            _ => Err(syn::Error::new(
                input.ident.span(),
                "Only constructed types are supported for deriving InnerSubtypeConstraint.",
            )),
        }
    } else {
        Err(syn::Error::new(
            input.ident.span(),
            format!("'{}' attribute not found.", INNER_SUBTYPE_CONSTRAINT_ATTR,),
        ))
    }
}
