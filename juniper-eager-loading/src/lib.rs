//! juniper-eager-loading is a library for avoiding N+1 query bugs designed to work with
//! [Juniper][] and [juniper-from-schema][].
//!
//! <center>🚨 **This library is still experimental and everything is subject to change** 🚨</center>
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
//!     // Notice that `Connection` is generic and can be whatever you want.
//!     // This is this library can be data store agnostic.
//!     impl LoadFrom<i32> for Country {
//!         type Error = Box<dyn Error>;
//!         type Connection = super::DbConnection;
//!
//!         fn load(
//!             employments: &[i32],
//!             field_args: &(),
//!             db: &Self::Connection,
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
//! // Our Juniper context type.
//! pub struct Context {
//!     db: DbConnection,
//! }
//!
//! impl juniper::Context for Context {}
//!
//! // Our GraphQL user type.
//! // `#[derive(EagerLoading)]` takes care of all the heavy lifting.
//! #[derive(Clone, EagerLoading)]
//! // You need to set the connection and error type.
//! #[eager_loading(connection = "DbConnection", error = "Box<dyn Error>")]
//! pub struct User {
//!     // This user model is used to resolve `User.id`
//!     user: models::User,
//!
//!     // Setup a "has one" association between a user and a country.
//!     // `default` will use all the default attribute values.
//!     // Exacty what they are is explained below.
//!     #[has_one(default)]
//!     country: HasOne<Country>,
//! }
//!
//! // And the GraphQL country type.
//! #[derive(Clone, EagerLoading)]
//! #[eager_loading(connection = "DbConnection", error = "Box<dyn Error>")]
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
//!         let db = &executor.context().db;
//!         // Load the model users.
//!         let user_models = db.load_all_users();
//!
//!         // Turn the model users into GraphQL users.
//!         let mut users = User::from_db_models(&user_models);
//!
//!         // Perform the eager loading.
//!         // `trail` is used to only eager load the fields that are requested. Because
//!         // we're using `QueryTrail`s from "juniper_from_schema" it would be a compile
//!         // error if we eager loaded too much.
//!         User::eager_load_all_children_for_each(&mut users, &user_models, db, trail)?;
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
//! [`EagerLoadChildrenOfType`]: trait.EagerLoadChildrenOfType.html
//!
//! ## Attributes
//!
//! `#[derive(EagerLoading)]` has a few attributes you need to provide:
//!
//! | Name | Description | Default | Example |
//! |---|---|---|---|
//! | `connection` | The type of connection your app uses. This could be a database connection or a connection to another web service. | N/A | `connection = "diesel::pg::PgConnection"` |
//! | `error` | The type of error eager loading might result in. | N/A | `error = "diesel::result::Error"` |
//! | `model` | The model type behind your GraphQL struct | `models::{name of struct}` | `model = "crate::db::models::User"` |
//! | `id` | Which id type does your app use? | `i32` | `id = "UUID"` |
//! | `root_model_field` | The name of the field has holds the backing model | `{name of struct}` in snakecase. | `root_model_field = "user"` |
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
//! These are the attributes that are supported on all associations. None of these attributes take
//! arguments.
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
//! # Eager loading interfaces or unions
//!
//! Eager loading interfaces or unions is possible but it will require calling `.downcast()` on the
//! `QueryTrail`. See the [juniper-from-schema docs for more
//! info](https://docs.rs/juniper-from-schema/0.4.0/juniper_from_schema/#downcasting-for-interface-and-union-querytrails)
//! fo more info.
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

#![doc(html_root_url = "https://docs.rs/juniper-eager-loading/0.3.1")]
#![allow(clippy::single_match, clippy::type_complexity)]
#![deny(
    // missing_docs,
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

mod macros;

use juniper_from_schema::{QueryTrail, Walked};
use std::{fmt, hash::Hash};

pub use juniper_eager_loading_code_gen::EagerLoading;

/// Re-exports the traits needed for doing eager loading. Meant to be glob imported.
pub mod prelude {
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
/// You can find a complete example of `HasOne` [here](https://github.com/davidpdrsn/juniper-eager-loading/tree/master/juniper-eager-loading/examples/has_one.rs).
///
/// # Attributes
///
/// | Name | Description | Default | Example |
/// |---|---|---|---|
/// | `foreign_key_field` | The name of the foreign key field | `{name of field}_id` | `foreign_key_field = "country_id"` |
/// | `root_model_field` | The name of the field on the associated GraphQL type that holds the database model | `{name of field}` | `root_model_field = "country"` |
/// | `graphql_field` | The name of this field in your GraphQL schema | `{name of field}` | `graphql_field = "country"` |
/// | `default` | Use the default value for all unspecified attributes | N/A | `default` |
///
/// Additionally it also supports the attributes `print`, and `skip`. See the [root model
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

    /// Set the given value as the loaded value.
    pub fn loaded(&mut self, inner: T) {
        self.0.loaded(inner)
    }

    /// Check that a loaded value is present otherwise set `self` to an error state after which
    /// [`try_unwrap`][] will return an error.
    ///
    /// [`try_unwrap`]: struct.HasOne.html#method.try_unwrap
    pub fn assert_loaded_otherwise_failed(&mut self) {
        self.0.assert_loaded_otherwise_failed()
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

    fn loaded(&mut self, inner: T) {
        std::mem::replace(self, HasOneInner::Loaded(inner));
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
/// You can find a complete example of `OptionHasMany` [here](https://github.com/davidpdrsn/juniper-eager-loading/tree/master/juniper-eager-loading/examples/option_has_one.rs).
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

    /// Set the given value as the loaded value.
    pub fn loaded(&mut self, inner: T) {
        std::mem::replace(self, OptionHasOne(Some(inner)));
    }

    /// Check that a loaded value is present otherwise set `self` to `None`.
    pub fn assert_loaded_otherwise_failed(&mut self) {
        match self.0 {
            Some(_) => {}
            None => {
                std::mem::replace(self, OptionHasOne(None));
            }
        }
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
/// You can find a complete example of `HasMany` [here](https://github.com/davidpdrsn/juniper-eager-loading/tree/master/juniper-eager-loading/examples/has_many.rs).
///
/// # Attributes
///
/// | Name | Description | Default | Example |
/// |---|---|---|---|
/// | `foreign_key_field` | The name of the foreign key field | `{name of struct}_id` | `foreign_key_field = "user_id"` |
/// | `foreign_key_optional` | The foreign key type is optional | Not set | `foreign_key_optional` |
/// | `root_model_field` | The name of the field on the associated GraphQL type that holds the database model | N/A (unless using `skip`) | `root_model_field = "car"` |
/// | `graphql_field` | The name of this field in your GraphQL schema | `{name of field}` | `graphql_field = "country"` |
/// | `predicate_method` | Method used to filter child associations. This can be used if you only want to include a subset of the models | N/A (attribute is optional) | `predicate_method = "a_predicate_method"` |
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

    /// Add the loaded value to the list.
    pub fn loaded(&mut self, inner: T) {
        self.0.push(inner);
    }

    /// This function doesn't do anything since the default is an empty list and there is no error
    /// state.
    pub fn assert_loaded_otherwise_failed(&mut self) {}
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
/// You can find a complete example of `HasManyThrough` [here](https://github.com/davidpdrsn/juniper-eager-loading/tree/master/juniper-eager-loading/examples/has_many_through.rs).
///
/// # Attributes
///
/// | Name | Description | Default | Example |
/// |---|---|---|---|
/// | `model_field` | The field on the contained type that holds the model | `{name of contained type}` in snakecase | `model_field = "company"` |
/// | `join_model` | The model we have to join with | N/A | `join_model = "models::Employment"` |
/// | `join_model_field` | The field on the join model type that holds the model | `{name of join model type}` in snakecase | `join_model_field = "employment"` |
/// | `foreign_key_field` | The field on the join model that contains the parent models id | `{name of parent type in lowercase}_id` | `foreign_key_field = "car_id"` |
/// | `graphql_field` | The name of this field in your GraphQL schema | `{name of field}` | `graphql_field = "country"` |
/// | `predicate_method` | Method used to filter child associations. This can be used if you only want to include a subset of the models. This method will be called to filter the join models. | N/A (attribute is optional) | `predicate_method = "a_predicate_method"` |
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

    /// Add the loaded value to the list.
    pub fn loaded(&mut self, inner: T) {
        self.0.push(inner);
    }

    /// This function doesn't do anything since the default is an empty list and there is no error
    /// state.
    pub fn assert_loaded_otherwise_failed(&mut self) {}
}

/// A GraphQL type backed by a model object.
///
/// You shouldn't need to implement this trait yourself even when customizing eager loading.
pub trait GraphqlNodeForModel: Sized {
    /// The model type.
    type Model: Clone;

    /// The id type the model uses.
    type Id: 'static + Hash + Eq;

    /// The connection type required to do the loading. This can be a database connection or maybe
    /// a connection an external web service.
    type Connection;

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
/// #     type Connection = DbConnection;
/// #     fn load(employments: &[i32], field_args: &(), db: &Self::Connection) -> Result<Vec<Self>, Self::Error> {
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
/// #[eager_loading(connection = "DbConnection", error = "Box<dyn std::error::Error>")]
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
/// #[eager_loading(connection = "DbConnection", error = "Box<dyn std::error::Error>")]
/// pub struct Country {
///     country: models::Country,
/// }
///
/// #[allow(missing_docs, dead_code)]
/// struct EagerLoadingContextUserForCountry;
///
/// impl<'look_ahead, 'query_trail>
///     EagerLoadChildrenOfType<
///         'look_ahead,
///         'query_trail,
///         Country,
///         EagerLoadingContextUserForCountry,
///         (),
///     > for User
/// {
///     type ChildId = Option<Self::Id>;
///     type FieldArguments = ();
///
///     fn child_ids(
///         models: &[Self::Model],
///         db: &Self::Connection,
///         field_args: &Self::FieldArguments,
///     ) -> Result<
///         juniper_eager_loading::LoadResult<
///             Self::ChildId,
///             (<Country as GraphqlNodeForModel>::Model, ()),
///         >,
///         Self::Error,
///     > {
///         let ids = models
///             .iter()
///             .map(|model| model.country_id.clone())
///             .collect::<Vec<_>>();
///         let ids = juniper_eager_loading::unique(ids);
///         Ok(juniper_eager_loading::LoadResult::Ids(ids))
///     }
///
///     fn load_children(
///         ids: &[Self::ChildId],
///         db: &Self::Connection,
///         field_args: &Self::FieldArguments,
///     ) -> Result<Vec<<Country as GraphqlNodeForModel>::Model>, Self::Error> {
///         let ids = ids
///             .into_iter()
///             .filter_map(|id| id.as_ref())
///             .cloned()
///             .collect::<Vec<_>>();
///         let ids = juniper_eager_loading::unique(ids);
///         <
///             <Country as GraphqlNodeForModel>::Model as
///                 juniper_eager_loading::LoadFrom<Self::Id, Self::FieldArguments>
///         >::load(&ids, field_args, db)
///     }
///
///     fn is_child_of(node: &Self, child: &(Country, &()), field_args: &Self::FieldArguments) -> bool {
///         node.user.country_id == Some((child.0).country.id)
///     }
///
///     fn loaded_child(node: &mut Self, child: Country) {
///         node.country.loaded(child)
///     }
///
///     fn assert_loaded_otherwise_failed(node: &mut Self) {
///         node.country.assert_loaded_otherwise_failed();
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
/// If model type of the child. If your `User` struct has a field of type `OptionHasOne<Country>`,
/// this type will default to `models::Country`.
///
/// ## `QueryTrailT`
///
/// Since [we cannot depend directly](trait.GenericQueryTrail.html) on [`QueryTrail`][] we have to
/// depend on this generic version instead.
///
/// The generic constraint enforces that [`.walk()`][] must to have been called on the `QueryTrail` to
/// ensure the field we're trying to eager load is actually part of the incoming GraphQL query.
/// Otherwise the field will not be eager loaded. This is how the compiler can guarantee that we
/// don't eager load too much.
///
/// [`QueryTrail`]: https://docs.rs/juniper-from-schema/#query-trails
/// [`.walk()`]: https://docs.rs/juniper-from-schema/#k
///
/// ## `Context`
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
pub trait EagerLoadChildrenOfType<'look_ahead, 'query_trail, Child, Context, JoinModel = ()>
where
    Self: GraphqlNodeForModel,
    Child: GraphqlNodeForModel<Connection = Self::Connection, Error = Self::Error>
        + EagerLoadAllChildren
        + Clone,
    JoinModel: 'static + Clone + ?Sized,
{
    /// The id type the child uses. This will be different for the different [association types][].
    ///
    /// [association types]: /#associations
    type ChildId: Hash + Eq;

    type FieldArguments;

    /// Given a list of models, load either the list of child ids or child models associated.
    fn child_ids(
        models: &[Self::Model],
        db: &Self::Connection,
        field_args: &Self::FieldArguments,
    ) -> Result<LoadResult<Self::ChildId, (Child::Model, JoinModel)>, Self::Error>;

    /// Load a list of children from a list of ids.
    fn load_children(
        ids: &[Self::ChildId],
        db: &Self::Connection,
        field_args: &Self::FieldArguments,
    ) -> Result<Vec<Child::Model>, Self::Error>;

    /// Does this parent and this child belong together?
    fn is_child_of(parent: &Self, child: &(Child, &JoinModel), field_args: &Self::FieldArguments) -> bool;

    /// Store the loaded child on the association.
    fn loaded_child(node: &mut Self, child: Child);

    /// The association should have been loaded by now, if not store an error inside the
    /// association (if applicable for the particular association).
    fn assert_loaded_otherwise_failed(node: &mut Self);

    /// Combine all the methods above to eager load the children for a list of GraphQL values and
    /// models.
    fn eager_load_children(
        nodes: &mut [Self],
        models: &[Self::Model],
        db: &Self::Connection,
        trail: &QueryTrail<'look_ahead, Child, Walked>,
        field_args: &Self::FieldArguments,
    ) -> Result<(), Self::Error> {
        let child_models = match Self::child_ids(models, db, field_args)? {
            LoadResult::Ids(child_ids) => {
                assert!(same_type::<JoinModel, ()>());

                let loaded_models = Self::load_children(&child_ids, db, field_args)?;
                loaded_models
                    .into_iter()
                    .map(|model| {
                        #[allow(unsafe_code)]
                        let join_model = unsafe {
                            // This branch will only ever be called if `JoinModel` is `()`. That
                            // happens for all the `Has*` types except `HasManyThrough`.
                            //
                            // `HasManyThrough` requires something to join the two types on,
                            // therefore `child_ids` will return a variant of `LoadResult::Models`
                            std::mem::transmute_copy::<(), JoinModel>(&())
                        };

                        (model, join_model)
                    })
                    .collect::<Vec<_>>()
            }
            LoadResult::Models(model_and_join_pairs) => model_and_join_pairs,
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
            db,
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
                .filter(|child_model| Self::is_child_of(node, child_model, field_args))
                .cloned()
                .collect::<Vec<_>>();

            for child in matching_children {
                Self::loaded_child(node, child.0);
            }

            Self::assert_loaded_otherwise_failed(node);
        }

        Ok(())
    }
}

/// Are two types the same?
fn same_type<A: 'static, B: 'static>() -> bool {
    use std::any::TypeId;
    TypeId::of::<A>() == TypeId::of::<B>()
}

/// The result of loading child ids.
///
/// [`HasOne`][] and [`OptionHasOne`][] can return the child ids because the model has the foreign
/// key. However for [`HasMany`][] and [`HasManyThrough`][] the model itself doesn't have the
/// foreign key, the join models do. So we have the return those instead.
///
/// Unless you're customizing [`EagerLoadChildrenOfType`] you shouldn't have to worry about this.
///
/// [`HasOne`]: struct.HasOne.html
/// [`OptionHasOne`]: struct.OptionHasOne.html
/// [`HasMany`]: struct.HasMany.html
/// [`HasManyThrough`]: struct.HasManyThrough.html
/// [`EagerLoadChildrenOfType`]: trait.EagerLoadChildrenOfType.html
#[derive(Debug)]
pub enum LoadResult<A, B> {
    /// Ids where loaded.
    Ids(Vec<A>),

    /// Models were loaded.
    Models(Vec<B>),
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
        db: &Self::Connection,
        trail: &QueryTrail<'_, Self, Walked>,
    ) -> Result<(), Self::Error>;

    /// Perform eager loading for a single GraphQL value.
    ///
    /// This is the function you should call for eager loading associations of a single value.
    fn eager_load_all_children(
        node: Self,
        models: &[Self::Model],
        db: &Self::Connection,
        trail: &QueryTrail<'_, Self, Walked>,
    ) -> Result<Self, Self::Error> {
        let mut nodes = vec![node];
        Self::eager_load_all_children_for_each(&mut nodes, models, db, trail)?;

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
/// TODO: document Args
///
/// [`HasMany`]: struct.HasMany.html
/// [`HasManyThrough`]: struct.HasManyThrough.html
pub trait LoadFrom<T, Args = ()>: Sized {
    /// The error type. This must match the error set in `#[eager_loading(error_type = _)]`.
    type Error;

    /// The connection type required to do the loading. This can be a database connection or maybe
    /// a connection an external web service.
    type Connection;

    /// Perform the load.
    fn load(ids: &[T], args: &Args, db: &Self::Connection) -> Result<Vec<Self>, Self::Error>;
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
        // t.compile_fail("tests/compile_fail/*.rs");
    }
}
