use crate::{config::*, ext::GenericsExt};

pub fn derive_struct_impl(
    name: syn::Ident,
    mut generics: syn::Generics,
    container: syn::DataStruct,
    config: &Config,
) -> proc_macro2::TokenStream {
    let crate_root = &config.crate_root;

    let list: Vec<_> = container
        .fields
        .iter()
        .enumerate()
        .map(|(i, field)| FieldConfig::new(field, config).encode(i, true))
        .collect();

    generics.add_trait_bounds(crate_root, quote::format_ident!("Encode"));
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let encode_impl = if config.delegate {
        let ty = &container.fields.iter().next().unwrap().ty;

        if let Some(tag) = config.tag.as_ref().filter(|tag| tag.is_explicit()) {
            let tag = tag.to_tokens(crate_root);
            let encode = quote!(encoder.encode_explicit_prefix(#tag, &self.0).map(drop));
            if config.option_type.is_option_type(ty) {
                quote! {
                    if &self.0.is_some() {
                        #encode
                    }
                }
            } else {
                encode
            }
        } else {
            // let constraint_name = format_ident!("ENCODE_DELEGATE_CONSTRAINTS");
            // let constraints = config
            //     .constraints
            //     .const_expr(crate_root)
            //     .unwrap_or_else(|| quote!(#crate_root::types::Constraints::default()));
            // print!("{:?}", config.identifier);
            // print!("{:?}", config);
            // println!("{}", quote!(#constraints));
            quote!(
                // const #constraint_name : #crate_root::types::Constraints = #crate_root::types::Constraints::from_fixed_size(&<#ty as #crate_root::AsnType>::CONSTRAINTS.merge(
                //     #constraints
                // ));
                // let (data, test) = <#ty as #crate_root::AsnType>::CONSTRAINTS.merge(
                //     constraints
                // );
                let merged  = <#ty as #crate_root::AsnType>::CONSTRAINTS.merge(
                    constraints
                );
                // dbg!(&data[..test]);
                // dbg!(&test);
                // let constraint : #crate_root::types::Constraints = #crate_root::types::Constraints::new(&data[..test]);
                // dbg!(&constraint);
                // let merged = <#ty as #crate_root::AsnType>::CONSTRAINTS.merge(constraint);
                let merged_constraints : #crate_root::types::Constraints = #crate_root::types::Constraints::from_fixed_size(&merged);
                // dbg!(constraintts);
                // dbg!(#constraint_name);
                // dbg!(&constraints);
                // dbg!(<#ty as #crate_root::AsnType>::CONSTRAINTS);
                match tag {
                    #crate_root::types::Tag::EOC => {
                        self.0.encode(encoder)
                    }
                    _ => {
                        <#ty as #crate_root::Encode>::encode_with_tag_and_constraints(
                            &self.0,
                            encoder,
                            tag,
                            // data,
                            // constraints
                            // Correct but misses override..
                            // #constraint_name
                            merged_constraints
                            // Empty
                            // <#ty as #crate_root::AsnType>::CONSTRAINTS,
                        )
                    }
                }
            )
        }
    } else {
        let operation = config
            .set
            .then(|| quote!(encode_set))
            .unwrap_or_else(|| quote!(encode_sequence));

        let encode_impl = quote! {
            encoder.#operation::<Self, _>(tag, |encoder| {
                #(#list)*

                Ok(())
            }).map(drop)
        };

        if config.tag.as_ref().map_or(false, |tag| tag.is_explicit()) {
            map_to_inner_type(
                config.tag.clone().unwrap(),
                &name,
                &container.fields,
                &generics,
                crate_root,
                true,
            )
        } else {
            encode_impl
        }
    };

    let vars = fields_as_vars(&container.fields);
    quote! {
        #[allow(clippy::mutable_key_type)]
        impl #impl_generics  #crate_root::Encode for #name #ty_generics #where_clause {
            fn encode_with_tag_and_constraints<EN: #crate_root::Encoder>(&self, encoder: &mut EN, tag: #crate_root::types::Tag, constraints: #crate_root::types::Constraints) -> core::result::Result<(), EN::Error> {
                #(#vars)*

                #encode_impl
            }
        }
    }
}

pub fn map_to_inner_type(
    tag: crate::tag::Tag,
    name: &syn::Ident,
    fields: &syn::Fields,
    generics: &syn::Generics,
    crate_root: &syn::Path,
    is_explicit: bool,
) -> proc_macro2::TokenStream {
    let inner_name = quote::format_ident!("Inner{}", name);
    let mut inner_generics = generics.clone();
    let lifetime = syn::Lifetime::new(
        &format!("'inner{}", uuid::Uuid::new_v4().as_u128()),
        proc_macro2::Span::call_site(),
    );
    inner_generics
        .params
        .push(syn::LifetimeDef::new(lifetime.clone()).into());

    let (field_defs, init_fields) = match &fields {
        syn::Fields::Named(_) => {
            let field_defs = fields.iter().map(|field| {
                let name = field.ident.as_ref().unwrap();
                let attrs = &field.attrs;
                let ty = &field.ty;
                quote!(#(#attrs)* #name : &#lifetime #ty)
            });

            let init_fields = fields.iter().map(|field| {
                let name = field.ident.as_ref().unwrap();
                let name_prefixed = format_ident!("__rasn_field_{}", name);
                quote!(#name : &#name_prefixed)
            });

            fn wrap(
                fields: impl Iterator<Item = proc_macro2::TokenStream>,
            ) -> proc_macro2::TokenStream {
                quote!({ #(#fields),* })
            }

            (wrap(field_defs), wrap(init_fields))
        }
        syn::Fields::Unnamed(_) => {
            let field_defs = fields.iter().map(|field| {
                let ty = &field.ty;
                quote!(&#lifetime #ty)
            });

            let init_fields = fields.iter().enumerate().map(|(i, _)| {
                let i = syn::Index::from(i);
                quote!(&self.#i)
            });

            fn wrap(
                fields: impl Iterator<Item = proc_macro2::TokenStream>,
            ) -> proc_macro2::TokenStream {
                quote!((#(#fields),*))
            }

            (wrap(field_defs), wrap(init_fields))
        }
        syn::Fields::Unit => (quote!(;), quote!()),
    };

    let tag = tag.to_tokens(crate_root);
    let inner_impl = if is_explicit {
        quote!(encoder.encode_explicit_prefix(#tag, &inner).map(drop))
    } else {
        quote!(inner.encode_with_tag(encoder, #tag))
    };

    quote! {
        #[derive(#crate_root::AsnType, #crate_root::Encode)]
        struct #inner_name #inner_generics #field_defs

        let inner = #inner_name #init_fields;

        #inner_impl
    }
}

fn fields_as_vars(fields: &syn::Fields) -> impl Iterator<Item = proc_macro2::TokenStream> + '_ {
    fields.iter().enumerate().map(|(i, field)| {
        let self_name = field
            .ident
            .as_ref()
            .map(|ident| quote!(#ident))
            .unwrap_or_else(|| {
                let i = syn::Index::from(i);
                quote!(#i)
            });

        let name = field
            .ident
            .as_ref()
            .map(|ident| {
                let prefixed = format_ident!("__rasn_field_{}", ident);
                quote!(#prefixed)
            })
            .unwrap_or_else(|| {
                let ident = format_ident!("i{}", i);
                quote!(#ident)
            });

        quote!(#[allow(unused)] let #name = &self.#self_name;)
    })
}
