use heck::SnakeCase;
use proc_macro2::{Span, TokenStream};
use proc_macro_error::*;
use quote::{format_ident, quote};
use std::ops::{Deref, DerefMut};
use syn::{
    self,
    parse::{Parse, ParseStream},
    Ident, Token,
};

macro_rules! token_stream_getter {
    ( $name:ident ) => {
        pub fn $name(&self) -> TokenStream {
            let value = &self.$name;
            quote! { #value }
        }
    }
}

macro_rules! parse_attrs {
    (
        $input:ident,
        noops = [$($noop:ident),*],
        switches = [$($switch:ident),*],
        values = [$($value:ident),*],
    ) => {
        $( $switch = None; )*
        $( $value = None; )*

        let content;
        syn::parenthesized!(content in $input);

        while !content.is_empty() {
            let ident = content.parse::<Ident>()?;

            match &*ident.to_string() {
                $( stringify!($noop) => {}, )*
                $( stringify!($switch) => $switch = Some(()), )*
                $(
                    stringify!($value) => {
                        content.parse::<Token![=]>()?;
                        $value = Some(content.parse()?);
                    }
                )*
                other => {
                    let supported = [
                        $( stringify!($noop), )*
                        $( stringify!($switch), )*
                        $( stringify!($value), )*
                    ]
                        .iter()
                        .map(|s| format!("`{}`", s))
                        .collect::<Vec<_>>()
                        .join(", ");

                    abort!(
                        ident.span(),
                        "Unknown argument `{}`. Supported arguments are {}",
                        other,
                        supported,
                    )
                }
            }

            content.parse::<Token![,]>().ok();
        }
    }
}

#[derive(Debug)]
pub struct DeriveArgs {
    model: Option<syn::Type>,
    id: Option<syn::Type>,
    context: syn::Type,
    error: syn::Type,
    root_model_field: Option<syn::Ident>,
    // TODO: Document this new attribute
    print: Option<()>,
}

impl Parse for DeriveArgs {
    fn parse(input: ParseStream) -> syn::Result<DeriveArgs> {
        let mut print;
        let mut model;
        let mut id;
        let mut context;
        let mut error;
        let mut root_model_field;

        parse_attrs! {
            input,
            noops = [],
            switches = [print],
            values = [model, id, context, error, root_model_field],
        }

        let context = get_mandatory_arg(input, context, "eager_loading", "context")?;
        let error = get_mandatory_arg(input, error, "eager_loading", "error")?;

        Ok(DeriveArgs {
            print,
            model,
            id,
            context,
            error,
            root_model_field,
        })
    }
}

impl DeriveArgs {
    token_stream_getter!(context);
    token_stream_getter!(error);

    pub fn model(&self, struct_name: &syn::Ident) -> TokenStream {
        if let Some(inner) = &self.model {
            quote! { #inner }
        } else {
            quote! { models::#struct_name }
        }
    }

    pub fn id(&self) -> TokenStream {
        if let Some(inner) = &self.id {
            quote! { #inner }
        } else {
            quote! { i32 }
        }
    }

    pub fn root_model_field(&self, struct_name: &syn::Ident) -> TokenStream {
        if let Some(inner) = &self.root_model_field {
            quote! { #inner }
        } else {
            let struct_name = struct_name.to_string().to_snake_case();
            let struct_name = Ident::new(&struct_name, Span::call_site());
            quote! { #struct_name }
        }
    }

    pub fn print(&self) -> bool {
        self.print.is_some()
    }
}

#[derive(Debug, Clone)]
pub struct HasOne {
    print: Option<()>,
    skip: Option<()>,
    foreign_key_field: Option<syn::Ident>,
    root_model_field: Option<syn::Ident>,
    graphql_field: Option<syn::Ident>,
}

impl Parse for HasOne {
    fn parse(input: ParseStream) -> syn::Result<HasOne> {
        let mut print;
        let mut skip;
        let mut foreign_key_field;
        let mut root_model_field;
        let mut graphql_field;

        parse_attrs! {
            input,
            noops = [default],
            switches = [print, skip],
            values = [foreign_key_field, root_model_field, graphql_field],
        }

        Ok(HasOne {
            print,
            skip,
            foreign_key_field,
            root_model_field,
            graphql_field,
        })
    }
}

#[derive(Debug, Clone)]
pub struct OptionHasOne {
    has_one: HasOne,
}

impl Parse for OptionHasOne {
    fn parse(input: ParseStream) -> syn::Result<OptionHasOne> {
        let has_one = input.parse::<HasOne>()?;

        Ok(OptionHasOne { has_one })
    }
}

#[derive(Debug, Clone)]
pub struct HasMany {
    print: Option<()>,
    skip: Option<()>,
    foreign_key_field: Option<syn::Ident>,
    pub foreign_key_optional: Option<()>,
    root_model_field: Option<syn::Ident>,
    predicate_method: Option<syn::Ident>,
    graphql_field: Option<syn::Ident>,
}

impl HasMany {
    pub fn predicate_method(&self) -> &Option<syn::Ident> {
        &self.predicate_method
    }
}

impl Parse for HasMany {
    fn parse(input: ParseStream) -> syn::Result<HasMany> {
        let mut print;
        let mut skip;
        let mut foreign_key_field;
        let mut foreign_key_optional;
        let mut root_model_field;
        let mut predicate_method;
        let mut graphql_field;

        parse_attrs! {
            input,
            noops = [],
            switches = [print, skip, foreign_key_optional],
            values = [
                foreign_key_field,
                root_model_field,
                predicate_method,
                graphql_field
            ],
        }

        if root_model_field.is_none() && skip.is_none() {
            return Err(input.error("For the attribute #[has_many(...)] you must provide either `root_model_field` or `skip`. Both were missing"));
        }

        Ok(HasMany {
            print,
            skip,
            foreign_key_field,
            foreign_key_optional,
            root_model_field,
            predicate_method,
            graphql_field,
        })
    }
}

#[derive(Debug, Clone)]
pub struct HasManyThrough {
    print: Option<()>,
    skip: Option<()>,
    join_model: Option<syn::TypePath>,
    model_field: Option<syn::Type>,
    foreign_key_field: Option<syn::Ident>,
    predicate_method: Option<syn::Ident>,
    graphql_field: Option<syn::Ident>,
}

impl HasManyThrough {
    pub fn join_model(&self, span: Span) -> syn::Type {
        self.join_model
            .as_ref()
            .cloned()
            .map(syn::Type::Path)
            .unwrap_or_else(|| abort!(span, "`#[has_many_through]` missing `join_model`"))
    }

    pub fn model_field(&self, inner_type: &syn::Type) -> TokenStream {
        if let Some(inner) = &self.model_field {
            quote! { #inner }
        } else {
            let inner_type = type_to_string(inner_type).to_snake_case();
            let inner_type = Ident::new(&inner_type, Span::call_site());
            quote! { #inner_type }
        }
    }

    pub fn model_id_field(&self, inner_type: &syn::Type) -> Ident {
        Ident::new(
            &format!("{}_id", self.model_field(inner_type)),
            Span::call_site(),
        )
    }

    pub fn predicate_method(&self) -> &Option<syn::Ident> {
        &self.predicate_method
    }
}

impl Parse for HasManyThrough {
    fn parse(input: ParseStream) -> syn::Result<HasManyThrough> {
        let mut print;
        let mut skip;
        let mut join_model;
        let mut model_field;
        let mut foreign_key_field;
        let mut predicate_method;
        let mut graphql_field;

        parse_attrs! {
            input,
            noops = [],
            switches = [print, skip],
            values = [
                join_model,
                model_field,
                foreign_key_field,
                predicate_method,
                graphql_field
            ],
        }

        if join_model.is_none() && skip.is_none() {
            return Err(input.error("For the attribute #[has_many_through(...)] you must provide either `join_model` or `skip`. Both were missing"));
        }

        Ok(HasManyThrough {
            print,
            skip,
            join_model,
            model_field,
            foreign_key_field,
            predicate_method,
            graphql_field,
        })
    }
}

#[derive(Debug, Clone)]
pub enum FieldArgs {
    HasOne(Spanned<HasOne>),
    OptionHasOne(Spanned<OptionHasOne>),
    HasMany(Spanned<HasMany>),
    HasManyThrough(Spanned<Box<HasManyThrough>>),
}

impl FieldArgs {
    pub fn skip(&self) -> bool {
        match self {
            FieldArgs::HasOne(inner) => inner.skip.is_some(),
            FieldArgs::OptionHasOne(inner) => inner.has_one.skip.is_some(),
            FieldArgs::HasMany(inner) => inner.skip.is_some(),
            FieldArgs::HasManyThrough(inner) => inner.skip.is_some(),
        }
    }

    pub fn print(&self) -> bool {
        match self {
            FieldArgs::HasOne(inner) => inner.print.is_some(),
            FieldArgs::OptionHasOne(inner) => inner.has_one.print.is_some(),
            FieldArgs::HasMany(inner) => inner.print.is_some(),
            FieldArgs::HasManyThrough(inner) => inner.print.is_some(),
        }
    }

    pub fn graphql_field(&self) -> &Option<syn::Ident> {
        match self {
            FieldArgs::HasOne(inner) => &inner.graphql_field,
            FieldArgs::OptionHasOne(inner) => &inner.has_one.graphql_field,
            FieldArgs::HasMany(inner) => &inner.graphql_field,
            FieldArgs::HasManyThrough(inner) => &inner.graphql_field,
        }
    }

    pub fn foreign_key_field(&self, field_name: &Ident) -> TokenStream {
        let foreign_key_field = match self {
            FieldArgs::HasOne(inner) => &inner.foreign_key_field,
            FieldArgs::OptionHasOne(inner) => &inner.has_one.foreign_key_field,
            FieldArgs::HasMany(inner) => &inner.foreign_key_field,
            FieldArgs::HasManyThrough(inner) => &inner.foreign_key_field,
        };

        if let Some(inner) = foreign_key_field {
            quote! { #inner }
        } else {
            let field_name = field_name.to_string().to_snake_case();
            let field_name = format_ident!("{}_id", field_name);
            quote! { #field_name }
        }
    }
}

pub trait RootModelField {
    fn get_root_model_field(&self) -> &Option<Ident>;

    fn root_model_field(&self, field_name: &Ident) -> TokenStream {
        if let Some(inner) = self.get_root_model_field() {
            quote! { #inner }
        } else {
            let field_name = field_name.to_string().to_snake_case();
            let field_name = Ident::new(&field_name, Span::call_site());
            quote! { #field_name }
        }
    }
}

impl RootModelField for HasOne {
    fn get_root_model_field(&self) -> &Option<Ident> {
        &self.root_model_field
    }
}

impl RootModelField for OptionHasOne {
    fn get_root_model_field(&self) -> &Option<Ident> {
        self.has_one.get_root_model_field()
    }
}

impl RootModelField for HasMany {
    fn get_root_model_field(&self) -> &Option<Ident> {
        &self.root_model_field
    }
}

fn type_to_string(ty: &syn::Type) -> String {
    use quote::ToTokens;
    let mut tokenized = quote! {};
    ty.to_tokens(&mut tokenized);
    tokenized.to_string()
}

#[derive(Debug, Clone)]
pub struct Spanned<T>(Span, T);

impl<T> Spanned<T> {
    pub fn new(span: Span, t: T) -> Self {
        Self(span, t)
    }

    pub fn span(&self) -> Span {
        self.0
    }
}

impl<T> Deref for Spanned<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.1
    }
}

impl<T> DerefMut for Spanned<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.1
    }
}

fn get_mandatory_arg<T>(
    input: ParseStream,
    value: Option<T>,
    attr: &str,
    name: &str,
) -> syn::Result<T> {
    if let Some(value) = value {
        Ok(value)
    } else {
        Err(input.error(&format!("#[{}] is missing `{}` argument", attr, name)))
    }
}
