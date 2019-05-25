use darling::{FromDeriveInput, FromMeta};
use lazy_static::lazy_static;
use proc_macro2::TokenStream;
use quote::quote;
use std::sync::atomic::{AtomicBool, Ordering};
use syn::{parse_macro_input, DeriveInput, GenericArgument, NestedMeta, PathArguments, Type};

pub fn gen_tokens(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let options = match Options::from_derive_input(&ast) {
        Ok(options) => options,
        Err(err) => panic!("{}", err),
    };

    let out = DeriveData::new(ast, options);
    let tokens = out.build_derive_output();

    tokens.into()
}

#[derive(FromDeriveInput, Debug)]
#[darling(attributes(load_from_ids), forward_attrs(doc, cfg, allow))]
struct Options {
    #[darling(default)]
    id: Option<syn::Path>,
    #[darling(default)]
    connection: Option<syn::Path>,
    table: syn::Path,
    #[darling(default)]
    error: Option<syn::Path>,
}

struct DeriveData {
    input: DeriveInput,
    options: Options,
    tokens: TokenStream,
}

impl DeriveData {
    fn new(input: DeriveInput, options: Options) -> Self {
        Self {
            input,
            options,
            tokens: quote! {},
        }
    }

    fn build_derive_output(mut self) -> TokenStream {
        let struct_name = self.struct_name();
        let id = self.id();
        let connection = self.connection();
        let table = self.table();
        let error = self.error();

        self.tokens.extend(quote! {
            impl juniper_eager_loading::LoadFromIds for #struct_name {
                type Id = #id;
                type Error = #error;
                type Connection = #connection;

                fn load(ids: &[Self::Id], db: &Self::Connection) -> Result<Vec<Self>, Self::Error> {
                    use diesel::pg::expression::dsl::any;

                    #table::table
                        .filter(#table::id.eq(any(ids)))
                        .load::<#struct_name>(db)
                        .map_err(std::convert::From::from)
                }
            }
        });

        self.tokens
    }

    fn struct_name(&self) -> &syn::Ident {
        &self.input.ident
    }

    fn id(&self) -> TokenStream {
        self.options
            .id
            .as_ref()
            .map(|inner| quote! { #inner })
            .unwrap_or_else(|| quote! { i32 })
    }

    fn connection(&self) -> TokenStream {
        self.options
            .connection
            .as_ref()
            .map(|inner| quote! { #inner })
            .unwrap_or_else(|| quote! { diesel::pg::PgConnection })
    }

    fn table(&self) -> &syn::Path {
        &self.options.table
    }

    fn error(&self) -> TokenStream {
        self.options
            .error
            .as_ref()
            .map(|inner| quote! { #inner })
            .unwrap_or_else(|| quote! { diesel::result::Error })
    }
}
