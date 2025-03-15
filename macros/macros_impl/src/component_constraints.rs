// allow dead code, never read for whole file
#![allow(unused_imports)]
#![allow(dead_code)]
use syn::parse::Parser;
use syn::{
    braced, parenthesized,
    parse::{Parse, ParseStream},
    Data, DataStruct, Error, Expr, Fields, Ident, LitStr, Path, Result, Token,
};

/// Converts a ASN.1 CamelCase string into snake_case.
/// Used for the struct field names.
pub fn field_ident_to_snake_case(ident: &Ident) -> Ident {
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
    Leaf(NamedBlock),
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
        type_ident: &Ident,
        field_access: &proc_macro2::TokenStream,
        field_var: &Ident,
        err_instead_of_false: bool,
    ) -> proc_macro2::TokenStream {
        match self {
            ConstraintExpr::Leaf(named_block) => named_block.value.quote(
                type_ident,
                &named_block.name,
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
                            let field_basename = field_ident_to_snake_case(&named_block.name);
                            let combined_field_access =
                                quote::quote!(#field_access.#field_basename);
                            e.quote(type_ident, &combined_field_access, field_var, true)
                        }
                        _ => e.quote(type_ident, field_access, field_var, true),
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
                        ConstraintExpr::Leaf(inner) => field_ident_to_snake_case(&inner.name),
                        _ => type_ident.clone(),
                    };
                    let field_access = match expr {
                        ConstraintExpr::Leaf(_) => quote::quote!(#field_access.#field_basename),
                        _ => field_access.clone(),
                    };
                    let constr_identifier =
                        format_ident!("__constraint_{}_ok", &field_basename.to_string());
                    // Expression returns boolean instead of error if fails, so we can combined the OR check later on
                    let check = expr.quote(type_ident, &field_access, &constr_identifier, false);
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
                // If you allow nested blocks in the AST, you can delegate.
                // Here you might simply call the quote on the BlockContents.
                // block.quote(field_ident, field_access)
                todo!("ConstraintExpr::Block todo")
            }
        }
    }
}

impl Parse for ConstraintExpr {
    fn parse(input: ParseStream) -> Result<Self> {
        // If expression starts with a parenthesized group, it's is two or more expressions, chained with or/and.
        if input.peek(syn::token::Paren) {
            let mut expr = parse_single_expr(input)?;

            while input.peek(Ident) {
                let op: Ident = input.parse()?;
                let op_str = op.to_string();
                if op_str != "or" && op_str != "and" {
                    // Not a valid operator, push it back or break.
                    return Err(syn::Error::new(op.span(), "Expected 'or' or 'and'"));
                }
                let next_expr = parse_single_expr(input)?;
                // Combine based on operator.
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
            // ident?
        } else if input.peek(Ident) {
            // the first field identifier
            // let parser = <NamedBlock as syn::parse::Parse>::parse;
            println!("Parsing NamedBlock first time...");
            let parsed = NamedBlock::parse(input)?;
            dbg!(parsed);
            Err(syn::Error::new(input.span(), "TODO"))
        } else {
            // error
            Err(syn::Error::new(
                input.span(),
                "Expected a parenthesized group",
            ))
        }
    }
}

/// Represents: `someIdentifier => <ConstraintValue>`
#[derive(Debug)]
struct NamedBlock {
    name: Ident,
    kind: BlockKind,
    value: ConstraintValue,
}

/// Possible things that can appear after `=>`:
#[derive(Debug)]
enum ConstraintValue {
    /// A block with optional `kind` and zero or more `NamedBlock`s
    Block(BlockContents),
    /// For something like `(MyType, size(1))` #  Subtype Notation
    ConstrainedType(ConstrainedType),
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
            _ => {
                quote! {}
            }
        }
    }
}

#[derive(Debug)]
struct ConstrainedType {
    ty: Path,
    size_constraint: Option<usize>,
}
impl Parse for ConstrainedType {
    fn parse(input: ParseStream) -> Result<Self> {
        let content;
        parenthesized!(content in input);
        let ty: Path = content.parse()?;
        if content.peek(Token![,]) {
            let _: Token![,] = content.parse()?;
            let size: Expr = content.parse()?;
            match size {
                // looking for e.g. size(1)
                Expr::Call(call) => {
                    if let Expr::Path(path) = &*call.func {
                        if path.path.is_ident("size") {
                            let size = match call.args.first() {
                                Some(Expr::Lit(lit)) => {
                                    if let syn::Lit::Int(int) = &lit.lit {
                                        Some(int.base10_parse::<usize>().unwrap())
                                    } else {
                                        None
                                    }
                                }
                                _ => None,
                            };
                            return Ok(ConstrainedType {
                                ty,
                                size_constraint: size,
                            });
                        }
                    }
                    return Err(
                        Error::new(content.span(), "Invalid With Component constraint. Currently only type and size are supported.")
                    );

                }
                _ => return Err(
                    Error::new(content.span(), "Invalid With Component constraint. Currently only type and size are supported.")
                ),
            }
        }
        Ok(ConstrainedType {
            ty,
            size_constraint: None,
        })
    }
}

/// When we see `{ ... }`, we parse into `BlockContents`.
#[derive(Debug)]
struct BlockContents {
    /// Comma-separated sub-blocks: `tbsData => { ... }, signer => present, ...`
    blocks: Vec<NamedBlock>,
}

impl Parse for BlockContents {
    fn parse(input: ParseStream) -> Result<Self> {
        let blocks = input
            .parse_terminated(NamedBlock::parse, Token![,])?
            .into_iter()
            .collect();
        Ok(BlockContents { blocks })
    }
}

#[derive(Debug)]
enum BlockKind {
    Choice(String),
    Sequence(String),
    Set(String),
    SequenceOf(String),
    SetOf(String),
}
impl BlockKind {
    fn from_str(s: &str) -> Self {
        let (kind, inner_type) = s.split_once(':').unwrap_or(("", ""));
        match kind {
            "choice" => BlockKind::Choice(inner_type.into()),
            "sequence" => BlockKind::Sequence(inner_type.into()),
            "set" => BlockKind::Set(inner_type.into()),
            "sequenceof" => BlockKind::SequenceOf(inner_type.into()),
            "setof" => BlockKind::SetOf(inner_type.into()),
            _ => BlockKind::Sequence(":".into()),
        }
    }
    fn get_named_type(&self) -> &str {
        match self {
            BlockKind::Choice(inner_type)
            | BlockKind::Sequence(inner_type)
            | BlockKind::Set(inner_type)
            | BlockKind::SequenceOf(inner_type)
            | BlockKind::SetOf(inner_type) => inner_type,
        }
    }
}

// e.g.  kind = "choice",
// Assume that "kind" ident already parsed
impl Parse for BlockKind {
    fn parse(input: ParseStream) -> Result<Self> {
        let _: Token![=] = input.parse()?;
        let kind_str: LitStr = input.parse()?;
        Ok(BlockKind::from_str(&kind_str.value()))
    }
}

impl Parse for ConstraintValue {
    fn parse(input: ParseStream) -> Result<Self> {
        if input.peek(syn::token::Brace) {
            let content;
            braced!(content in input);
            let blocks = content
                .parse_terminated(NamedBlock::parse, Token![,])?
                .into_iter()
                .collect();
            Ok(ConstraintValue::Block(BlockContents { blocks }))
        } else if input.peek(Ident) {
            let next_ident: Ident = input.parse()?;
            if next_ident == "present" {
                Ok(ConstraintValue::Present)
            } else if next_ident == "absent" {
                Ok(ConstraintValue::Absent)
            } else {
                Err(Error::new(
                    input.span(),
                    "Expected a block with a field name, e.g. `tbsData => { ... }` or tbsData = absent or type, e.g. `kind = \"choice\"`",
                ))
            }
        } else if input.peek(syn::token::Paren) {
            let constrained_type: ConstrainedType = input.parse()?;
            Ok(ConstraintValue::ConstrainedType(constrained_type))
        } else {
            Err(Error::new(
                input.span(),
                "Expected a block with a field name, e.g. `tbsData => { ... }` or component constraint, e.g. `(MyType, size(1))`",
            ))
        }
    }
}

impl Parse for NamedBlock {
    fn parse(input: ParseStream) -> Result<Self> {
        dbg!(input);
        let next_ident: Ident = input.parse()?;
        let mut kind = BlockKind::Sequence("".into());
        let name = if next_ident == "kind" && input.peek(Token![=]) {
            kind = BlockKind::parse(input)?;
            // 'kind' should be the first entry in the block, and remaining cannot be empty
            let _: Token![,] = input.parse()?;
            input.parse()?
        } else {
            next_ident
        };
        let _: Token![=>] = input.parse()?;
        let value: ConstraintValue = input.parse()?;
        Ok(Self { name, kind, value })
    }
}

/// Attempt to parse parenthesized group, with ident and =>
fn parse_single_expr(input: ParseStream) -> Result<ConstraintExpr> {
    // Expect a parenthesized group, just with single expression for non-block constraint values.
    let content;
    parenthesized!(content in input);
    let nb: NamedBlock = content.parse()?;
    Ok(ConstraintExpr::Leaf(nb))
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
    match data {
        Data::Struct(DataStruct {
            fields: Fields::Named(_),
            ..
        }) => {}
        _ => {
            return Err(Error::new(
                ident.span(),
                "Expected a standard struct with named fields.",
            ))
        }
    }
    let parser = <ConstraintExpr as syn::parse::Parse>::parse;
    let parsed = parser.parse2(dsl)?;
    let constraint_ident = format_ident!("__constraint_ok");
    let default_field_access = quote! { self };
    let checks = parsed.quote(ident, &default_field_access, &constraint_ident, true);
    Ok(insert_to_validate_components_skeleton(ident, checks))
}

mod tests {

    use super::*;
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
        let checks = parsed.quote(&ident, &default_field_access, &constraint_ident, true);
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
        let checks = parsed.quote(&ident, &default_field_access, &constraint_ident, true);
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
        // #[inner_subtype_constraint(
        //     r#type => choice(CertificateType::implicit),
        //     toBeSigned => {
        //         verifyKeyIndicator => choice(VerificationKeyIndicator::reconstructionValue)
        //     },
        //     signature => absent
        // )]
        let dsl = quote! {
            r#type => choice(CertificateType::implicit),
            toBeSigned => {
                verifyKeyIndicator => choice(VerificationKeyIndicator::reconstructionValue)
            },
            signature => absent
        };
        let parser = <ConstraintExpr as syn::parse::Parse>::parse;
        let parsed = parser.parse2(dsl).unwrap();
        dbg!(parsed);
    }
}
