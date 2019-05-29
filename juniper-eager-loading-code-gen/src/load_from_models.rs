use darling::FromDeriveInput;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

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
#[darling(attributes(load_from_models), forward_attrs(doc, cfg, allow))]
struct Options {
    #[darling(default)]
    connection: Option<syn::Path>,
    table: syn::Path,
    from_model: syn::Path,
    foreign_key: syn::Ident,
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
        let connection = self.connection();
        let table = self.table();
        let error = self.error();
        let from_model = self.from_model();
        let foreign_key = self.foreign_key();

        self.tokens.extend(quote! {
            impl juniper_eager_loading::LoadFromModels<#from_model> for #struct_name {
                type Error = #error;
                type Connection = #connection;

                fn load(
                    models: &[#from_model],
                    db: &Self::Connection,
                ) -> Result<Vec<TeamMembership>, Self::Error> {
                    use diesel::pg::expression::dsl::any;
                    use schema::#table;

                    let model_ids = models
                        .iter()
                        .map(|model| model.id)
                        .collect::<Vec<_>>();

                    let res = #table::table
                        .filter(#table::#foreign_key.eq(any(model_ids)))
                        .load::<#struct_name>(db)?;

                    Ok(res)
                }
            }
        });

        self.tokens
    }

    fn struct_name(&self) -> &syn::Ident {
        &self.input.ident
    }

    fn from_model(&self) -> &syn::Path {
        &self.options.from_model
    }

    fn foreign_key(&self) -> &syn::Ident {
        &self.options.foreign_key
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
