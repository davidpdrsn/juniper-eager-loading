use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    braced, parenthesized,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    Ident, Token, Type,
};

pub fn go(input: proc_macro::TokenStream, backend: Backend) -> proc_macro::TokenStream {
    let input = match syn::parse::<Input>(input) {
        Ok(x) => x,
        Err(err) => return err.to_compile_error().into(),
    };

    let mut tokens = TokenStream::new();

    for impl_ in &input.impls {
        impl_.gen_tokens(&input, &backend, &mut tokens);
    }

    tokens.into()
}

#[derive(Debug)]
pub enum Backend {
    Pg,
    Mysql,
    Sqlite,
}

mod kw {
    syn::custom_keyword!(error);
    syn::custom_keyword!(context);
}

#[derive(Debug)]
struct Input {
    error_ty: Type,
    context_ty: Type,
    impls: Punctuated<InputImpl, Token![,]>,
}

impl Parse for Input {
    fn parse(input: ParseStream) -> syn::parse::Result<Self> {
        let prelude;
        parenthesized!(prelude in input);

        prelude.parse::<kw::error>()?;
        prelude.parse::<Token![=]>()?;
        let error_ty = prelude.parse::<Type>()?;

        prelude.parse::<Token![,]>()?;

        prelude.parse::<kw::context>()?;
        prelude.parse::<Token![=]>()?;
        let context_ty = prelude.parse::<Type>()?;

        if prelude.peek(Token![,]) {
            prelude.parse::<Token![,]>()?;
        }

        input.parse::<Token![=>]>()?;

        let content;
        braced!(content in input);
        let impls = Punctuated::parse_terminated(&content)?;

        Ok(Self {
            error_ty,
            context_ty,
            impls,
        })
    }
}

#[derive(Debug)]
enum InputImpl {
    HasOne(HasOne),
    HasMany(HasMany),
}

#[derive(Debug)]
struct HasOne {
    id_ty: Type,
    table: Ident,
    self_ty: Type,
}

#[derive(Debug)]
struct HasMany {
    join_ty: Type,
    join_from: Ident,
    table: Ident,
    join_to: Ident,
    self_ty: Type,
}

impl Parse for InputImpl {
    fn parse(input: ParseStream) -> syn::parse::Result<Self> {
        let id_ty = input.parse::<Type>()?;

        if input.peek(Token![.]) {
            let join_ty = id_ty;
            input.parse::<Token![.]>()?;
            let join_from = input.parse::<Ident>()?;

            input.parse::<Token![->]>()?;

            let inside;
            parenthesized!(inside in input);
            let table = inside.parse::<Ident>()?;
            inside.parse::<Token![.]>()?;
            let join_to = inside.parse::<Ident>()?;
            inside.parse::<Token![,]>()?;
            let self_ty = inside.parse::<Type>()?;

            Ok(InputImpl::HasMany(HasMany {
                join_ty,
                join_from,
                table,
                join_to,
                self_ty,
            }))
        } else {
            input.parse::<Token![->]>()?;

            let inside;
            parenthesized!(inside in input);

            let table = inside.parse::<Ident>()?;
            inside.parse::<Token![,]>()?;
            let self_ty = inside.parse::<Type>()?;

            Ok(InputImpl::HasOne(HasOne {
                id_ty,
                table,
                self_ty,
            }))
        }
    }
}

impl InputImpl {
    fn gen_tokens(&self, input: &Input, backend: &Backend, out: &mut TokenStream) {
        match self {
            InputImpl::HasOne(has_one) => has_one.gen_tokens(input, backend, out),
            InputImpl::HasMany(has_many) => has_many.gen_tokens(input, backend, out),
        }
    }
}

impl HasOne {
    fn gen_tokens(&self, input: &Input, backend: &Backend, out: &mut TokenStream) {
        let error_ty = &input.error_ty;
        let context_ty = &input.context_ty;

        let id_ty = &self.id_ty;
        let self_ty = &self.self_ty;
        let table = &self.table;

        let filter = match backend {
            Backend::Pg => {
                quote! {
                    #table::id.eq(diesel::pg::expression::dsl::any(ids))
                }
            }
            Backend::Mysql | Backend::Sqlite => {
                quote! {
                    #table::id.eq_any(ids)
                }
            }
        };

        out.extend(quote! {
            impl juniper_eager_loading::LoadFrom<#id_ty> for #self_ty {
                type Error = #error_ty;
                type Context = #context_ty;

                fn load(
                    ids: &[#id_ty],
                    _field_args: &(),
                    ctx: &Self::Context,
                ) -> Result<Vec<Self>, Self::Error> {
                    #table::table
                    .filter(#filter)
                        .load::<#self_ty>(ctx.db())
                        .map_err(From::from)
                }
            }
        });
    }
}

impl HasMany {
    fn gen_tokens(&self, input: &Input, backend: &Backend, out: &mut TokenStream) {
        let error_ty = &input.error_ty;
        let context_ty = &input.context_ty;

        let join_ty = &self.join_ty;
        let join_from = &self.join_from;
        let table = &self.table;
        let join_to = &self.join_to;
        let self_ty = &self.self_ty;

        let filter = match backend {
            Backend::Pg => {
                quote! {
                    #table::#join_to.eq(diesel::pg::expression::dsl::any(from_ids))
                }
            }
            Backend::Mysql | Backend::Sqlite => {
                quote! {
                    #table::#join_to.eq_any(from_ids)
                }
            }
        };

        out.extend(quote! {
            impl juniper_eager_loading::LoadFrom<#join_ty> for #self_ty {
                type Error = #error_ty;
                type Context = #context_ty;

                fn load(
                    froms: &[#join_ty],
                    _field_args: &(),
                    ctx: &Self::Context,
                ) -> Result<Vec<Self>, Self::Error> {
                    let from_ids = froms
                        .iter()
                        .map(|other| other.#join_from)
                        .collect::<Vec<_>>();

                    #table::table
                        .filter(#filter)
                        .load(ctx.db())
                        .map_err(From::from)
                }
            }
        })
    }
}
