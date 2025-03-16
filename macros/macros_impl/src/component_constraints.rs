// allow dead code, never read for whole file
#![allow(unused_imports)]
#![allow(dead_code)]
use std::ops::Deref;

use crate::config::Constraints;
use quote::ToTokens;
use syn::parse::Parser;
use syn::Index;
use syn::{
    braced, parenthesized,
    parse::{Parse, ParseStream},
    Data, DataStruct, Error, Expr, Fields, Ident, LitStr, Path, Result, Token,
};

/// Converts a ASN.1 CamelCase string into snake_case.
/// Used for the struct field names.
pub fn field_ident_to_snake_case(ident: &Ident) -> Ident {
    // Special case for Rust keyword 'type'
    if ident == "r#type" {
        return ident.clone();
    }
    let orig = ident.to_string();
    let mut snake = String::with_capacity(orig.len());
    for (i, ch) in orig.chars().enumerate() {
        if ch.is_uppercase() {
            if i != 0 {
                snake.push('_');
            }
            for lower in ch.to_lowercase() {
                snake.push(lower);
            }
        } else {
            snake.push(ch);
        }
    }
    Ident::new(&snake, ident.span())
}
/// In ASN.1, if the first letter of a choice/enum variant is lowercase, it should be uppercase.
fn enum_variant_to_upper_intial(ident: &Ident) -> Ident {
    let orig = ident.to_string();
    let mut upper = String::with_capacity(orig.len());
    for (i, ch) in orig.chars().enumerate() {
        if i == 0 {
            for uppercase_ch in ch.to_uppercase() {
                upper.push(uppercase_ch);
            }
        } else {
            upper.push(ch);
        }
    }
    Ident::new(&upper, ident.span())
}

/// An AST node for a logical expression of constraints.
#[derive(Debug)]
enum ConstraintExpr {
    /// A single named block or a group of blocks.
    Leaf(Box<NamedBlock>),
    /// A grouping of blocks (for cases where you have a block with nested sub-blocks).
    Block(BlockContents),
    /// Logical AND between two or more constraint expressions.
    And(Vec<ConstraintExpr>),
    /// Logical OR between two or more constraint expressions.
    Or(Vec<ConstraintExpr>),
}

impl ConstraintExpr {
    fn quote(
        &self,
        base_type_ident: &Ident,
        type_ident: &Ident,
        field_access: &proc_macro2::TokenStream,
        field_var: &Ident,
        err_instead_of_false: bool,
    ) -> proc_macro2::TokenStream {
        match self {
            ConstraintExpr::Leaf(named_block) => named_block.value.quote(
                type_ident,
                &named_block.field_name,
                field_access,
                field_var,
                err_instead_of_false,
            ),
            ConstraintExpr::And(exprs) => {
                // For AND, all constraints must be met.
                let checks = exprs.iter().map(|e| {
                    // For leaf nodes, combine the parent field access with the leaf name
                    match e {
                        ConstraintExpr::Leaf(named_block) => {
                            let field_basename = field_ident_to_snake_case(&named_block.field_name);
                            let combined_field_access =
                                quote::quote!(#field_access.#field_basename);
                            e.quote(
                                base_type_ident,
                                type_ident,
                                &combined_field_access,
                                field_var,
                                true,
                            )
                        }
                        _ => e.quote(base_type_ident, type_ident, field_access, field_var, true),
                    }
                });
                quote! {
                    {
                        // We can just chain the checks - first fail we raise an error
                        #(#checks)*
                    }
                }
            }
            ConstraintExpr::Or(exprs) => {
                // For OR, at least one must pass.
                let mut tokens = proc_macro2::TokenStream::new();
                let mut constraint_vars = Vec::with_capacity(exprs.len());
                for (i, expr) in exprs.iter().enumerate() {
                    // Extract field name if it's a leaf, or use the original field identifier
                    let field_basename = match expr {
                        ConstraintExpr::Leaf(inner) => field_ident_to_snake_case(&inner.field_name),
                        _ => type_ident.clone(),
                    };
                    let field_access = match expr {
                        ConstraintExpr::Leaf(_) => quote::quote!(#field_access.#field_basename),
                        _ => field_access.clone(),
                    };
                    let constr_identifier =
                        format_ident!("__constraint_{}_ok", &field_basename.to_string());
                    // Expression returns boolean instead of error if fails, so we can combined the OR check later on
                    let check = expr.quote(
                        base_type_ident,
                        type_ident,
                        &field_access,
                        &constr_identifier,
                        false,
                    );
                    constraint_vars.push(constr_identifier);
                    tokens.extend(quote! {
                        #check
                    });
                    if i == exprs.len() - 1 {
                        // Check that at least one constraint was satisfied
                        tokens.extend(quote! {
                            if !(#(#constraint_vars) || *) {
                                return Err(InnerSubtypeConstraintError::InvalidCombination {
                                    type_name: stringify!(#type_ident),
                                    details: concat!(
                                        "At least one of the inner subtype constraints must be met for ",
                                        stringify!(#type_ident)
                                    ),
                                });
                            }
                        });
                    }
                }
                tokens
            }
            ConstraintExpr::Block(block) => {
                let block_contents = &block.blocks;
                let mut tokens = proc_macro2::TokenStream::new();
                // let block_type_ident = &block.block_type;
                for block in block_contents {
                    let constraint_indent = format_ident!("__constraint_{}_ok", &block.field_name);
                    let field_name = &block.field_name;
                    let field_access = quote! { #field_access.#field_name };
                    tokens.extend(block.value.quote(
                        type_ident,
                        &block.field_name,
                        &field_access,
                        &constraint_indent,
                        true,
                    ));
                }
                dbg!(&tokens.to_string());
                tokens

                // quote! {
                //     if !matches!(&#field_access, #base_type_ident {
                //         #(#block_contents),*,
                //         ..
                //     }) {
                //         return Err(InnerSubtypeConstraintError::InvalidCombination {
                //             type_name: stringify!(#type_ident),
                //             details: concat!(
                //                 "Invalid component combination in ",
                //                 stringify!(#type_ident),
                //                 " which is alias for ",
                //                 stringify!(#base_type_ident)
                //             ),
                //         });
                //     }
                // }
            }
        }
    }
}

impl Parse for ConstraintExpr {
    fn parse(input: ParseStream) -> Result<Self> {
        if input.peek(syn::token::Paren) {
            // Logic for a parenthesized group (allowing explicit or/and operators)
            let mut expr = parse_single_expr(input)?;
            while input.peek(Ident) {
                let op: Ident = input.parse()?;
                let op_str = op.to_string();
                if op_str != "or" && op_str != "and" {
                    return Err(syn::Error::new(op.span(), "Expected 'or' or 'and'"));
                }
                let next_expr = parse_single_expr(input)?;
                expr = match op_str.as_str() {
                    "or" => match expr {
                        ConstraintExpr::Or(mut vec) => {
                            vec.push(next_expr);
                            ConstraintExpr::Or(vec)
                        }
                        _ => ConstraintExpr::Or(vec![expr, next_expr]),
                    },
                    "and" => match expr {
                        ConstraintExpr::And(mut vec) => {
                            vec.push(next_expr);
                            ConstraintExpr::And(vec)
                        }
                        _ => ConstraintExpr::And(vec![expr, next_expr]),
                    },
                    _ => unreachable!(),
                }
            }
            Ok(expr)
        } else if input.peek(Ident) {
            // Parse a comma-separated list of NamedBlock items.
            let blocks = input.parse_terminated(NamedBlock::parse, Token![,])?;
            let vec = blocks.into_iter().collect::<Vec<_>>();
            if vec.len() == 1 {
                Ok(ConstraintExpr::Leaf(vec.into_iter().next().unwrap().into()))
            } else {
                // Root level block - no type defition
                let block_type = ConstrainedType(TypeVariant::Constructed((
                    Ident::new("root", proc_macro2::Span::call_site()),
                    None,
                )));
                Ok(ConstraintExpr::Block(BlockContents {
                    block_type,
                    blocks: vec,
                }))
            }
        } else {
            Err(syn::Error::new(
                input.span(),
                "Expected a parenthesized group or a comma-separated list of constraints",
            ))
        }
    }
}

/// Represents field name and its constraint.
#[derive(Debug)]
struct NamedBlock {
    field_name: Ident,
    value: ConstraintValue,
}
impl Parse for NamedBlock {
    fn parse(input: ParseStream) -> Result<Self> {
        let name: Ident = input.parse()?;
        let _: Token![=>] = input.parse()?;
        let value: ConstraintValue = input.parse()?;
        Ok(Self {
            field_name: field_ident_to_snake_case(&name),
            value,
        })
    }
}
impl ToTokens for NamedBlock {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self.value {
            ConstraintValue::Block(ref block) => {
                let name = &self.field_name;
                let block_type = &block.block_type;
                let block = &block.blocks;
                match block_type {
                    ConstrainedType(TypeVariant::Constructed(_)) => {
                        tokens.extend(quote! {
                            #name: #block_type { #(#block),* , ..}
                        });
                    }
                    ConstrainedType(TypeVariant::Choice((typedef, None))) => {
                        // dbg!(block);
                        // tokens.extend(quote! {
                        //     HMM
                        // });
                        panic!("Choice without variant not yet implemented");
                    }
                    _ => {
                        tokens.extend(quote! {
                            #name: #block_type { #(#block),* , ..}
                        });
                    }
                }
                // tokens.extend(quote! {
                //     #name: #block_type { #(#block),* , ..}
                // });
            }
            ConstraintValue::ConstrainedType(ref ty) => {
                let name = &self.field_name;
                let ty = &ty.0;
                tokens.extend(quote! {
                    #name: #ty
                });
            }
            ConstraintValue::Primitive(ref _constraints) => {
                todo!("Value/size/attribute constraints not yet implemented");
            }
            // usable only in constructed context
            ConstraintValue::Present => {
                let name = &self.field_name;
                tokens.extend(quote! {
                    #name: Some(_)
                });
            }
            // usable only in constructed context
            ConstraintValue::Absent => {
                let name = &self.field_name;
                tokens.extend(quote! {
                    #name:  None
                });
            }
        }
    }
}

/// Possible things that can appear after `=>`:
#[derive(Debug)]
enum ConstraintValue {
    /// A block with more inner type constraints.
    Block(BlockContents),
    /// For something like `constructed(MyType)` or choice(Type::Variant) #  Subtype restriction notation
    ConstrainedType(ConstrainedType),
    /// For size/value/pattern constraint values
    Primitive(Constraints),
    Present,
    Absent,
}

impl ConstraintValue {
    // Generate checks if there is only one constraint, not combined with `and` or `or`.
    fn quote(
        &self,
        type_ident: &Ident,
        field_ident: &Ident,
        field_access: &proc_macro2::TokenStream,
        field_constrain_var: &Ident,
        err_instead_of_false: bool,
    ) -> proc_macro2::TokenStream {
        match self {
            ConstraintValue::Present => {
                let error_or_false = if err_instead_of_false {
                    quote! {
                        return Err(InnerSubtypeConstraintError::MissingRequiredComponent  {
                            type_name: stringify!(#type_ident),
                            components: &[stringify!(#field_ident)],
                        });
                    }
                } else {
                    quote! { false }
                };
                quote! {
                    let #field_constrain_var = if #field_access.is_some() {
                        true
                    } else { #error_or_false };
                }
            }
            ConstraintValue::Absent => {
                let error_or_false = if err_instead_of_false {
                    quote! {
                        return Err(InnerSubtypeConstraintError::UnexpectedComponentPresent {
                            type_name: stringify!(#type_ident),
                            component_name: stringify!(#field_ident),
                        });
                    }
                } else {
                    quote! { false }
                };
                quote! {
                    let #field_constrain_var = if #field_access.is_none() {
                        true
                    } else { #error_or_false };
                }
            }
            ConstraintValue::Block(block) => {
                match &block.block_type {
                    ConstrainedType(TypeVariant::Constructed(_)) => {
                        let block_contents = &block.blocks;
                        let block_type = &block.block_type;
                        let mut tokens = proc_macro2::TokenStream::new();
                        let mut constraints = Vec::with_capacity(block_contents.len());
                        for block in block_contents {
                            let constraint_indent =
                                format_ident!("__constraint_{}_ok", &block.field_name);
                            let field_name = &block.field_name;
                            let field_access = quote! { #field_access.#field_name };
                            tokens.extend(block.value.quote(
                                type_ident,
                                &block.field_name,
                                &field_access,
                                &constraint_indent,
                                true,
                            ));
                            constraints.push(constraint_indent);
                        }
                        // and for all constraints if true, if not raise error
                        tokens.extend(quote! {
                            let #field_constrain_var = if #(#constraints) && * {
                                true
                            } else {
                                return Err(InnerSubtypeConstraintError::InvalidCombination {
                                    type_name: stringify!(#type_ident),
                                    details: concat!(
                                        "Invalid component combination in ",
                                        stringify!(#type_ident),
                                        " in inner block ",
                                        stringify!(#block_type)
                                    ),
                                });
                            };
                        });
                        tokens
                    }
                    ConstrainedType(TypeVariant::Choice((typedef, None))) => {
                        let block_contents = &block.blocks;
                        let block_type = &block.block_type;
                        let mut tokens = proc_macro2::TokenStream::new();
                        let mut constraints = Vec::with_capacity(block_contents.len());
                        // generate code where we check whether variant is present or not
                        for block in block_contents {
                            let constraint_indent =
                                format_ident!("__constraint_{}_ok", &block.field_name);
                            let field_name = &block.field_name;
                            let field_access = quote! { #field_access.#field_name };
                            tokens.extend(block.value.quote(
                                type_ident,
                                &block.field_name,
                                &field_access,
                                &constraint_indent,
                                true,
                            ));
                            constraints.push(constraint_indent);
                        }
                        tokens
                    }
                    _ => {
                        todo!("Enum check");
                    }
                }
            }
            _ => {
                println!("ERROR????");
                quote! {}
            }
        }
    }
}
impl Parse for ConstraintValue {
    fn parse(input: ParseStream) -> Result<Self> {
        if input.peek(Ident) {
            let ident_lookahead = input.lookahead1();
            if ident_lookahead.peek(Ident) {
                let fork = input.fork();
                let next_ident: Ident = fork.parse()?;
                let ident_str = next_ident.to_string();
                if ident_str == "present" {
                    let _: Ident = input.parse()?; // Consume "present"
                    Ok(ConstraintValue::Present)
                } else if ident_str == "absent" {
                    let _: Ident = input.parse()?; // Consume "absent"
                    Ok(ConstraintValue::Absent)
                } else {
                    let fork = input.fork();
                    let possible_block: ConstrainedType = fork.parse()?;
                    // No variant - likely a block type definition
                    if possible_block.get_variant().is_none() {
                        Ok(ConstraintValue::Block(BlockContents::parse(input)?))
                    } else {
                        let constrained_type: ConstrainedType = input.parse()?;
                        Ok(ConstraintValue::ConstrainedType(constrained_type))
                    }
                }
            } else {
                Err(Error::new(
                    input.span(),
                    "Expected a block with a field name, e.g. `tbsData => { ... }` or component constraint, e.g. `(MyType, size(1))",
                ))
            }
        } else {
            Err(Error::new(
                input.span(),
                "Expected a block with a field name, e.g. `tbsData => { ... }` or component constraint, e.g. `(MyType, size(1))`",
            ))
        }
    }
}
#[derive(Debug)]
struct ConstrainedType(TypeVariant);

impl Deref for ConstrainedType {
    type Target = TypeVariant;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl ToTokens for ConstrainedType {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.0.to_tokens(tokens);
    }
}

impl Parse for ConstrainedType {
    fn parse(input: ParseStream) -> Result<Self> {
        if input.peek(Ident) {
            let fork = input.fork();
            let kind: Ident = fork.parse()?;
            if !TypeVariant::variant_names().contains(&kind.to_string().as_str()) {
                return Err(Error::new(
                    kind.span(),
                    "Expected a type variant, e.g. choice, newtype, enum",
                ));
            }
            let base = TypeVariant::parse(input)?;

            Ok(ConstrainedType(base))
        } else {
            Err(Error::new(
                input.span(),
                "Expected a type variant, e.g. choice, newtype, enum",
            ))
        }
        // TODO primitive constraints
        // let content;
        // parenthesized!(content in input);
        // let ty: Path = content.parse()?;
        // if content.peek(Token![,]) {
        //     let _: Token![,] = content.parse()?;
        //     let size: Expr = content.parse()?;
        //     match size {
        //         // looking for e.g. size(1)
        //         Expr::Call(call) => {
        //             if let Expr::Path(path) = &*call.func {
        //                 if path.path.is_ident("size") {
        //                     let size = match call.args.first() {
        //                         Some(Expr::Lit(lit)) => {
        //                             if let syn::Lit::Int(int) = &lit.lit {
        //                                 Some(int.base10_parse::<usize>().unwrap())
        //                             } else {
        //                                 None
        //                             }
        //                         }
        //                         _ => None,
        //                     };
        //                     return Err(Error::new(content.span(), "Invalid With Component constraint. Currently only type and size are supported."))
        //                         ;
        //                     // return Ok(ConstrainedType {
        //                     //     ty,
        //                     //     size_constraint: size,
        //                     // });
        //                 }
        //             }
        //             return Err(
        //                 Error::new(content.span(), "Invalid With Component constraint. Currently only type and size are supported.")
        //             );

        //         }
        //         _ => return Err(
        //             Error::new(content.span(), "Invalid With Component constraint. Currently only type and size are supported.")
        //         ),
        //     }
        // }
        // Err(Error::new(
        //     content.span(),
        //     "Invalid With Component constraint. Currently only type and size are supported.",
        // ))
        // // Ok(ConstrainedType {
        // //     ty,
        //     size_constraint: None,
        // })
    }
}

/// When we see `{ ... }`, we parse into `BlockContents`.
#[derive(Debug)]
struct BlockContents {
    /// Type of the block {} area
    block_type: ConstrainedType,
    /// Comma-separated sub-blocks: `tbsData => { ... }, signer => present, ...`
    blocks: Vec<NamedBlock>,
}

impl Parse for BlockContents {
    fn parse(input: ParseStream) -> Result<Self> {
        let block_type: ConstrainedType = input.parse()?;
        let content;
        braced!(content in input);
        let blocks = content
            .parse_terminated(NamedBlock::parse, Token![,])?
            .into_iter()
            .collect();
        Ok(BlockContents { block_type, blocks })
    }
}

/// Possible type variants that component constraint can have implicitly
/// Usually applies on `CHOICE`, `ENUMERATED`, `SEQUENCE`, `SET`, `SEQUENCE OF`, `SET OF`
/// Contains the base type name and the chosen variant.
#[derive(Debug)]
enum TypeVariant {
    Choice((Ident, Option<Ident>)),
    Constructed((Ident, Option<Ident>)),
    Enumerated((Ident, Option<Ident>)),
}

impl TypeVariant {
    fn from_str(kind: Ident, typedef: Ident, variant: Option<Ident>) -> Self {
        match kind.to_string().as_str() {
            "choice" => {
                TypeVariant::Choice((typedef, variant.as_ref().map(enum_variant_to_upper_intial)))
            }
            "constructed" => TypeVariant::Constructed((typedef, variant)),
            "enumerated" => TypeVariant::Enumerated((
                typedef,
                variant.as_ref().map(enum_variant_to_upper_intial),
            )),
            _ => {
                let span = typedef.span();
                TypeVariant::Constructed((Ident::new("", span), None))
            }
        }
    }

    fn get_type_name(&self) -> &Ident {
        match self {
            TypeVariant::Choice((left, _))
            | TypeVariant::Constructed((left, _))
            | TypeVariant::Enumerated((left, _)) => left,
        }
    }

    fn get_variant(&self) -> &Option<Ident> {
        match self {
            TypeVariant::Choice((_, right))
            | TypeVariant::Constructed((_, right))
            | TypeVariant::Enumerated((_, right)) => right,
        }
    }
    fn variant_names() -> &'static [&'static str] {
        &["choice", "constructed", "enumerated"]
    }
}

impl Parse for TypeVariant {
    fn parse(input: ParseStream) -> Result<Self> {
        let kind_str: Ident = input.parse()?;
        let type_def;
        parenthesized!(type_def in input);
        let parts = type_def.parse_terminated(syn::Ident::parse, Token![::])?;
        if parts.is_empty() || parts.len() > 2 {
            return Err(Error::new(
                input.span(),
                "Expected format TypeName::Variant or TypeName and nothing else.",
            ));
        }
        if parts.len() == 1 {
            Ok(TypeVariant::from_str(kind_str, parts[0].clone(), None))
        } else {
            Ok(TypeVariant::from_str(
                kind_str,
                parts[0].clone(),
                Some(parts[1].clone()),
            ))
        }
    }
}
impl ToTokens for TypeVariant {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            TypeVariant::Choice((left, right)) => {
                tokens.extend(quote! {
                    #left::#right(_)
                });
            }
            TypeVariant::Constructed((left, right)) => {
                let kind = if let Some(ref r) = right { r } else { left };
                tokens.extend(quote! {
                    #kind
                });
            }
            TypeVariant::Enumerated((left, right)) => {
                tokens.extend(quote! {
                    #left::#right
                });
            }
        }
    }
}

/// Attempt to parse parenthesized group, with ident and =>
fn parse_single_expr(input: ParseStream) -> Result<ConstraintExpr> {
    // Expect a parenthesized group, just with single expression for non-block constraint values.
    let content;
    parenthesized!(content in input);
    let nb: NamedBlock = content.parse()?;
    Ok(ConstraintExpr::Leaf(nb.into()))
}

fn parse_dsl(input: proc_macro2::TokenStream) -> Result<BlockContents> {
    let parser = <BlockContents as syn::parse::Parse>::parse;
    parser.parse2(input)
}

fn insert_to_validate_components_skeleton(
    ident: &Ident,
    checks: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    quote! {
        impl InnerSubtypeConstraint for #ident {
            fn validate_components(self) -> Result<Self, InnerSubtypeConstraintError> {
                #checks
                Ok(self)
            }
        }
    }
}

/// Derive constraint checks for structured types with componenet constraints
pub(crate) fn derive_inner_subtype_constraint_impl(
    ident: &Ident,
    data: &Data,
    dsl: proc_macro2::TokenStream,
) -> Result<proc_macro2::TokenStream> {
    let inner_type = match data {
        Data::Struct(DataStruct {
            fields: Fields::Named(_),
            ..
        }) => {
            // For named field structs, we don't need to extract the inner type
            None
        }
        Data::Struct(DataStruct {
            fields: Fields::Unnamed(unnamed_fields),
            ..
        }) => {
            // For tuple structs, check if it's a newtype pattern (single field)
            if unnamed_fields.unnamed.len() == 1 {
                let field = unnamed_fields.unnamed.first().unwrap();
                if let syn::Type::Path(type_path) = &field.ty {
                    if type_path.qself.is_none() && !type_path.path.segments.is_empty() {
                        // Get the first (and usually only) segment's ident
                        Some(type_path.path.segments.first().unwrap().ident.clone())
                    } else {
                        return Err(Error::new(
                            ident.span(),
                            "Expected a simple type name for the newtype field.",
                        ));
                    }
                } else {
                    return Err(Error::new(
                        ident.span(),
                        "Expected a path type for the newtype field.",
                    ));
                }
            } else {
                return Err(Error::new(
                    ident.span(),
                    "Expected a newtype struct with a single unnamed field.",
                ));
            }
        }
        _ => {
            return Err(Error::new(
                ident.span(),
                "Expected a standard struct with named fields or a newtype struct.",
            ))
        }
    };

    let parser = <ConstraintExpr as syn::parse::Parse>::parse;
    let parsed = parser.parse2(dsl)?;
    let constraint_ident = format_ident!("__constraint_ok");
    let default_field_access = if inner_type.is_some() {
        quote! { self.0 }
    } else {
        quote! { self }
    };

    // Use the inner type if available, otherwise use the struct's own ident
    let base_type = if let Some(ref ty) = inner_type {
        ty
    } else {
        ident
    };

    let checks = parsed.quote(
        base_type,
        ident,
        &default_field_access,
        &constraint_ident,
        true,
    );

    Ok(insert_to_validate_components_skeleton(ident, checks))
}

#[cfg(test)]
mod tests {

    use super::*;
    use insta::assert_snapshot;
    use syn::File;

    #[test]
    fn test_basic_or_constraint_dsl() {
        // #[inner_subtype_constraint(
        //     (data => present) or (extDataHash => present) or (omitted => present)
        // )]
        let or_dsl = quote! {
            (data => present) or
            (extDataHash => present) or
            (omitted => present)
        };
        let parser = <ConstraintExpr as syn::parse::Parse>::parse;
        let parsed = parser.parse2(or_dsl).unwrap();
        let constraint_ident = format_ident!("__constraint_ok");
        let default_field_access = quote! { self };
        let ident = format_ident!("TestStruct");
        let base_type = format_ident!("BaseTestStruct");
        let checks = parsed.quote(
            &base_type,
            &ident,
            &default_field_access,
            &constraint_ident,
            true,
        );
        let complete = insert_to_validate_components_skeleton(&ident, checks);
        let syntax_tree: File = syn::parse2(complete).unwrap();
        let formatted = prettyplease::unparse(&syntax_tree);
        insta::assert_snapshot!(formatted);
    }
    #[test]
    fn test_basic_and_constraint_dsl() {
        // #[inner_subtype_constraint(
        //     (data => present) and (extDataHash => present) and (omitted => present)
        // )]
        let and_dsl = quote! {
            (data => present) and
            (extDataHash => present) and
            (omitted => present)
        };
        let parser = <ConstraintExpr as syn::parse::Parse>::parse;
        let parsed = parser.parse2(and_dsl).unwrap();
        let constraint_ident = format_ident!("__constraint_ok");
        let default_field_access = quote! { self };
        let ident = format_ident!("TestStruct");
        let base_type = format_ident!("BaseTestStruct");
        let checks = parsed.quote(
            &base_type,
            &ident,
            &default_field_access,
            &constraint_ident,
            true,
        );
        let complete = insert_to_validate_components_skeleton(&ident, checks);
        let syntax_tree: File = syn::parse2(complete).unwrap();
        let formatted = prettyplease::unparse(&syntax_tree);
        insta::assert_snapshot!(formatted);
    }
    #[test]
    fn test_nested_with_choice_dsl() {
        // /// ImplicitCertificate ::= CertificateBase (WITH COMPONENTS {...,
        //   type(implicit),
        //   toBeSigned(WITH COMPONENTS {...,
        //     verifyKeyIndicator(WITH COMPONENTS {reconstructionValue})
        //   }),
        //   signature ABSENT
        // })
        let dsl = quote! {
            r#type => enumerated(CertificateType::implicit),
            toBeSigned => constructed(ToBeSignedCertificate) {
                verifyKeyIndicator => choice(VerificationKeyIndicator::reconstructionValue)
            },
            signature => absent
        };
        let parser = <ConstraintExpr as syn::parse::Parse>::parse;
        let parsed = parser.parse2(dsl).unwrap();
        let constraint_ident = format_ident!("__constraint_ok");
        let default_field_access = quote! { self.0 };
        let ident = format_ident!("ImplicitCertificate");
        let base_type = format_ident!("CertificateBase");
        let checks = parsed.quote(
            &base_type,
            &ident,
            &default_field_access,
            &constraint_ident,
            true,
        );
        let complete = insert_to_validate_components_skeleton(&ident, checks);
        let syntax_tree: File = syn::parse2(complete).unwrap();
        let formatted = prettyplease::unparse(&syntax_tree);
        assert_snapshot!(formatted);
    }
    #[test]
    fn test_choice_with_excluded_variants() {
        let dsl = quote! {
            toBeSigned => constructed(ToBeSignedCertificate) {
                id => choice(CertificateId) {
                    linkageData => absent,
                    binaryId => absent,
                },
            },
            certRequestPermissions => absent,
            canRequestRollover => absent
        };
        let parser = <ConstraintExpr as syn::parse::Parse>::parse;
        let parsed = parser.parse2(dsl).unwrap();
        let constraint_ident = format_ident!("__constraint_ok");
        let default_field_access = quote! { self.0 };
        let ident = format_ident!("EtsiCertificate");
        let base_type = format_ident!("Certificate");
        let checks = parsed.quote(
            &base_type,
            &ident,
            &default_field_access,
            &constraint_ident,
            true,
        );
        let complete = insert_to_validate_components_skeleton(&ident, checks);
        let syntax_tree: File = syn::parse2(complete).unwrap();
        let formatted = prettyplease::unparse(&syntax_tree);
        println!("{}", formatted);
    }
}
