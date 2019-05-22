//! See the docs for "juniper-eager-loading" for more info about this.

#![recursion_limit = "128"]

extern crate proc_macro;
extern crate proc_macro2;

mod derive_eager_loading;

#[proc_macro_derive(EagerLoading, attributes(eager_loading))]
pub fn derive_eager_loading(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    derive_eager_loading::gen_tokens(input)
}
