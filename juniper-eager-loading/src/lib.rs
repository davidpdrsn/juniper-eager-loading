//! juniper-eager-loading is a library for avoiding N+1 query bugs designed to work with
//! [Juniper][] and [juniper-from-schema][].
//!
//! It is designed to make the most common assocation setups easy to handle and while being
//! flexible and allowing you to customize things as needed. It is also 100% data store agnostic.
//! So regardless if your API is backed by an SQL database or another API you can still use this
//! library.
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
//! front, before looping over the users. So instead of N+1 queries you get 2:
//!
//! ```sql
//! select * from users
//! select * from countries where id in (?, ?, ?, ?)
//! ```
//!
//! Since you're loading the countries up front, this strategy is called "eager loading".
//!
//! # N+1s in GraphQL
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
//! resolvers wont need to run queries.
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
//! You might create the corresponding Rust model object like this:
//!
//! ```
//! struct User {
//!     id: i32,
//!     country_id: i32
//! }
//! ```
//!
//! However this approach has one big issue. How are you going to resolve the field `User.country`
//! without doing database queries? All the resolver has access to is a `User` with a `country_id`
//! field. It can't get the country without loading it from the database...
//!
//! Fundamentally these kinds of model structs don't work well for eager loading with GraphQL. So
//! this library takes a different approach. What if we created separate structs for the database
//! models and the GraphQL models? Something like this:
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
//! mod graphql {
//!     use super::models;
//!
//!     struct User {
//!         user: models::User,
//!         country: HasOne<Country>,
//!     }
//!
//!     struct Country {
//!         country: models::Country
//!     }
//!
//!     enum HasOne<T> {
//!         Loaded(T),
//!         NotLoaded,
//!     }
//! }
//! ```
//!
//! Now we're able to resolve the query with code like so:
//!
//! 1. Load all the users (first query).
//! 2. Map the users to a list of country ids.
//! 3. Load all the countries with those ids (second query).
//! 4. Zip the users with the country with the correct id, so change `User.country` from
//!    `HasOne::NotLoaded` to `HasOne::Loaded(matching_country)`.
//! 5. When resolving the GraphQL field `User.country` simply return the loaded country.
//!
//! # A real example
//!
//! Since this library requires [juniper-from-schema][] it is best if you're first familiar with
//! that.
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
//!     // Exacty that they are is explained below.
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
//! - `GraphqlNodeForModel`
//! - `EagerLoadAllChildren`
//! - Each association field must implement `EagerLoadChildrenOfType`
//!
//! Implementing these traits involves lots of boilerplate, therefore you should use
//! `#[derive(EagerLoading)]` to derive implementations as much as possible.
//!
//! Sometimes you might need customized eager loading for a specific association, in that case you
//! should still have `#[derive(EagerLoading)]` on your struct but implement
//! `EagerLoadChildrenOfType` yourself for the field that requires a custom setup. An example of
//! how to do that can be found [here](TODO).
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
//! `EagerLoadChildrenOfType` will be implemented by `#[derive(EagerLoading)]`.
//!
//! ## Attributes supported on all associations
//!
//! Theser are the attributes that are supported on all associations. None of these attributes take
//! arguments.
//!
//! ### `default`
//!
//! Use the default values for all attributes not provided. For example `#[has_one(default)]`.
//!
//! ### `skip`
//!
//! Skip implementing `EagerLoadChildrenOfType` for the field. This is useful if you need to
//! provide a custom implementation.
//!
//! ### `print`
//!
//! This will cause the implementation of `EagerLoadChildrenOfType` for the field to be printed
//! while compiling. This is useful when combined with `skip`. It will print a good starting place
//! for you to customize.
//!
//! The resulting code wont be formatted. We recommend you do that with
//! [rustfmt](https://github.com/rust-lang/rustfmt).
//!
//! [Juniper]: https://github.com/graphql-rust/juniper
//! [juniper-from-schema]: https://github.com/davidpdrsn/juniper-from-schema

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
    unused_variables,
)]

use juniper_from_schema::Walked;
use std::{fmt, hash::Hash};

pub use juniper_eager_loading_code_gen::EagerLoading;

/// Re-exports the traits needed for doing eager loading. Meant to be glob imported.
pub mod prelude {
    pub use super::EagerLoadAllChildren;
    pub use super::EagerLoadChildrenOfType;
    pub use super::GraphqlNodeForModel;
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum AssociationType {
    HasOne,
    OptionHasOne,
    HasMany,
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
/// ```
/// # use juniper::{Executor, FieldResult};
/// # use juniper_eager_loading::{prelude::*, EagerLoading, HasOne};
/// # use juniper_from_schema::graphql_schema;
/// # use std::error::Error;
/// # graphql_schema! {
/// #     schema { query: Query }
/// #     type Query { noop: Boolean! @juniper(ownership: "owned") }
/// #     type User {
/// #         id: Int!
/// #         country: Country!
/// #     }
/// #     type Country {
/// #         id: Int!
/// #     }
/// # }
/// # pub struct Query;
/// # impl QueryFields for Query {
/// #     fn field_noop(
/// #         &self,
/// #         executor: &Executor<'_, Context>,
/// #     ) -> FieldResult<bool> {
/// #         unimplemented!()
/// #     }
/// # }
/// # impl juniper_eager_loading::LoadFrom<i32> for models::Country {
/// #     type Error = Box<dyn std::error::Error>;
/// #     type Connection = DbConnection;
/// #     fn load(
/// #         employments: &[i32],
/// #         db: &Self::Connection,
/// #     ) -> Result<Vec<Self>, Self::Error> {
/// #         unimplemented!()
/// #     }
/// # }
/// # pub struct DbConnection;
/// # impl DbConnection {
/// #     fn load_all_users(&self) -> Vec<models::User> {
/// #         unimplemented!()
/// #     }
/// # }
/// # pub struct Context { db: DbConnection }
/// # impl juniper::Context for Context {}
/// # impl UserFields for User {
/// #     fn field_id(
/// #         &self,
/// #         executor: &Executor<'_, Context>,
/// #     ) -> FieldResult<&i32> {
/// #         unimplemented!()
/// #     }
/// #     fn field_country(
/// #         &self,
/// #         executor: &Executor<'_, Context>,
/// #         trail: &QueryTrail<'_, Country, Walked>,
/// #     ) -> FieldResult<&Country> {
/// #         unimplemented!()
/// #     }
/// # }
/// # impl CountryFields for Country {
/// #     fn field_id(
/// #         &self,
/// #         executor: &Executor<'_, Context>,
/// #     ) -> FieldResult<&i32> {
/// #         unimplemented!()
/// #     }
/// # }
/// # mod models {
/// #     #[derive(Clone)]
/// #     pub struct User {
/// #         pub id: i32,
/// #         pub country_id: i32
/// #     }
/// #     #[derive(Clone)]
/// #     pub struct Country {
/// #         pub id: i32,
/// #     }
/// # }
/// #
/// # fn main() {}
/// #
/// #[derive(Clone, EagerLoading)]
/// #[eager_loading(connection = "DbConnection", error = "Box<dyn std::error::Error>")]
/// pub struct User {
///     user: models::User,
///
///     // these are the defaults. `#[has_one(default)]` would also work here.
///     #[has_one(
///         foreign_key_field = "country_id",
///         model = "models::Country",
///         root_model_field = "country",
///         graphql_field = "country",
///     )]
///     country: HasOne<Country>,
/// }
///
/// #[derive(Clone, EagerLoading)]
/// #[eager_loading(connection = "DbConnection", error = "Box<dyn std::error::Error>")]
/// pub struct Country {
///     country: models::Country,
/// }
/// ```
///
/// # Attributes
///
/// | Name | Description | Default | Example |
/// |---|---|---|---|
/// | `foreign_key_field` | The name of the foreign key field | `{name of field}_id` | `foreign_key_field = "country_id"` |
/// | `model` | The database model type | `models::{name of contained type}` | `model = "models::Country"` |
/// | `root_model_field` | The name of the field on the associated GraphQL type that hold the database model | `{name of field}` | `root_model_field = "country"` |
/// | `graphql_field` | The name of this field in your GraphQL schema | `{name of field}` | `graphql_field = "country"` |
///
/// Additionally it also supports the attributes `default`, `print`, and `skip`. See the [root
/// model docs](/#attributes-supported-on-all-associations) for more into on those.
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

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct OptionHasOne<T>(Option<T>);

impl<T> Default for OptionHasOne<T> {
    fn default() -> Self {
        OptionHasOne(None)
    }
}

impl<T> OptionHasOne<T> {
    pub fn try_unwrap(&self) -> Result<&Option<T>, Error> {
        Ok(&self.0)
    }

    pub fn loaded(&mut self, inner: T) {
        std::mem::replace(self, OptionHasOne(Some(inner)));
    }

    pub fn assert_loaded_otherwise_failed(&mut self) {
        match self.0 {
            Some(_) => {}
            None => {
                std::mem::replace(self, OptionHasOne(None));
            }
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct HasMany<T>(Vec<T>);

impl<T> Default for HasMany<T> {
    fn default() -> Self {
        HasMany(Vec::new())
    }
}

impl<T> HasMany<T> {
    pub fn try_unwrap(&self) -> Result<&Vec<T>, Error> {
        Ok(&self.0)
    }

    pub fn loaded(&mut self, inner: T) {
        self.0.push(inner);
    }

    pub fn assert_loaded_otherwise_failed(&mut self) {}
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct HasManyThrough<T>(Vec<T>);

impl<T> Default for HasManyThrough<T> {
    fn default() -> Self {
        HasManyThrough(Vec::new())
    }
}

impl<T> HasManyThrough<T> {
    pub fn try_unwrap(&self) -> Result<&Vec<T>, Error> {
        Ok(&self.0)
    }

    pub fn loaded(&mut self, inner: T) {
        self.0.push(inner);
    }

    pub fn assert_loaded_otherwise_failed(&mut self) {}
}

pub trait GraphqlNodeForModel: Sized {
    type Model;
    type Id: 'static + Hash + Eq;
    type Connection;
    type Error;

    fn new_from_model(model: &Self::Model) -> Self;

    fn from_db_models(models: &[Self::Model]) -> Vec<Self> {
        models
            .iter()
            .map(|model| Self::new_from_model(model))
            .collect::<Vec<_>>()
    }
}

pub trait GenericQueryTrail<T, K> {}

pub trait EagerLoadChildrenOfType<Child, QueryTrailT, Context, JoinModel = ()>
where
    Self: GraphqlNodeForModel,
    Child: GraphqlNodeForModel<
            Model = Self::ChildModel,
            Connection = Self::Connection,
            Error = Self::Error,
            Id = Self::Id,
        > + EagerLoadAllChildren<QueryTrailT>
        + Clone,
    QueryTrailT: GenericQueryTrail<Child, Walked>,
    JoinModel: 'static + Clone + ?Sized,
{
    type ChildModel: Clone;
    type ChildId: Hash + Eq;

    fn child_ids(
        models: &[Self::Model],
        db: &Self::Connection,
    ) -> Result<LoadResult<Self::ChildId, (Self::ChildModel, JoinModel)>, Self::Error>;

    fn load_children(
        ids: &[Self::ChildId],
        db: &Self::Connection,
    ) -> Result<Vec<Self::ChildModel>, Self::Error>;

    fn is_child_of(node: &Self, child: &(Child, &JoinModel)) -> bool;

    fn loaded_or_failed_child(node: &mut Self, child: Child);

    fn assert_loaded_otherwise_failed(node: &mut Self);

    fn eager_load_children(
        nodes: &mut [Self],
        models: &[Self::Model],
        db: &Self::Connection,
        trail: &QueryTrailT,
    ) -> Result<(), Self::Error> {
        let child_models = match Self::child_ids(models, db)? {
            LoadResult::Ids(child_ids) => {
                assert!(same_type::<JoinModel, ()>());

                let loaded_models = Self::load_children(&child_ids, db)?;
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
                .filter(|child_model| Self::is_child_of(node, child_model))
                .cloned()
                .collect::<Vec<_>>();

            for child in matching_children {
                Self::loaded_or_failed_child(node, child.0);
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

#[derive(Debug)]
pub enum LoadResult<A, B> {
    Ids(Vec<A>),
    Models(Vec<B>),
}

pub trait EagerLoadAllChildren<QueryTrailT>
where
    Self: GraphqlNodeForModel,
{
    fn eager_load_all_children_for_each(
        nodes: &mut [Self],
        models: &[Self::Model],
        db: &Self::Connection,
        trail: &QueryTrailT,
    ) -> Result<(), Self::Error>;

    fn eager_load_all_chilren(
        node: Self,
        models: &[Self::Model],
        db: &Self::Connection,
        trail: &QueryTrailT,
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
/// If you're using Diesel it is recommend that you use the macro [`impl_LoadFrom_for_diesel`][] to
/// generate implementations.
///
/// [`HasMany`]: struct.HasMany.html
/// [`HasManyThrough`]: struct.HasManyThrough.html
/// [`impl_LoadFrom_for_diesel`]: TODO
pub trait LoadFrom<T>: Sized {
    /// The error type. This must match the error set in `#[eager_loading(error_type = _)]`.
    type Error;

    /// The connection type required to do the loading. This can be a database connection or maybe
    /// a connection an external web service.
    type Connection;

    /// Perform the load.
    fn load(ids: &[T], db: &Self::Connection) -> Result<Vec<Self>, Self::Error>;
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
