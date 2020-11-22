#![allow(unused_variables, unused_imports, dead_code, unused_mut)]
#![allow(clippy::type_complexity)]

mod helpers;

use assert_json_diff::{assert_json_eq, assert_json_include};
use helpers::{SortedExtension, StatsHash};
use juniper::{Executor, FieldError, FieldResult};
use juniper_eager_loading::{
    prelude::*, EagerLoading, HasMany, HasManyThrough, HasOne, LoadChildrenOutput, LoadFrom,
    OptionHasOne,
};
use juniper_from_schema::graphql_schema;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{borrow::Borrow, collections::HashMap, hash::Hash};

graphql_schema! {
    schema {
      query: Query
      mutation: Mutation
    }

    type Query {
      countries: [Country!]! @juniper(ownership: "owned")
    }

    type Mutation {
      noop: Boolean!
    }

    type User {
        id: Int!
        isAdmin: Boolean!
        country: Country!
    }

    type Country {
        id: Int!
        users(onlyAdmins: Boolean!): [User!]!
    }
}

mod models {
    use super::*;
    use either::Either;

    #[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
    pub struct User {
        pub id: i32,
        pub country_id: i32,
        pub admin: bool,
    }

    #[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
    pub struct Country {
        pub id: i32,
    }

    impl juniper_eager_loading::LoadFrom<i32> for Country {
        type Error = Box<dyn std::error::Error>;
        type Context = super::Context;

        fn load(ids: &[i32], _: &(), ctx: &Self::Context) -> Result<Vec<Self>, Self::Error> {
            let mut models = ctx
                .db
                .countries
                .all_values()
                .into_iter()
                .filter(|value| ids.contains(&value.id))
                .cloned()
                .collect::<Vec<_>>();
            models.sort_by_key(|model| model.id);
            Ok(models)
        }
    }

    impl juniper_eager_loading::LoadFrom<i32, CountryUsersArgs<'_>> for User {
        type Error = Box<dyn std::error::Error>;
        type Context = super::Context;

        fn load(
            ids: &[i32],
            field_args: &CountryUsersArgs,
            ctx: &Self::Context,
        ) -> Result<Vec<Self>, Self::Error> {
            let models = ctx
                .db
                .users
                .all_values()
                .into_iter()
                .filter(|value| ids.contains(&value.id));

            let mut models = if field_args.only_admins() {
                Either::Left(models.filter(|user| user.admin))
            } else {
                Either::Right(models)
            }
            .cloned()
            .collect::<Vec<_>>();

            models.sort_by_key(|model| model.id);

            Ok(models)
        }
    }

    impl juniper_eager_loading::LoadFrom<i32> for User {
        type Error = Box<dyn std::error::Error>;
        type Context = super::Context;

        fn load(ids: &[i32], _: &(), ctx: &Self::Context) -> Result<Vec<Self>, Self::Error> {
            let mut models = ctx
                .db
                .users
                .all_values()
                .into_iter()
                .filter(|value| ids.contains(&value.id))
                .cloned()
                .collect::<Vec<_>>();
            models.sort_by_key(|model| model.id);
            Ok(models)
        }
    }

    impl juniper_eager_loading::LoadFrom<Country> for User {
        type Error = Box<dyn std::error::Error>;
        type Context = super::Context;

        fn load(
            countries: &[Country],
            _: &(),
            ctx: &Self::Context,
        ) -> Result<Vec<Self>, Self::Error> {
            let country_ids = countries.iter().map(|c| c.id).collect::<Vec<_>>();
            let mut models = ctx
                .db
                .users
                .all_values()
                .into_iter()
                .filter(|user| country_ids.contains(&user.country_id))
                .cloned()
                .collect::<Vec<_>>();
            models.sort_by_key(|model| model.id);
            Ok(models)
        }
    }

    impl LoadFrom<Country, CountryUsersArgs<'_>> for User {
        type Error = Box<dyn std::error::Error>;
        type Context = super::Context;

        fn load(
            countries: &[Country],
            args: &CountryUsersArgs,
            ctx: &Self::Context,
        ) -> Result<Vec<Self>, Self::Error> {
            let only_admins = args.only_admins();

            let country_ids = countries.iter().map(|c| c.id).collect::<Vec<_>>();

            let models = ctx
                .db
                .users
                .all_values()
                .into_iter()
                .filter(|user| country_ids.contains(&user.country_id));

            let models = if only_admins {
                Either::Left(models.filter(|user| user.admin))
            } else {
                Either::Right(models)
            };

            let mut models = models.cloned().collect::<Vec<_>>();
            models.sort_by_key(|model| model.id);
            Ok(models)
        }
    }
}

pub struct Db {
    users: StatsHash<i32, models::User>,
    countries: StatsHash<i32, models::Country>,
}

pub struct Context {
    db: Db,
}

impl juniper::Context for Context {}

pub struct Query;

impl QueryFields for Query {
    fn field_countries(
        &self,
        executor: &Executor<'_, Context>,
        trail: &QueryTrail<'_, Country, Walked>,
    ) -> FieldResult<Vec<Country>> {
        let ctx = executor.context();

        let mut country_models = ctx
            .db
            .countries
            .all_values()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        country_models.sort_by_key(|country| country.id);

        let countries = Country::eager_load_each(&country_models, ctx, trail)?;

        Ok(countries)
    }
}

pub struct Mutation;

impl MutationFields for Mutation {
    fn field_noop(&self, _executor: &Executor<'_, Context>) -> FieldResult<&bool> {
        Ok(&true)
    }
}

// The default values are commented out
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug, EagerLoading)]
#[eager_loading(
    model = models::User,
    id = i32,
    context = Context,
    error = Box<dyn std::error::Error>,
)]
pub struct User {
    user: models::User,

    #[has_one(default)]
    country: HasOne<Country>,
}

impl UserFields for User {
    fn field_id(&self, _executor: &Executor<'_, Context>) -> FieldResult<&i32> {
        Ok(&self.user.id)
    }

    fn field_is_admin(&self, _executor: &Executor<'_, Context>) -> FieldResult<&bool> {
        Ok(&self.user.admin)
    }

    fn field_country(
        &self,
        _executor: &Executor<'_, Context>,
        _trail: &QueryTrail<'_, Country, Walked>,
    ) -> FieldResult<&Country> {
        Ok(self.country.try_unwrap()?)
    }
}

#[derive(Clone, Eq, PartialEq, Debug, Ord, PartialOrd, EagerLoading)]
#[eager_loading(
    model = models::Country,
    id = i32,
    context = Context,
    error = Box<dyn std::error::Error>,
)]
pub struct Country {
    country: models::Country,

    #[has_many(skip)]
    users: HasMany<User>,
}

#[allow(missing_docs, dead_code)]
struct EagerLoadingContextCountryForUsers;

impl<'a> EagerLoadChildrenOfType<'a, User, EagerLoadingContextCountryForUsers, ()> for Country {
    type FieldArguments = CountryUsersArgs<'a>;

    fn load_children(
        models: &[Self::Model],
        field_args: &Self::FieldArguments,
        ctx: &Self::Context,
    ) -> Result<
        LoadChildrenOutput<<User as juniper_eager_loading::EagerLoading>::Model, ()>,
        Self::Error,
    > {
        let children = LoadFrom::load(&models, field_args, ctx)?;
        Ok(LoadChildrenOutput::ChildModels(children))
    }

    fn is_child_of(
        node: &Self,
        child: &User,
        _join_model: &(),
        _field_args: &Self::FieldArguments,
        _ctx: &Self::Context,
    ) -> bool {
        node.country.id == child.user.country_id
    }

    fn association(node: &mut Country) -> &mut dyn Association<User> {
        &mut node.users
    }
}

impl CountryFields for Country {
    fn field_users(
        &self,
        executor: &Executor<'_, Context>,
        trail: &QueryTrail<'_, User, Walked>,
        _only_admins: bool,
    ) -> FieldResult<&Vec<User>> {
        Ok(self.users.try_unwrap()?)
    }

    fn field_id(&self, _executor: &Executor<'_, Context>) -> FieldResult<&i32> {
        Ok(&self.country.id)
    }
}

#[test]
fn loading_user() {
    let mut countries = StatsHash::new("countries");
    let mut users = StatsHash::new("users");

    let mut country = models::Country { id: 10 };
    let country_id = country.id;

    countries.insert(country_id, country.clone());

    let bob = models::User {
        id: 1,
        country_id,
        admin: true,
    };
    let alice = models::User {
        id: 2,
        country_id,
        admin: false,
    };
    users.insert(bob.id, bob.clone());
    users.insert(alice.id, alice.clone());

    let db = Db { users, countries };
    let (json, counts) = run_query(
        r#"
        query Test {
            countries {
                id
                users(onlyAdmins: true) {
                    id
                    isAdmin
                    country {
                        id
                    }
                }
            }
        }
    "#,
        db,
    );

    assert_eq!(1, counts.user_reads);
    assert_eq!(2, counts.country_reads);

    assert_json_eq!(
        json!({
            "countries": [
                {
                    "id": country.id,
                    "users": [
                        {
                            "id": bob.id,
                            "isAdmin": true,
                            "country": { "id": country.id }
                        },
                    ]
                }
            ],
        }),
        json,
    );
}

struct DbStats {
    user_reads: usize,
    country_reads: usize,
}

fn run_query(query: &str, db: Db) -> (Value, DbStats) {
    let ctx = Context { db };

    let (result, errors) = juniper::execute(
        query,
        None,
        &Schema::new(Query, Mutation),
        &juniper::Variables::new(),
        &ctx,
    )
    .unwrap();

    if !errors.is_empty() {
        panic!(
            "GraphQL errors\n{}",
            serde_json::to_string_pretty(&errors).unwrap()
        );
    }

    let json: Value = serde_json::from_str(&serde_json::to_string(&result).unwrap()).unwrap();

    println!("{}", serde_json::to_string_pretty(&json).unwrap());

    (
        json,
        DbStats {
            user_reads: ctx.db.users.reads_count(),
            country_reads: ctx.db.countries.reads_count(),
        },
    )
}
