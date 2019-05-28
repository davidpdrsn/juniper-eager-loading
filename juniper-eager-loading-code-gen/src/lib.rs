//! See the docs for "juniper-eager-loading" for more info about this.

#![recursion_limit = "256"]
#![deny(unused_variables, dead_code, unused_must_use, unused_import)]

extern crate proc_macro;
extern crate proc_macro2;

mod derive_eager_loading;
mod load_from_ids;

#[proc_macro_derive(EagerLoading, attributes(eager_loading))]
pub fn derive_eager_loading(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    derive_eager_loading::gen_tokens(input)
}

#[proc_macro_derive(LoadFromIds, attributes(load_from_ids))]
pub fn load_from_ids(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    load_from_ids::gen_tokens(input)
}
