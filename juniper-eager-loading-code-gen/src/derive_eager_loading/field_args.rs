use darling::{FromDeriveInput, FromMeta};
use heck::SnakeCase;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{self, Ident};

macro_rules! token_stream_getter {
    ( $name:ident ) => {
        pub fn $name(&self) -> TokenStream {
            let value = &self.$name;
            quote! { #value }
        }
    }
}

#[derive(FromDeriveInput, Debug)]
#[darling(attributes(eager_loading), forward_attrs(doc, cfg, allow))]
pub struct DeriveArgs {
    #[darling(default)]
    model: Option<syn::Path>,
    #[darling(default)]
    id: Option<syn::Path>,
    connection: syn::Path,
    error: syn::Path,
    #[darling(default)]
    root_model_field: Option<syn::Ident>,
}

impl DeriveArgs {
    token_stream_getter!(connection);
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
}

#[derive(FromMeta)]
pub struct HasOne {
    pub has_one: HasOneInner,
}

#[derive(FromMeta)]
pub struct OptionHasOne {
    pub option_has_one: HasOneInner,
}

#[derive(FromMeta)]
pub struct HasOneInner {
    #[darling(default)]
    print: Option<()>,
    #[darling(default)]
    skip: Option<()>,
    #[allow(dead_code)]
    #[darling(default)]
    default: (),
    #[darling(default)]
    foreign_key_field: Option<syn::Ident>,
    #[darling(default)]
    model: Option<syn::Path>,
    #[darling(default)]
    root_model_field: Option<syn::Ident>,
    #[darling(default)]
    graphql_field: Option<syn::Ident>,
}

#[derive(FromMeta)]
pub struct HasMany {
    pub has_many: HasManyInner,
}

#[derive(FromMeta)]
pub struct HasManyInner {
    #[darling(default)]
    print: Option<()>,
    #[darling(default)]
    skip: Option<()>,
    #[darling(default)]
    foreign_key_field: Option<syn::Ident>,
    #[darling(default)]
    model: Option<syn::Path>,
    #[darling(default)]
    root_model_field: Option<syn::Ident>,
    #[darling(default)]
    predicate_method: Option<syn::Ident>,
    #[darling(default)]
    graphql_field: Option<syn::Ident>,
}

#[derive(FromMeta)]
pub struct HasManyThrough {
    pub has_many_through: HasManyThroughInner,
}

#[derive(FromMeta)]
pub struct HasManyThroughInner {
    #[darling(default)]
    print: Option<()>,
    #[darling(default)]
    skip: Option<()>,
    #[darling(default)]
    model: Option<syn::Path>,
    #[darling(default)]
    join_model: Option<syn::Path>,
    #[darling(default)]
    model_field: Option<syn::Path>,
    #[darling(default)]
    join_model_field: Option<syn::Path>,
    #[darling(default)]
    predicate_method: Option<syn::Ident>,
    #[darling(default)]
    graphql_field: Option<syn::Ident>,
}

pub struct FieldArgs {
    foreign_key_field: Option<syn::Ident>,
    join_model_field: Option<syn::Path>,
    model: Option<syn::Path>,
    model_field: Option<syn::Path>,
    pub join_model: Option<syn::Path>,
    pub skip: bool,
    pub print: bool,
    root_model_field: Option<syn::Ident>,
    predicate_method: Option<syn::Ident>,
    graphql_field: Option<syn::Ident>,
}

impl FieldArgs {
    pub fn model(&self, inner_type: &syn::Type) -> TokenStream {
        if let Some(inner) = &self.model {
            quote! { #inner }
        } else {
            quote! { models::#inner_type }
        }
    }

    pub fn foreign_key_field(&self, field_name: &Ident) -> TokenStream {
        if let Some(inner) = &self.foreign_key_field {
            quote! { #inner }
        } else {
            let field_name = field_name.to_string().to_snake_case();
            let field_name = format!("{}_id", field_name);
            let field_name = Ident::new(&field_name, Span::call_site());
            quote! { #field_name }
        }
    }

    pub fn root_model_field(&self, field_name: &Ident) -> TokenStream {
        if let Some(inner) = &self.root_model_field {
            quote! { #inner }
        } else {
            let field_name = field_name.to_string().to_snake_case();
            let field_name = Ident::new(&field_name, Span::call_site());
            quote! { #field_name }
        }
    }

    pub fn graphql_field(&self) -> &Option<syn::Ident> {
        &self.graphql_field
    }

    pub fn predicate_method(&self) -> Option<syn::Ident> {
        self.predicate_method.clone()
    }

    pub fn join_model(&self) -> TokenStream {
        if let Some(inner) = &self.join_model {
            quote! { #inner }
        } else {
            quote! { () }
        }
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

    pub fn join_model_field(&self) -> TokenStream {
        if let Some(inner) = &self.join_model_field {
            quote! { #inner }
        } else {
            if let Some(join_model) = &self.join_model {
                let name = join_model.segments.last().unwrap();
                let name = name.value();
                let name = &name.ident;
                let name = name.to_string().to_snake_case();
                let name = Ident::new(&name, Span::call_site());
                quote! { #name }
            } else {
                // This method is only used by `HasManyThrough` for which the `model_field` attribute is
                // mandatory, so it will always be present when needed.
                quote! { __eager_loading_unreachable }
            }
        }
    }
}

fn type_to_string(ty: &syn::Type) -> String {
    use quote::ToTokens;
    let mut tokenized = quote! {};
    ty.to_tokens(&mut tokenized);
    tokenized.to_string()
}

impl From<HasOneInner> for FieldArgs {
    fn from(inner: HasOneInner) -> Self {
        Self {
            foreign_key_field: inner.foreign_key_field,
            model: inner.model,
            root_model_field: inner.root_model_field,
            join_model: None,
            model_field: None,
            join_model_field: None,
            skip: inner.skip.is_some(),
            print: inner.print.is_some(),
            predicate_method: None,
            graphql_field: inner.graphql_field,
        }
    }
}

impl From<HasManyInner> for FieldArgs {
    fn from(inner: HasManyInner) -> Self {
        if inner.root_model_field.is_none() && inner.skip.is_none() {
            panic!("For the attribute #[has_many(...)] you must provide either `root_model_field` or `skip`. Both were missing");
        }

        Self {
            foreign_key_field: inner.foreign_key_field,
            model: inner.model,
            root_model_field: inner.root_model_field,
            join_model: None,
            model_field: None,
            join_model_field: None,
            skip: inner.skip.is_some(),
            print: inner.print.is_some(),
            predicate_method: inner.predicate_method,
            graphql_field: inner.graphql_field,
        }
    }
}

impl From<HasManyThroughInner> for FieldArgs {
    fn from(inner: HasManyThroughInner) -> Self {
        if inner.join_model.is_none() && inner.skip.is_none() {
            panic!("For the attribute #[has_many_through(...)] you must provide either `join_model` or `skip`. Both were missing");
        }

        Self {
            foreign_key_field: None,
            model: inner.model,
            root_model_field: None,
            join_model: inner.join_model,
            model_field: inner.model_field,
            join_model_field: inner.join_model_field,
            skip: inner.skip.is_some(),
            print: inner.print.is_some(),
            predicate_method: inner.predicate_method,
            graphql_field: inner.graphql_field,
        }
    }
}
