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
pub struct DbEdge {
    pub db_edge: DbEdgeInner,
}

#[derive(FromMeta)]
pub struct DbEdgeInner {
    #[allow(dead_code)]
    #[darling(default)]
    default: (),
    #[darling(default)]
    foreign_key_field: Option<syn::Ident>,
    #[darling(default)]
    model: Option<syn::Path>,
    #[darling(default)]
    root_model_field: Option<syn::Ident>,
}

#[derive(FromMeta)]
pub struct VecDbEdge {
    pub vec_db_edge: VecDbEdgeInner,
}

#[derive(FromMeta)]
pub struct VecDbEdgeInner {
    #[darling(default)]
    foreign_key_field: Option<syn::Ident>,
    #[darling(default)]
    model: Option<syn::Path>,
    root_model_field: syn::Ident,
    #[darling(default)]
    association_type: Option<AssociationType>,
}

pub struct FieldArgs {
    foreign_key_field: Option<syn::Ident>,
    model: Option<syn::Path>,
    root_model_field: Option<syn::Ident>,
    association_type: Option<AssociationType>,
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

    pub fn association_type(&self) -> AssociationType {
        if let Some(ty) = self.association_type {
            ty
        } else {
            AssociationType::OneToMany
        }
    }
}

impl From<DbEdgeInner> for FieldArgs {
    fn from(inner: DbEdgeInner) -> Self {
        Self {
            foreign_key_field: inner.foreign_key_field,
            model: inner.model,
            root_model_field: inner.root_model_field,
            association_type: None,
        }
    }
}

impl From<VecDbEdgeInner> for FieldArgs {
    fn from(inner: VecDbEdgeInner) -> Self {
        Self {
            foreign_key_field: inner.foreign_key_field,
            model: inner.model,
            root_model_field: Some(inner.root_model_field),
            association_type: inner.association_type,
        }
    }
}

#[derive(FromMeta, Copy, Clone, Debug)]
pub enum AssociationType {
    OneToMany,
    ManyToMany,
}
