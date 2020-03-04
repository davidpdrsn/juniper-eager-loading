//! juniper-eager-loading is a library for avoiding N+1 query bugs designed to work with
//! [Juniper][] and [juniper-from-schema][].
//!
//! It is designed to make the most common assocation setups easy to handle and while being
//! flexible and allowing you to customize things as needed. It is also 100% data store agnostic.
//! So regardless if your API is backed by an SQL database or another API you can still use this
//! library.
//!
//! If you're familiar with N+1 queries in GraphQL and eager loading, feel free to skip forward to
//! ["A real example"](#a-real-example).
//!
//! *NOTE*: Since this library requires [juniper-from-schema][] it is best if you're first familiar
//! with that.
//!
//! # Table of contents
//!
//! - [What is N+1 query bugs?](#what-is-n1-query-bugs)
//!     - [N+1s in GraphQL](#n1s-in-graphql)
//! - [How this library works at a high level](#how-this-library-works-at-a-high-level)
//! - [A real example](#a-real-example)
//! - [`#[derive(EagerLoading)]`](#deriveeagerloading)
//!     - [Attributes](#attributes)
//! - [Associations](#associations)
//!     - [Attributes supported on all associations](#attributes-supported-on-all-associations)
//! - [Eager loading interfaces or unions](#eager-loading-interfaces-or-unions)
//! - [Eager loading fields that take arguments](#eager-loading-fields-that-take-arguments)
//! - [Diesel helper](#diesel-helper)
//! - [When your GraphQL schema doesn't match your database schema](#when-your-graphql-schema-doesnt-match-your-database-schema)
//!
//! # What is N+1 query bugs?
//!
//! Imagine you have the following GraphQL schema
//!
//! ```graphql
//! schema {
//!     query: Query
//! }
//!
//! type Query {
//!     allUsers: [User!]!
//! }
//!
//! type User {
//!     id: Int!
//!     country: Country!
//! }
//!
//! type Country {
//!     id: Int!
//! }
//! ```
//!
//! And someone executes the following query:
//!
//! ```graphql
//! query SomeQuery {
//!     allUsers {
//!         country {
//!             id
//!         }
//!     }
//! }
//! ```
//!
//! If you resolve that query naively with an SQL database as you data store you will see something
//! like this in your logs:
//!
//! ```sql
//! select * from users
//! select * from countries where id = ?
//! select * from countries where id = ?
//! select * from countries where id = ?
//! select * from countries where id = ?
//! ...
//! ```
//!
//! This happens because you first load all the users and then for each user in a loop you load
//! that user's country. That is 1 query to load the users and N additional queries to load the
//! countries. Therefore the name "N+1 query". These kinds of bugs can really hurt performance of
//! your app since you're doing many more database calls than necessary.
//!
//! One possible solution to this is called "eager loading". The idea is to load all countries up
//! front, before looping over the users. So instead of doing N+1 queries you do 2:
//!
//! ```sql
//! select * from users
//! select * from countries where id in (?, ?, ?, ?)
//! ```
//!
//! Since you're loading the countries up front, this strategy is called "eager loading".
//!
//! ## N+1s in GraphQL
//!
//! If you're not careful when implementing a GraphQL API you'll have lots of these N+1 query bugs.
//! Whenever a field returns a list of types and those types perform queries in their resolvers,
//! you'll have N+1 query bugs.
//!
//! This is also a problem in REST APIs, however because the responses are fixed we can more easily
//! setup the necessary eager loads because we know the types needed to compute the response.
//!
//! However in GraphQL the responses are not fixed. They depend on the incoming queries, which are
//! not known ahead of time. So setting up the correct amount of eager loading requires inspecting
//! the queries before executing them and eager loading the types requested such that the actual
//! resolvers wont need to run queries. That is exactly what this library does.
//!
//! # How this library works at a high level
//!
//! If you have a GraphQL type like this
//!
//! ```graphql
//! type User {
//!     id: Int!
//!     country: Country!
//! }
//! ```
//!
//! You might create the corresponding Rust model type like this:
//!
//! ```
//! struct User {
//!     id: i32,
//!     country_id: i32
//! }
//! ```
//!
//! However this approach has one big issue. How are you going to resolve the field `User.country`
//! without doing a database query? All the resolver has access to is a `User` with a `country_id`
//! field. It can't get the country without loading it from the database...
//!
//! Fundamentally these kinds of model structs don't work for eager loading with GraphQL. So
//! this library takes a different approach.
//!
//! What if we created separate structs for the database models and the GraphQL models? Something
//! like this:
//!
//! ```
//! # fn main() {}
//! #
//! mod models {
//!     pub struct User {
//!         id: i32,
//!         country_id: i32
//!     }
//!
//!     pub struct Country {
//!         id: i32,
//!     }
//! }
//!
//! struct User {
//!     user: models::User,
//!     country: HasOne<Country>,
//! }
//!
//! struct Country {
//!     country: models::Country
//! }
//!
//! enum HasOne<T> {
//!     Loaded(T),
//!     NotLoaded,
//! }
//! ```
//!
//! Now we're able to resolve the query with code like this:
//!
//! 1. Load all the users (first query).
//! 2. Map the users to a list of country ids.
//! 3. Load all the countries with those ids (second query).
//! 4. Pair up the users with the country with the correct id, so change `User.country` from
//!    `HasOne::NotLoaded` to `HasOne::Loaded(matching_country)`.
//! 5. When resolving the GraphQL field `User.country` simply return the loaded country.
//!
//! # A real example
//!
//! ```
//! use juniper::{Executor, FieldResult};
//! use juniper_eager_loading::{prelude::*, EagerLoading, HasOne};
//! use juniper_from_schema::graphql_schema;
//! use std::error::Error;
//!
//! // Define our GraphQL schema.
//! graphql_schema! {
//!     schema {
//!         query: Query
//!     }
//!
//!     type Query {
//!         allUsers: [User!]! @juniper(ownership: "owned")
//!     }
//!
//!     type User {
//!         id: Int!
//!         country: Country!
//!     }
//!
//!     type Country {
//!         id: Int!
//!     }
//! }
//!
//! // Our model types.
//! mod models {
//!     use std::error::Error;
//!     use juniper_eager_loading::LoadFrom;
//!
//!     #[derive(Clone)]
//!     pub struct User {
//!         pub id: i32,
//!         pub country_id: i32
//!     }
//!
//!     #[derive(Clone)]
//!     pub struct Country {
//!         pub id: i32,
//!     }
//!
//!     // This trait is required for eager loading countries.
//!     // It defines how to load a list of countries from a list of ids.
//!     // Notice that `Context` is generic and can be whatever you want.
//!     // It will normally be your Juniper context which would contain
//!     // a database connection.
//!     impl LoadFrom<i32> for Country {
//!         type Error = Box<dyn Error>;
//!         type Context = super::Context;
//!
//!         fn load(
//!             employments: &[i32],
//!             field_args: &(),
//!             ctx: &Self::Context,
//!         ) -> Result<Vec<Self>, Self::Error> {
//!             // ...
//!             # unimplemented!()
//!         }
//!     }
//! }
//!
//! // Our sample database connection type.
//! pub struct DbConnection;
//!
//! impl DbConnection {
//!     // Function that will load all the users.
//!     fn load_all_users(&self) -> Vec<models::User> {
//!         // ...
//!         # unimplemented!()
//!     }
//! }
//!
//! // Our Juniper context type which contains a database connection.
//! pub struct Context {
//!     db: DbConnection,
//! }
//!
//! impl juniper::Context for Context {}
//!
//! // Our GraphQL user type.
//! // `#[derive(EagerLoading)]` takes care of generating all the boilerplate code.
//! #[derive(Clone, EagerLoading)]
//! // You need to set the context and error type.
//! #[eager_loading(
//!     context = Context,
//!     error = Box<dyn Error>,
//!
//!     // These match the default so you wouldn't have to specify them
//!     model = models::User,
//!     id = i32,
//!     root_model_field = user,
//! )]
//! pub struct User {
//!     // This user model is used to resolve `User.id`
//!     user: models::User,
//!
//!     // Setup a "has one" association between a user and a country.
//!     //
//!     // We could also have used `#[has_one(default)]` here.
//!     #[has_one(
//!         foreign_key_field = country_id,
//!         root_model_field = country,
//!         graphql_field = country,
//!     )]
//!     country: HasOne<Country>,
//! }
//!
//! // And the GraphQL country type.
//! #[derive(Clone, EagerLoading)]
//! #[eager_loading(context = Context, error = Box<dyn Error>)]
//! pub struct Country {
//!     country: models::Country,
//! }
//!
//! // The root query GraphQL type.
//! pub struct Query;
//!
//! impl QueryFields for Query {
//!     // The resolver for `Query.allUsers`.
//!     fn field_all_users(
//!         &self,
//!         executor: &Executor<'_, Context>,
//!         trail: &QueryTrail<'_, User, Walked>,
//!     ) -> FieldResult<Vec<User>> {
//!         let ctx = executor.context();
//!
//!         // Load the model users.
//!         let user_models = ctx.db.load_all_users();
//!
//!         // Turn the model users into GraphQL users.
//!         let mut users = User::from_db_models(&user_models);
//!
//!         // Perform the eager loading.
//!         // `trail` is used to only eager load the fields that are requested. Because
//!         // we're using `QueryTrail`s from "juniper_from_schema" it would be a compile
//!         // error if we eager loaded associations that aren't requested in the query.
//!         User::eager_load_all_children_for_each(&mut users, &user_models, ctx, trail)?;
//!
//!         Ok(users)
//!     }
//! }
//!
//! impl UserFields for User {
//!     fn field_id(
//!         &self,
//!         executor: &Executor<'_, Context>,
//!     ) -> FieldResult<&i32> {
//!         Ok(&self.user.id)
//!     }
//!
//!     fn field_country(
//!         &self,
//!         executor: &Executor<'_, Context>,
//!         trail: &QueryTrail<'_, Country, Walked>,
//!     ) -> FieldResult<&Country> {
//!         // This will unwrap the country from the `HasOne` or return an error if the
//!         // country wasn't loaded, or wasn't found in the database.
//!         Ok(self.country.try_unwrap()?)
//!     }
//! }
//!
//! impl CountryFields for Country {
//!     fn field_id(
//!         &self,
//!         executor: &Executor<'_, Context>,
//!     ) -> FieldResult<&i32> {
//!         Ok(&self.country.id)
//!     }
//! }
//! #
//! # fn main() {}
//! ```
//!
//! # `#[derive(EagerLoading)]`
//!
//! For a type to support eager loading it needs to implement the following traits:
//!
//! - [`GraphqlNodeForModel`][]
//! - [`EagerLoadAllChildren`][]
//! - Each association field must implement [`EagerLoadChildrenOfType`][]
//!
//! [`GraphqlNodeForModel`]: trait.GraphqlNodeForModel.html
//! [`EagerLoadAllChildren`]: trait.EagerLoadAllChildren.html
//!
//! Implementing these traits involves lots of boilerplate, therefore you should use
//! `#[derive(EagerLoading)]` to derive implementations as much as possible.
//!
//! Sometimes you might need customized eager loading for a specific association, in that case you
//! should still have `#[derive(EagerLoading)]` on your struct but implement
//! [`EagerLoadChildrenOfType`][] yourself for the field that requires a custom setup. An example
//! of how to do that can be found
//! [here](trait.EagerLoadChildrenOfType.html#manual-implementation).
//!
//! If you're interested in seeing full examples without any macros look
//! [here](https://github.com/davidpdrsn/juniper-eager-loading/tree/master/examples).
//!
//! [`EagerLoadChildrenOfType`]: trait.EagerLoadChildrenOfType.html
//!
//! ## Attributes
//!
//! `#[derive(EagerLoading)]` has a few attributes you need to provide:
//!
//! | Name | Description | Default | Example |
//! |---|---|---|---|
//! | `context` | The type of your Juniper context. This will often hold your database connection or something else than can be used to load data. | N/A | `context = Context` |
//! | `error` | The type of error eager loading might result in. | N/A | `error = diesel::result::Error` |
//! | `model` | The model type behind your GraphQL struct | `models::{name of struct}` | `model = crate::db::models::User` |
//! | `id` | Which id type does your app use? | `i32` | `id = UUID` |
//! | `root_model_field` | The name of the field has holds the backing model | `{name of struct}` in snakecase. | `root_model_field = user` |
//! | `primary_key_field` | The field that holds the primary key of the model. This field is only used by code generated for `#[has_many]` and `#[has_many_through]` associations. | `id` | `primary_key_field = identifier` |
//! | `print` | If set it will print the generated implementation of `GraphqlNodeForModel` and `EagerLoadAllChildren` | Not set | `print` |
//!
//! # Associations
//!
//! Assocations are things like "user has one country". These are the fields that need to be eager
//! loaded to avoid N+1s. Each assocation works for different kinds of foreign key setups and has
//! to be eager loaded differently. They should fit most kinds of associations you have in your
//! app. Click on each for more detail.
//!
//! The documation for each assocation assumes that you're using an SQL database, but it should be
//! straight forward to adapt to other kinds of data stores.
//!
//! - [`HasOne`](struct.HasOne.html)
//! - [`OptionHasOne`](struct.OptionHasOne.html)
//! - [`HasMany`](struct.HasMany.html)
//! - [`HasManyThrough`](struct.HasManyThrough.html)
//!
//! For each field of your GraphQL struct that is one of these four types the trait
//! [`EagerLoadChildrenOfType`][] will be implemented by `#[derive(EagerLoading)]`.
//!
//! ## Attributes supported on all associations
//!
//! These are the attributes that are supported on all associations.
//!
//! ### `skip`
//!
//! Skip implementing [`EagerLoadChildrenOfType`][] for the field. This is useful if you need to
//! provide a custom implementation.
//!
//! ### `print`
//!
//! This will cause the implementation of [`EagerLoadChildrenOfType`][] for the field to be printed
//! while compiling. This is useful when combined with `skip`. It will print a good starting place
//! for you to customize.
//!
//! The resulting code wont be formatted. We recommend you do that with
//! [rustfmt](https://github.com/rust-lang/rustfmt).
//!
//! ### `fields_arguments`
//!
//! Used to specify the type that'll be use for [`EagerLoadChildrenOfType::FieldArguments`][]. More
//! info [here](#eager-loading-fields-that-take-arguments).
//!
//! For example `#[has_one(fields_arguments = CountryUsersArgs)]`. You can find a complete example
//! [here](https://github.com/davidpdrsn/juniper-eager-loading/tree/master/examples/field_with_arguments.rs).
//!
//! The code generation defaults [`EagerLoadChildrenOfType::FieldArguments`][] to `()`. That works
//! for fields that don't take arguments.
//!
//! [`EagerLoadChildrenOfType::FieldArguments`]: trait.EagerLoadChildrenOfType.html#associatedtype.FieldArguments
//!
//! # Eager loading interfaces or unions
//!
//! Eager loading interfaces or unions is possible but it will require calling `.downcast()` on the
//! `QueryTrail`. See the [juniper-from-schema docs for more
//! info](https://docs.rs/juniper-from-schema/0.4.0/juniper_from_schema/#downcasting-for-interface-and-union-querytrails)
//! fo more info.
//!
//! # Eager loading fields that take arguments
//!
//! If you have a GraphQL field that takes arguments you probably have to consider them for eager
//! loading purposes.
//!
//! If you're using on code generation for such fields you have to specify the type on the
//! association field. More into [here](/#fields_arguments).
//!
//! If you implement [`EagerLoadChildrenOfType`][] manually you have to set
//! [`EagerLoadChildrenOfType::FieldArguments`][] to the type of the arguments struct generated by
//! juniper-from-schema. You can find more info
//! [here](https://docs.rs/juniper-from-schema/0.5.0/juniper_from_schema/#querytrails-for-fields-that-take-arguments).
//!
//! You also have to implement [`LoadFrom<T, ArgumentType>`][`LoadFrom`] for your model. You can find a complete
//! example
//! [here](https://github.com/davidpdrsn/juniper-eager-loading/tree/master/examples/field_with_arguments.rs).
//!
//! If you see a type error like:
//!
//! ```text
//! error[E0308]: mismatched types
//!    --> src/main.rs:254:56
//!     |
//!    254 | #[derive(Clone, Eq, PartialEq, Debug, Ord, PartialOrd, EagerLoading)]
//!     |                                                           ^^^^^^^^^^^^ expected (), found struct `query_trails::CountryUsersArgs`
//!     |
//!     = note: expected type `&()`
//!                found type `&query_trails::CountryUsersArgs<'_>`
//! ```
//!
//! It is because your GraphQL field `Country.users` takes arguments. The code generation
//! defaults to using `()` for the type of the arguments so therefore you get this type error. The
//! neat bit is that the compiler wont let you forget about handling arguments.
//!
//! [`EagerLoadChildrenOfType`]: trait.EagerLoadChildrenOfType.html
//! [`EagerLoadChildrenOfType::FieldArguments`]: trait.EagerLoadChildrenOfType.html#associatedtype.FieldArguments
//! [`LoadFrom`]: trait.LoadFrom.html
//!
//! # Diesel helper
//!
//! Implementing [`LoadFrom`][] for lots of model types might involve lots of boilerplate. If
//! you're using Diesel it is recommend that you use one of [the macros to
//! generate](index.html#macros) implementations.
//!
//! [`LoadFrom`]: trait.LoadFrom.html
//! [Diesel]: https://diesel.rs
//! [`EagerLoadChildrenOfType`]: trait.EagerLoadChildrenOfType.html
//!
//! # When your GraphQL schema doesn't match your database schema
//!
//! This library supports eager loading most kinds of association setups, however it probably
//! doesn't support all that might exist in your app. It also works best when your database schema
//! closely matches your GraphQL schema.
//!
//! If you find yourself having to implement something that isn't directly supported remember that
//! you're still free to implement you resolver functions exactly as you want. So if doing queries
//! in a resolver is the only way to get the behaviour you need then so be it. Avoiding some N+1
//! queries is better than avoiding none.
//!
//! However if you have a setup that you think this library should support please don't hestitate
//! to [open an issue](https://github.com/davidpdrsn/juniper-eager-loading).
//!
//! [Juniper]: https://github.com/graphql-rust/juniper
//! [juniper-from-schema]: https://github.com/davidpdrsn/juniper-from-schema

#![doc(html_root_url = "https://docs.rs/juniper-eager-loading/0.5.0")]
#![allow(clippy::single_match, clippy::type_complexity)]
#![deny(
    missing_docs,
    dead_code,
    missing_copy_implementations,
    missing_debug_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unstable_features,
    unused_import_braces,
    unused_imports,
    unused_must_use,
    unused_qualifications,
    unused_variables
)]

mod association;
mod macros;

use juniper_from_schema::{QueryTrail, Walked};
use std::{fmt, hash::Hash, mem::transmute_copy};

pub use association::Association;
pub use juniper_eager_loading_code_gen::EagerLoading;

#[doc(hidden)]
pub mod proc_macros {
    pub use juniper_eager_loading_code_gen::{
        impl_load_from_for_diesel_mysql, impl_load_from_for_diesel_pg,
        impl_load_from_for_diesel_sqlite,
    };
}

/// Re-exports the traits needed for doing eager loading. Meant to be glob imported.
pub mod prelude {
    pub use super::Association;
    pub use super::EagerLoadAllChildren;
    pub use super::EagerLoadChildrenOfType;
    pub use super::GraphqlNodeForModel;
}

/// The types of associations.
///
/// This is used for [`Error`] to report which kind of association encountered an error.
///
/// [`Error`]: enum.Error.html
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum AssociationType {
    /// There was an error with a [`HasOne`](struct.HasOne.html).
    HasOne,
    /// There was an error with an [`OptionHasOne`](struct.OptionHasOne.html).
    OptionHasOne,
    /// There was an error with a [`HasMany`](struct.HasMany.html).
    HasMany,
    /// There was an error with a [`HasManyThrough`](struct.HasManyThrough.html).
    HasManyThrough,
}

/// A non-optional "has one" association.
///
/// Imagine you have these models:
///
/// ```
/// struct User {
///     id: i32,
///     country_id: i32,
/// }
///
/// struct Country {
///     id: i32,
/// }
/// ```
///
/// For this setup we say "a user has one country". This means that `User` has a field named
/// `country_id` that references the id of another country.
///
/// # Example
///
/// You can find a complete example of `HasOne` [here](https://github.com/davidpdrsn/juniper-eager-loading/tree/master/examples/has_one.rs).
///
/// # Attributes
///
/// | Name | Description | Default | Example |
/// |---|---|---|---|
/// | `foreign_key_field` | The name of the foreign key field | `{name of field}_id` | `foreign_key_field = country_id` |
/// | `root_model_field` | The name of the field on the associated GraphQL type that holds the model | `{name of field}` | `root_model_field = country` |
/// | `graphql_field` | The name of this field in your GraphQL schema | `{name of field}` | `graphql_field = country` |
/// | `child_primary_key_field` | The name of the primary key field on the associated model | `id` | `child_primary_key_field = identifier` |
/// | `default` | Use the default value for all unspecified attributes | N/A | `default` |
///
/// Additionally it also supports the attributes `print`, `skip`, and `field_arguments`. See the [root model
/// docs](/#attributes-supported-on-all-associations) for more into on those.
///
/// # Errors
///
/// When calling [`try_unwrap`][] to get the loaded value it will return an error if the value has
/// not been loaded, or if the load failed.
///
/// For example if a user has a `country_id` of `10` but there is no `Country` with id `10` then
/// [`try_unwrap`][] will return an error.
///
/// [`try_unwrap`]: struct.HasOne.html#method.try_unwrap
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct HasOne<T>(HasOneInner<T>);

impl<T> Default for HasOne<T> {
    fn default() -> Self {
        HasOne(HasOneInner::default())
    }
}

impl<T> HasOne<T> {
    /// Borrow the loaded value. If the value has not been loaded it will return an error.
    pub fn try_unwrap(&self) -> Result<&T, Error> {
        self.0.try_unwrap()
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
enum HasOneInner<T> {
    Loaded(T),
    NotLoaded,
    LoadFailed,
}

impl<T> Default for HasOneInner<T> {
    fn default() -> Self {
        HasOneInner::NotLoaded
    }
}

impl<T> HasOneInner<T> {
    fn try_unwrap(&self) -> Result<&T, Error> {
        match self {
            HasOneInner::Loaded(inner) => Ok(inner),
            HasOneInner::NotLoaded => Err(Error::NotLoaded(AssociationType::HasOne)),
            HasOneInner::LoadFailed => Err(Error::LoadFailed(AssociationType::HasOne)),
        }
    }

    fn assert_loaded_otherwise_failed(&mut self) {
        match self {
            HasOneInner::NotLoaded => {
                std::mem::replace(self, HasOneInner::LoadFailed);
            }
            _ => {}
        }
    }
}

/// An optional "has-one association".
///
/// It works exactly like [`HasOne`] except it doesn't error if the association doesn't get loaded.
/// The value doesn't get loaded it defaults to `None`.
///
/// # Example
///
/// You can find a complete example of `OptionHasMany` [here](https://github.com/davidpdrsn/juniper-eager-loading/tree/master/examples/option_has_one.rs).
///
/// # Attributes
///
/// It supports the same attributes as [`HasOne`].
///
/// [`HasOne`]: struct.HasOne.html
///
/// # Errors
///
/// [`try_unwrap`][] will never error. If the association wasn't loaded or wasn't found it will
/// return `Ok(None)`.
///
/// [`try_unwrap`]: struct.OptionHasOne.html#method.try_unwrap
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct OptionHasOne<T>(Option<T>);

impl<T> Default for OptionHasOne<T> {
    fn default() -> Self {
        OptionHasOne(None)
    }
}

impl<T> OptionHasOne<T> {
    /// Borrow the loaded value. If the value has not been loaded it will return `Ok(None)`. It
    /// will not error.
    pub fn try_unwrap(&self) -> Result<&Option<T>, Error> {
        Ok(&self.0)
    }
}

/// A "has many" association.
///
/// Imagine you have these models:
///
/// ```
/// struct User {
///     id: i32,
/// }
///
/// struct Car {
///     id: i32,
///     user_id: i32,
/// }
/// ```
///
/// For this setup we say "user has many cars" and "cars have one user". This is the inverse of a
/// `HasOne` assocation because the foreign key is on `Car` instead of `User`.
///
/// This means users can own many cars, but cars can only be owned by one user.
///
/// # Example
///
/// You can find a complete example of `HasMany` [here](https://github.com/davidpdrsn/juniper-eager-loading/tree/master/examples/has_many.rs).
///
/// # Attributes
///
/// | Name | Description | Default | Example |
/// |---|---|---|---|
/// | `foreign_key_field` | The name of the foreign key field | `{name of struct}_id` | `foreign_key_field = user_id` |
/// | `foreign_key_optional` | The foreign key type is optional | Not set | `foreign_key_optional` |
/// | `root_model_field` | The name of the field on the associated GraphQL type that holds the database model | N/A (unless using `skip`) | `root_model_field = car` |
/// | `graphql_field` | The name of this field in your GraphQL schema | `{name of field}` | `graphql_field = country` |
/// | `predicate_method` | Method used to filter child associations. This can be used if you only want to include a subset of the models | N/A (attribute is optional) | `predicate_method = a_predicate_method` |
///
/// Additionally it also supports the attributes `print`, `skip`, and `field_arguments`. See the [root model
/// docs](/#attributes-supported-on-all-associations) for more into on those.
///
/// # Errors
///
/// [`try_unwrap`][] will never error. If the association wasn't loaded or wasn't found it will
/// return `Ok(vec![])`.
///
/// [`try_unwrap`]: struct.HasMany.html#method.try_unwrap
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct HasMany<T>(Vec<T>);

impl<T> Default for HasMany<T> {
    fn default() -> Self {
        HasMany(Vec::new())
    }
}

impl<T> HasMany<T> {
    /// Borrow the loaded values. If no values have been loaded it will return an empty list.
    /// It will not return an error.
    pub fn try_unwrap(&self) -> Result<&Vec<T>, Error> {
        Ok(&self.0)
    }
}

/// A "has many through" association.
///
/// Imagine you have these models:
///
/// ```
/// struct User {
///     id: i32,
/// }
///
/// struct Company {
///     id: i32,
/// }
///
/// struct Employments {
///     id: i32,
///     user_id: i32,
///     company_id: i32,
/// }
/// ```
///
/// For this setup we say "user has many companies through employments". This means uses can work
/// at many companies and companies can have many employees, provided that we join with `Employment`.
///
/// This requires that we use [the `JoinModel`](trait.EagerLoadChildrenOfType.html#joinmodel) type
/// on [`EagerLoadChildrenOfType`][] and is therefore a bit different from the other associations
/// since it involves a third type.
///
/// [`EagerLoadChildrenOfType`]: trait.EagerLoadChildrenOfType.html
///
/// # Example
///
/// You can find a complete example of `HasManyThrough` [here](https://github.com/davidpdrsn/juniper-eager-loading/tree/master/examples/has_many_through.rs).
///
/// # Attributes
///
/// | Name | Description | Default | Example |
/// |---|---|---|---|
/// | `model_field` | The field on the contained type that holds the model | `{name of contained type}` in snakecase | `model_field = company` |
/// | `join_model` | The model we have to join with | N/A | `join_model = models::Employment` |
/// | `child_primary_key_field_on_join_model` | The field on the join model that holds the primary key of the child model (`Company` in the example above) | `{name of model}_id` | `child_primary_key_field_on_join_model = company_identifier` |
/// | `foreign_key_field` | The field on the join model that holds the primary key of the parent model (`User` in the example above) | `{name of model}_id` | `foreign_key_field = user_identifier` |
/// | `child_primary_key_field` | The field on the child model that holds its primary key | `id` | `foreign_key_field = identifier` |
/// | `graphql_field` | The name of this field in your GraphQL schema | `{name of field}` | `graphql_field = country` |
/// | `predicate_method` | Method used to filter child associations. This can be used if you only want to include a subset of the models. This method will be called to filter the join models. | N/A (attribute is optional) | `predicate_method = a_predicate_method` |
///
/// Additionally it also supports the attributes `print`, `skip`, and `field_arguments`. See the [root model
/// docs](/#attributes-supported-on-all-associations) for more into on those.
///
/// # Errors
///
/// [`try_unwrap`][] will never error. If the association wasn't loaded or wasn't found it will
/// return `Ok(vec![])`.
///
/// [`try_unwrap`]: struct.HasManyThrough.html#method.try_unwrap
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct HasManyThrough<T>(Vec<T>);

impl<T> Default for HasManyThrough<T> {
    fn default() -> Self {
        HasManyThrough(Vec::new())
    }
}

impl<T> HasManyThrough<T> {
    /// Borrow the loaded values. If no values have been loaded it will return an empty list.
    /// It will not return an error.
    pub fn try_unwrap(&self) -> Result<&Vec<T>, Error> {
        Ok(&self.0)
    }
}

/// A GraphQL type backed by a model object.
///
/// You shouldn't need to implement this trait yourself even when customizing eager loading.
pub trait GraphqlNodeForModel: Sized {
    /// The model type.
    type Model: Clone;

    /// The id type the model uses.
    type Id: 'static + Hash + Eq;

    /// Your Juniper context type.
    ///
    /// This will typically contain a database connection or a connection to some external API.
    type Context;

    /// The error type.
    type Error;

    /// Create a new GraphQL type from a model.
    fn new_from_model(model: &Self::Model) -> Self;

    /// Create a list of GraphQL types from a list of models.
    fn from_db_models(models: &[Self::Model]) -> Vec<Self> {
        models
            .iter()
            .map(|model| Self::new_from_model(model))
            .collect()
    }
}

/// Perform eager loading for a single association of a GraphQL struct.
///
/// `#[derive(EagerLoading)]` will implement this trait for each [association field][] your GraphQL
/// struct has.
///
/// [association field]: /#associations
///
/// # Manual implementation
///
/// Sometimes you might have a setup that `#[derive(EagerLoading)]` doesn't support. In those cases
/// you have to implement this trait yourself for those struct fields. Here is an example of how to
/// do that:
///
/// ```
/// # use juniper::{Executor, FieldResult};
/// # use juniper_eager_loading::{prelude::*, *};
/// # use juniper_from_schema::graphql_schema;
/// # use std::error::Error;
/// # pub struct Query;
/// # impl QueryFields for Query {
/// #     fn field_noop(&self, executor: &Executor<'_, Context>) -> FieldResult<bool> {
/// #         unimplemented!()
/// #     }
/// # }
/// # impl juniper_eager_loading::LoadFrom<i32> for models::Country {
/// #     type Error = Box<dyn std::error::Error>;
/// #     type Context = Context;
/// #     fn load(employments: &[i32], field_args: &(), ctx: &Self::Context) -> Result<Vec<Self>, Self::Error> {
/// #         unimplemented!()
/// #     }
/// # }
/// # pub struct DbConnection;
/// # impl DbConnection {
/// #     fn load_all_users(&self) -> Vec<models::User> {
/// #         unimplemented!()
/// #     }
/// # }
/// # pub struct Context {
/// #     db: DbConnection,
/// # }
/// # impl juniper::Context for Context {}
/// # impl UserFields for User {
/// #     fn field_id(&self, executor: &Executor<'_, Context>) -> FieldResult<&i32> {
/// #         unimplemented!()
/// #     }
/// #     fn field_country(
/// #         &self,
/// #         executor: &Executor<'_, Context>,
/// #         trail: &QueryTrail<'_, Country, Walked>,
/// #     ) -> FieldResult<&Option<Country>> {
/// #         unimplemented!()
/// #     }
/// # }
/// # impl CountryFields for Country {
/// #     fn field_id(&self, executor: &Executor<'_, Context>) -> FieldResult<&i32> {
/// #         unimplemented!()
/// #     }
/// # }
/// # fn main() {}
/// #
/// # graphql_schema! {
/// #     schema { query: Query }
/// #     type Query { noop: Boolean! @juniper(ownership: "owned") }
/// #     type User {
/// #         id: Int!
/// #         country: Country
/// #     }
/// #     type Country {
/// #         id: Int!
/// #     }
/// # }
/// # mod models {
/// #     #[derive(Clone)]
/// #     pub struct User {
/// #         pub id: i32,
/// #         pub country_id: Option<i32>,
/// #     }
/// #     #[derive(Clone)]
/// #     pub struct Country {
/// #         pub id: i32,
/// #     }
/// # }
/// #
/// #[derive(Clone, EagerLoading)]
/// #[eager_loading(context = Context, error = Box<dyn std::error::Error>)]
/// pub struct User {
///     user: models::User,
///
///     // Add `#[option_has_one(default, print)]` to get a good starting point for your
///     // manual implementaion.
///     #[option_has_one(skip)]
///     country: OptionHasOne<Country>,
/// }
///
/// #[derive(Clone, EagerLoading)]
/// #[eager_loading(context = Context, error = Box<dyn std::error::Error>)]
/// pub struct Country {
///     country: models::Country,
/// }
///
/// #[allow(missing_docs, dead_code)]
/// struct EagerLoadingContextUserForCountry;
///
/// impl<'a>
///     EagerLoadChildrenOfType<
///         'a,
///         Country,
///         EagerLoadingContextUserForCountry,
///     > for User
/// {
///     type FieldArguments = ();
///
///     fn load_children(
///         models: &[Self::Model],
///         field_args: &Self::FieldArguments,
///         ctx: &Self::Context,
///     ) -> Result<
///         LoadChildrenOutput<<Country as juniper_eager_loading::GraphqlNodeForModel>::Model>,
///         Self::Error,
///     > {
///         let ids = models
///             .iter()
///             .filter_map(|model| model.country_id)
///             .map(|id| id.clone())
///             .collect::<Vec<_>>();
///         let ids = juniper_eager_loading::unique(ids);
///
///         let children = <
///             <Country as GraphqlNodeForModel>::Model as juniper_eager_loading::LoadFrom<Self::Id>
///         >::load(&ids, field_args, ctx)?;
///
///         Ok(juniper_eager_loading::LoadChildrenOutput::ChildModels(children))
///     }
///
///     fn is_child_of(
///         node: &Self,
///         child: &Country,
///         _join_model: &(), _field_args: &Self::FieldArguments,
///         _ctx: &Self::Context,
///     ) -> bool {
///         node.user.country_id == Some(child.country.id)
///     }
///
///     fn association(node: &mut Self) -> &mut dyn Association<Country> {
///         &mut node.country
///     }
/// }
/// ```
///
/// # Generic parameters
///
/// The number of generic parameters to this trait might look scary, but in the vast majority of
/// cases you shouldn't have to worry about them.
///
/// ## `Child`
///
/// Is the model type of the child. If your `User` struct has a field of type `OptionHasOne<Country>`,
/// this type will default to `models::Country`.
///
/// ## `ImplContext`
///
/// This "context" type is needed in case your GraphQL type has multiple assocations to values
/// of the same type. Could for example be something like this
///
/// ```ignore
/// struct User {
///     home_country: HasOne<Country>,
///     current_country: HasOne<Country>,
/// }
/// ```
///
/// If we didn't have this we wouldn't be able to implement `EagerLoadChildrenOfType<Country>`
/// twice for `User`, because you cannot implement the same trait twice for the same type.
///
/// Note that this is _not_ the Juniper GraphQL context.
///
/// ## `JoinModel`
///
/// This type defaults to `()` and is only need for [`HasManyThrough`][]. In the other associations
/// there are only two types involved (such as `models::User` and `models::Country`) and one of
/// them will have a foreign key pointing to the other one. But consider this scenario instead
/// where users can work for many companies, and companies can have many employees:
///
/// ```
/// mod models {
///     struct User {
///         id: i32,
///     }
///
///     struct Company {
///         id: i32,
///     }
///
///     struct Employment {
///         id: i32,
///         user_id: i32,
///         company_id: i32,
///     }
/// }
/// ```
///
/// Imagine now we need to eager load the list of companies a given user works at. That means
/// [`LoadFrom`][] would return `Vec<models::Company>`. However that isn't enough information once
/// we need to pair users up with the correct companies. `User` doesn't have `company_id` and
/// `Company` doesn't have `user_id`.
///
/// Instead we need [`LoadFrom`] to return `Vec<(models::Company, models::Employment)>`. We say
/// "users have many companies through employments", because `models::Employment` is necessary for
/// pairing things up at the end of [`EagerLoadChildrenOfType`][].
///
/// In this case `JoinModel` would be `models::Employment`.
///
/// [`HasManyThrough`]: struct.HasManyThrough.html
/// [`LoadFrom`]: trait.LoadFrom.html
/// [`EagerLoadChildrenOfType`]: trait.EagerLoadChildrenOfType.html
// `JoinModel` cannot be an associated type because it requires a default.
pub trait EagerLoadChildrenOfType<'a, Child, ImplContext, JoinModel = ()>
where
    Self: GraphqlNodeForModel,
    Child: GraphqlNodeForModel<Context = Self::Context, Error = Self::Error>
        + EagerLoadAllChildren
        + Clone,
    JoinModel: 'static + Clone + ?Sized,
{
    /// The types of arguments the GraphQL field takes. The type used by the code generation can be
    /// customized with [`field_arguments = SomeType`][].
    ///
    /// [`field_arguments = SomeType`]: index.html#fields_arguments
    type FieldArguments;

    /// Load the children from the data store.
    fn load_children(
        models: &[Self::Model],
        field_args: &Self::FieldArguments,
        ctx: &Self::Context,
    ) -> Result<LoadChildrenOutput<Child::Model, JoinModel>, Self::Error>;

    /// Does this parent and this child belong together?
    ///
    /// The `join_model` is only used for `HasManyThrough` associations.
    fn is_child_of(
        parent: &Self,
        child: &Child,
        join_model: &JoinModel,
        field_args: &Self::FieldArguments,
        context: &Self::Context,
    ) -> bool;

    /// Return the particular association type.
    ///
    /// In most cases the implementation will be something like
    ///
    /// ```ignore
    /// fn association(node: &mut User) -> &mut dyn Association<Country> {
    ///     &mut node.country
    /// }
    /// ```
    fn association(node: &mut Self) -> &mut dyn Association<Child>;

    /// Combine all the methods above to eager load the children for a list of GraphQL values and
    /// models.
    fn eager_load_children(
        nodes: &mut [Self],
        models: &[Self::Model],
        ctx: &Self::Context,
        trail: &QueryTrail<'a, Child, Walked>,
        field_args: &Self::FieldArguments,
    ) -> Result<(), Self::Error> {
        let child_models = match Self::load_children(models, field_args, ctx)? {
            LoadChildrenOutput::ChildModels(child_models) => {
                assert!(same_type::<JoinModel, ()>());

                child_models
                    .into_iter()
                    .map(|model| {
                        #[allow(unsafe_code)]
                        let join_model = unsafe {
                            // This branch will only ever be called if `JoinModel` is `()`. That
                            // happens for all the `Has*` types except `HasManyThrough`.
                            //
                            // `HasManyThrough` requires something to join the two types on,
                            // therefore `child_ids` will return a variant of `LoadChildrenOutput::Models`
                            transmute_copy::<(), JoinModel>(&())
                        };

                        (model, join_model)
                    })
                    .collect::<Vec<_>>()
            }
            LoadChildrenOutput::ChildAndJoinModels(model_and_join_pairs) => model_and_join_pairs,
        };

        let children = child_models
            .iter()
            .map(|child_model| (Child::new_from_model(&child_model.0), child_model.1.clone()))
            .collect::<Vec<_>>();

        let mut children_without_join_models =
            children.iter().map(|x| x.0.clone()).collect::<Vec<_>>();

        let child_models_without_join_models =
            child_models.iter().map(|x| x.0.clone()).collect::<Vec<_>>();

        let len_before = child_models_without_join_models.len();

        Child::eager_load_all_children_for_each(
            &mut children_without_join_models,
            &child_models_without_join_models,
            ctx,
            trail,
        )?;

        assert_eq!(len_before, child_models_without_join_models.len());

        let children = children_without_join_models
            .into_iter()
            .enumerate()
            .map(|(idx, child)| {
                let join_model = &children[idx].1;
                (child, join_model)
            })
            .collect::<Vec<_>>();

        for node in nodes {
            let matching_children = children
                .iter()
                .filter(|child_model| {
                    Self::is_child_of(node, &child_model.0, &child_model.1, field_args, ctx)
                })
                .cloned()
                .collect::<Vec<_>>();

            for child in matching_children {
                Self::association(node).loaded_child(child.0);
            }

            Self::association(node).assert_loaded_otherwise_failed();
        }

        Ok(())
    }
}

/// Are two types the same?
fn same_type<A: 'static, B: 'static>() -> bool {
    use std::any::TypeId;
    TypeId::of::<A>() == TypeId::of::<B>()
}

/// The result of loading child models.
///
/// [`HasOne`][], [`OptionHasOne`][], [`HasMany`][] can return the child models directly because
/// the model has the foreign key. However for [`HasManyThrough`][] neither the parent or child
/// model has any of the foreign keys. Only the join model does. So we have to include those in the
/// result.
///
/// Unless you're customizing [`EagerLoadChildrenOfType`] you shouldn't have to worry about this.
///
/// [`HasOne`]: struct.HasOne.html
/// [`OptionHasOne`]: struct.OptionHasOne.html
/// [`HasMany`]: struct.HasMany.html
/// [`HasManyThrough`]: struct.HasManyThrough.html
/// [`EagerLoadChildrenOfType`]: trait.EagerLoadChildrenOfType.html
#[derive(Debug)]
pub enum LoadChildrenOutput<ChildModel, JoinModel = ()> {
    /// Child models were loaded.
    ChildModels(Vec<ChildModel>),

    /// Child models along with the respective join model was loaded.
    ChildAndJoinModels(Vec<(ChildModel, JoinModel)>),
}

/// The main entry point trait for doing eager loading.
///
/// You shouldn't need to implement this trait yourself even when customizing eager loading.
pub trait EagerLoadAllChildren
where
    Self: GraphqlNodeForModel,
{
    /// For each field in your GraphQL type that implements [`EagerLoadChildrenOfType`][] call
    /// [`eager_load_children`][] to do eager loading of that field.
    ///
    /// This is the function you should call for eager loading values for a GraphQL field that returns
    /// a list.
    ///
    /// [`EagerLoadChildrenOfType`]: trait.EagerLoadChildrenOfType.html
    /// [`eager_load_children`]: trait.EagerLoadChildrenOfType.html#method.eager_load_children
    fn eager_load_all_children_for_each(
        nodes: &mut [Self],
        models: &[Self::Model],
        ctx: &Self::Context,
        trail: &QueryTrail<'_, Self, Walked>,
    ) -> Result<(), Self::Error>;

    /// Perform eager loading for a single GraphQL value.
    ///
    /// This is the function you should call for eager loading associations of a single value.
    fn eager_load_all_children(
        node: Self,
        models: &[Self::Model],
        ctx: &Self::Context,
        trail: &QueryTrail<'_, Self, Walked>,
    ) -> Result<Self, Self::Error> {
        let mut nodes = vec![node];
        Self::eager_load_all_children_for_each(&mut nodes, models, ctx, trail)?;

        // This is safe because we just made a vec with exactly one element and
        // eager_load_all_children_for_each doesn't remove things from the vec
        Ok(nodes.remove(0))
    }
}

/// How should associated values actually be loaded?
///
/// Normally `T` will be your id type but for [`HasMany`][] and [`HasManyThrough`][] it might also
/// be other values.
///
/// If you're using Diesel it is recommend that you use one of [the macros to
/// generate](index.html#macros) implementations.
///
/// `Args` is the type of arguments your GraphQL field takes. This is how we're able to load things
/// differently depending the types of arguments. You can learn more
/// [here](index.html#eager-loading-fields-that-take-arguments).
///
/// [`HasMany`]: struct.HasMany.html
/// [`HasManyThrough`]: struct.HasManyThrough.html
pub trait LoadFrom<T, Args = ()>: Sized {
    /// The error type. This must match the error set in `#[eager_loading(error_type = _)]`.
    type Error;

    /// Your Juniper context type.
    ///
    /// This will typically contain a database connection or a connection to some external API.
    type Context;

    /// Perform the load.
    fn load(ids: &[T], args: &Args, context: &Self::Context) -> Result<Vec<Self>, Self::Error>;
}

/// The kinds of errors that can happen when doing eager loading.
#[derive(Debug)]
#[allow(missing_copy_implementations)]
pub enum Error {
    /// The association was not loaded.
    ///
    /// Did you forget to call
    /// [`eager_load_all_children_for_each`](trait.EagerLoadAllChildren.html#tymethod.eager_load_all_children_for_each)?
    NotLoaded(AssociationType),

    /// Loading the association failed. This can only happen when using
    /// [`HasOne`](struct.HasOne.html). All the other association types have defaults.
    LoadFailed(AssociationType),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::NotLoaded(kind) => {
                write!(f, "`{:?}` should have been eager loaded, but wasn't", kind)
            }
            Error::LoadFailed(kind) => write!(f, "Failed to load `{:?}`", kind),
        }
    }
}

impl std::error::Error for Error {}

/// Remove duplicates from a list.
///
/// This function is used to remove duplicate ids from
/// [`child_ids`](trait.EagerLoadChildrenOfType.html#tymethod.child_ids).
pub fn unique<T: Hash + Eq>(items: Vec<T>) -> Vec<T> {
    use std::collections::HashSet;

    items
        .into_iter()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
}

#[cfg(test)]
mod test {
    #[test]
    fn ui() {
        let t = trybuild::TestCases::new();
        t.pass("tests/compile_pass/*.rs");

        // We currently don't have any compile tests that should fail to build
        // t.compile_fail("tests/compile_fail/*.rs");
    }
}
