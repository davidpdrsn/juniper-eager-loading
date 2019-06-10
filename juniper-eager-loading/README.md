# [juniper-eager-loading](https://crates.io/crates/juniper-eager-loading)

🚨 **This library is still experimental and everything is subject to change** 🚨

This is a library for avoiding N+1 query bugs designed to work with
[Juniper][] and [juniper-from-schema][].

It is designed to make the most common association setups easy to handle and while being
flexible and allowing you to customize things as needed. It is also 100% data store agnostic.
So regardless if your API is backed by an SQL database or another API you can still use this
library.

See the [crate documentation](https://docs.rs/juniper-eager-loading/) for a usage examples and more info.

[Juniper]: https://github.com/graphql-rust/juniper
[juniper-from-schema]: https://github.com/davidpdrsn/juniper-from-schema
