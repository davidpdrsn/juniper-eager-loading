use heck::SnakeCase;
use proc_macro2::{Span, TokenStream};
use proc_macro_error::*;
use quote::quote;
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

#[derive(Debug)]
pub struct DeriveArgs {
    model: Option<syn::Path>,
    id: Option<syn::Path>,
    context: syn::Path,
    error: syn::Path,
    root_model_field: Option<syn::Ident>,
    // TODO: Document this new attribute
    print: Option<()>,
}

impl Parse for DeriveArgs {
    fn parse(input: ParseStream) -> syn::Result<DeriveArgs> {
        let mut print = None;
        let mut model = None;
        let mut id = None;
        let mut context = None;
        let mut error = None;
        let mut root_model_field = None;

        let content;
        syn::parenthesized!(content in input);
        while !content.is_empty() {
            let ident = content.parse::<Ident>()?;

            match &*ident.to_string() {
                "print" => print = Some(()),
                "model" => {
                    content.parse::<Token![=]>()?;
                    model = Some(content.parse()?);
                }
                "id" => {
                    content.parse::<Token![=]>()?;
                    id = Some(content.parse()?);
                }
                "context" => {
                    content.parse::<Token![=]>()?;
                    context = Some(content.parse()?);
                }
                "error" => {
                    content.parse::<Token![=]>()?;
                    error = Some(content.parse()?);
                }
                "root_model_field" => {
                    content.parse::<Token![=]>()?;
                    root_model_field = Some(content.parse()?);
                }
                other => abort!(ident.span(), "Unknown argument `{}`", other),
            }

            content.parse::<Token![,]>().ok();
        }

        Ok(DeriveArgs {
            print,
            model,
            id,
            context: context.unwrap(),
            error: error.unwrap(),
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

pub struct HasOne {
    print: Option<()>,
    skip: Option<()>,
    foreign_key_field: Option<syn::Ident>,
    root_model_field: Option<syn::Ident>,
    graphql_field: Option<syn::Ident>,
}

impl Parse for HasOne {
    fn parse(input: ParseStream) -> syn::Result<HasOne> {
        let mut print = None;
        let mut skip = None;
        let mut foreign_key_field = None;
        let mut root_model_field = None;
        let mut graphql_field = None;

        let content;
        syn::parenthesized!(content in input);
        while !content.is_empty() {
            let ident = content.parse::<Ident>()?;

            match &*ident.to_string() {
                "print" => print = Some(()),
                "skip" => skip = Some(()),
                "default" => {}
                "foreign_key_field" => {
                    content.parse::<Token![=]>()?;
                    foreign_key_field = Some(content.parse()?);
                }
                "root_model_field" => {
                    content.parse::<Token![=]>()?;
                    root_model_field = Some(content.parse()?);
                }
                "graphql_field" => {
                    content.parse::<Token![=]>()?;
                    graphql_field = Some(content.parse()?);
                }
                other => abort!(ident.span(), "Unknown argument `{}`", other),
            }

            content.parse::<Token![,]>().ok();
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

pub struct HasMany {
    print: Option<()>,
    skip: Option<()>,
    foreign_key_field: Option<syn::Ident>,
    foreign_key_optional: Option<()>,
    root_model_field: Option<syn::Ident>,
    predicate_method: Option<syn::Ident>,
    graphql_field: Option<syn::Ident>,
}

impl Parse for HasMany {
    fn parse(input: ParseStream) -> syn::Result<HasMany> {
        let mut print = None;
        let mut skip = None;
        let mut foreign_key_field = None;
        let mut foreign_key_optional = None;
        let mut root_model_field = None;
        let mut predicate_method = None;
        let mut graphql_field = None;

        let content;
        syn::parenthesized!(content in input);
        while !content.is_empty() {
            let ident = content.parse::<Ident>()?;

            match &*ident.to_string() {
                "print" => print = Some(()),
                "skip" => skip = Some(()),
                "foreign_key_optional" => foreign_key_optional = Some(()),
                "foreign_key_field" => {
                    content.parse::<Token![=]>()?;
                    foreign_key_field = Some(content.parse()?);
                }
                "root_model_field" => {
                    content.parse::<Token![=]>()?;
                    root_model_field = Some(content.parse()?);
                }
                "predicate_method" => {
                    content.parse::<Token![=]>()?;
                    predicate_method = Some(content.parse()?);
                }
                "graphql_field" => {
                    content.parse::<Token![=]>()?;
                    graphql_field = Some(content.parse()?);
                }
                // TODO: use abort!
                other => abort!(ident.span(), "Unknown argument `{}`", other),
            }

            content.parse::<Token![,]>().ok();
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

pub struct HasManyThrough {
    print: Option<()>,
    skip: Option<()>,
    join_model: Option<syn::Path>,
    model_field: Option<syn::Path>,
    join_model_field: Option<syn::Path>,
    foreign_key_field: Option<syn::Ident>,
    predicate_method: Option<syn::Ident>,
    graphql_field: Option<syn::Ident>,
}

impl Parse for HasManyThrough {
    fn parse(input: ParseStream) -> syn::Result<HasManyThrough> {
        let mut print = None;
        let mut skip = None;
        let mut join_model = None;
        let mut model_field = None;
        let mut join_model_field = None;
        let mut foreign_key_field = None;
        let mut predicate_method = None;
        let mut graphql_field = None;

        let content;
        syn::parenthesized!(content in input);
        while !content.is_empty() {
            let ident = content.parse::<Ident>()?;

            match &*ident.to_string() {
                "print" => print = Some(()),
                "skip" => skip = Some(()),
                "join_model" => {
                    content.parse::<Token![=]>()?;
                    join_model = Some(content.parse()?);
                }
                "model_field" => {
                    content.parse::<Token![=]>()?;
                    model_field = Some(content.parse()?);
                }
                "join_model_field" => {
                    content.parse::<Token![=]>()?;
                    join_model_field = Some(content.parse()?);
                }
                "foreign_key_field" => {
                    content.parse::<Token![=]>()?;
                    foreign_key_field = Some(content.parse()?);
                }
                "predicate_method" => {
                    content.parse::<Token![=]>()?;
                    predicate_method = Some(content.parse()?);
                }
                "graphql_field" => {
                    content.parse::<Token![=]>()?;
                    graphql_field = Some(content.parse()?);
                }
                // TODO: use abort!
                other => abort!(ident.span(), "Unknown argument `{}`", other),
            }

            content.parse::<Token![,]>().ok();
        }

        Ok(HasManyThrough {
            print,
            skip,
            join_model,
            model_field,
            join_model_field,
            foreign_key_field,
            predicate_method,
            graphql_field,
        })
    }
}

pub struct FieldArgs {
    foreign_key_field: Option<syn::Ident>,
    pub foreign_key_optional: bool,
    join_model_field: Option<syn::Path>,
    model_field: Option<syn::Path>,
    pub join_model: Option<syn::Path>,
    pub skip: bool,
    pub print: bool,
    root_model_field: Option<syn::Ident>,
    predicate_method: Option<syn::Ident>,
    graphql_field: Option<syn::Ident>,
}

impl FieldArgs {
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
        } else if let Some(join_model) = &self.join_model {
            let name = join_model.segments.last().unwrap();
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

fn type_to_string(ty: &syn::Type) -> String {
    use quote::ToTokens;
    let mut tokenized = quote! {};
    ty.to_tokens(&mut tokenized);
    tokenized.to_string()
}

impl From<HasOne> for FieldArgs {
    fn from(inner: HasOne) -> Self {
        Self {
            foreign_key_field: inner.foreign_key_field,
            foreign_key_optional: false,
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

impl From<HasMany> for FieldArgs {
    fn from(inner: HasMany) -> Self {
        if inner.root_model_field.is_none() && inner.skip.is_none() {
            panic!("For the attribute #[has_many(...)] you must provide either `root_model_field` or `skip`. Both were missing");
        }

        Self {
            foreign_key_field: inner.foreign_key_field,
            foreign_key_optional: inner.foreign_key_optional.is_some(),
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

impl From<HasManyThrough> for FieldArgs {
    fn from(inner: HasManyThrough) -> Self {
        if inner.join_model.is_none() && inner.skip.is_none() {
            panic!("For the attribute #[has_many_through(...)] you must provide either `join_model` or `skip`. Both were missing");
        }

        Self {
            foreign_key_field: inner.foreign_key_field,
            foreign_key_optional: false,
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
