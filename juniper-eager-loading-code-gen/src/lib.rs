//! See the docs for "juniper-eager-loading" for more info about this.

#![recursion_limit = "256"]
// #![deny(
//     unused_variables,
//     mutable_borrow_reservation_conflict,
//     dead_code,
//     unused_must_use,
//     unused_imports
// )]

extern crate proc_macro;
extern crate proc_macro2;

mod derive_eager_loading;
mod impl_load_from_for_diesel;

use impl_load_from_for_diesel::Backend;
use proc_macro_error::*;

#[proc_macro_derive(
    EagerLoading,
    attributes(eager_loading, has_one, option_has_one, has_many, has_many_through)
)]
#[proc_macro_error]
pub fn derive_eager_loading(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    derive_eager_loading::gen_tokens(input)
}

#[proc_macro]
pub fn impl_load_from_for_diesel_pg(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    impl_load_from_for_diesel::go(input, Backend::Pg)
}

#[proc_macro]
pub fn impl_load_from_for_diesel_mysql(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    impl_load_from_for_diesel::go(input, Backend::Mysql)
}

#[proc_macro]
pub fn impl_load_from_for_diesel_sqlite(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    impl_load_from_for_diesel::go(input, Backend::Sqlite)
}
