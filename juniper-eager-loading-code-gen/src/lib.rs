//! See the docs for "juniper-eager-loading" for more info about this.

#![recursion_limit = "256"]
#![deny(unused_variables, dead_code, unused_must_use, unused_imports)]

extern crate proc_macro;
extern crate proc_macro2;

mod derive_eager_loading;

#[proc_macro_derive(
    EagerLoading,
    attributes(eager_loading, has_one, option_has_one, has_many, has_many_through)
)]
pub fn derive_eager_loading(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    derive_eager_loading::gen_tokens(input)
}
