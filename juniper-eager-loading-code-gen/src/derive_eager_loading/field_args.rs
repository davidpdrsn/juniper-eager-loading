use bae::FromAttributes;
use heck::SnakeCase;
use proc_macro2::{Span, TokenStream};
use proc_macro_error::*;
use quote::{format_ident, quote};
use std::ops::{Deref, DerefMut};
use syn::{self, Ident};

macro_rules! token_stream_getter {
    ( $name:ident ) => {
        pub fn $name(&self) -> TokenStream {
            let value = &self.$name;
            quote! { #value }
        }
    };
}

#[derive(Debug, FromAttributes)]
pub struct EagerLoading {
    model: Option<syn::Type>,
    id: Option<syn::Type>,
    context: syn::Type,
    error: syn::Type,
    root_model_field: Option<syn::Ident>,
    print: Option<()>,
    primary_key_field: Option<syn::Ident>,
}

impl EagerLoading {
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

    pub fn primary_key_field(&self) -> syn::Ident {
        if let Some(id) = &self.primary_key_field {
            id.clone()
        } else {
            format_ident!("id")
        }
    }
}

#[derive(Debug, Clone, FromAttributes)]
pub struct HasOne {
    print: Option<()>,
    skip: Option<()>,
    field_arguments: Option<syn::TypePath>,
    foreign_key_field: Option<syn::Ident>,
    root_model_field: Option<syn::Ident>,
    graphql_field: Option<syn::Ident>,
    default: Option<()>,
    child_primary_key_field: Option<syn::Ident>,
}

impl HasOne {
    pub fn child_primary_key_field(&self) -> syn::Ident {
        let child_primary_key_field = &self.child_primary_key_field;

        if let Some(id) = child_primary_key_field {
            id.clone()
        } else {
            format_ident!("id")
        }
    }
}

#[derive(Debug, Clone, FromAttributes)]
pub struct OptionHasOne {
    print: Option<()>,
    skip: Option<()>,
    foreign_key_field: Option<syn::Ident>,
    root_model_field: Option<syn::Ident>,
    graphql_field: Option<syn::Ident>,
    default: Option<()>,
    field_arguments: Option<syn::TypePath>,
    child_primary_key_field: Option<syn::Ident>,
}

impl OptionHasOne {
    pub fn child_primary_key_field(&self) -> syn::Ident {
        let child_primary_key_field = &self.child_primary_key_field;

        if let Some(id) = child_primary_key_field {
            id.clone()
        } else {
            format_ident!("id")
        }
    }
}

#[derive(Debug, Clone, FromAttributes)]
pub struct HasMany {
    print: Option<()>,
    skip: Option<()>,
    field_arguments: Option<syn::TypePath>,
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

#[derive(Debug, Clone, FromAttributes)]
pub struct HasManyThrough {
    print: Option<()>,
    skip: Option<()>,
    field_arguments: Option<syn::TypePath>,
    model_field: Option<syn::Type>,
    join_model: Option<syn::TypePath>,
    foreign_key_field: Option<syn::Ident>,
    predicate_method: Option<syn::Ident>,
    graphql_field: Option<syn::Ident>,
    child_primary_key_field_on_join_model: Option<syn::Ident>,
    child_primary_key_field: Option<syn::Ident>,
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

    pub fn child_primary_key_field_on_join_model(&self, inner_type: &syn::Type) -> Ident {
        if let Some(id) = &self.child_primary_key_field_on_join_model {
            id.clone()
        } else {
            Ident::new(
                &format!("{}_id", self.model_field(inner_type)),
                Span::call_site(),
            )
        }
    }

    pub fn predicate_method(&self) -> &Option<syn::Ident> {
        &self.predicate_method
    }

    pub fn child_primary_key_field(&self) -> syn::Ident {
        if let Some(id) = &self.child_primary_key_field {
            id.clone()
        } else {
            format_ident!("id")
        }
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
            FieldArgs::OptionHasOne(inner) => inner.skip.is_some(),
            FieldArgs::HasMany(inner) => inner.skip.is_some(),
            FieldArgs::HasManyThrough(inner) => inner.skip.is_some(),
        }
    }

    pub fn print(&self) -> bool {
        match self {
            FieldArgs::HasOne(inner) => inner.print.is_some(),
            FieldArgs::OptionHasOne(inner) => inner.print.is_some(),
            FieldArgs::HasMany(inner) => inner.print.is_some(),
            FieldArgs::HasManyThrough(inner) => inner.print.is_some(),
        }
    }

    pub fn graphql_field(&self) -> &Option<syn::Ident> {
        match self {
            FieldArgs::HasOne(inner) => &inner.graphql_field,
            FieldArgs::OptionHasOne(inner) => &inner.graphql_field,
            FieldArgs::HasMany(inner) => &inner.graphql_field,
            FieldArgs::HasManyThrough(inner) => &inner.graphql_field,
        }
    }

    pub fn field_arguments(&self) -> syn::Type {
        let field_arguments = match self {
            FieldArgs::HasOne(inner) => &inner.field_arguments,
            FieldArgs::OptionHasOne(inner) => &inner.field_arguments,
            FieldArgs::HasMany(inner) => &inner.field_arguments,
            FieldArgs::HasManyThrough(inner) => &inner.field_arguments,
        };

        if let Some(field_arguments) = field_arguments {
            syn::parse2(quote! { #field_arguments<'a> }).unwrap()
        } else {
            syn::parse_str("()").unwrap()
        }
    }

    pub fn foreign_key_field(&self, field_name: &Ident) -> TokenStream {
        let foreign_key_field = match self {
            FieldArgs::HasOne(inner) => &inner.foreign_key_field,
            FieldArgs::OptionHasOne(inner) => &inner.foreign_key_field,
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
        &self.root_model_field
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
