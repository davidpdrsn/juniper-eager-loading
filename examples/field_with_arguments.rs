#![allow(unused_variables, unused_imports, dead_code)]

#[macro_use]
extern crate diesel;

use chrono::prelude::*;
use juniper::{Executor, FieldResult};
use juniper_eager_loading::{prelude::*, EagerLoading, HasMany};
use juniper_from_schema::graphql_schema;
use std::error::Error;

// the examples all use Diesel, but this library is data store agnostic
use diesel::prelude::*;

graphql_schema! {
    schema {
      query: Query
    }

    type Query {
      countries: [Country!]! @juniper(ownership: "owned")
    }

    type User {
        id: Int!
    }

    type Country {
        id: Int!
        users(activeSince: DateTimeUtc!): [User!]!
    }

    scalar DateTimeUtc
}

mod db_schema {
    table! {
        users {
            id -> Integer,
            country_id -> Integer,
            active_since -> Timestamptz,
        }
    }

    table! {
        countries {
            id -> Integer,
        }
    }
}

mod models {
    use super::CountryUsersArgs;
    use chrono::prelude::*;
    use diesel::prelude::*;

    #[derive(Clone, Debug, Queryable)]
    pub struct User {
        pub id: i32,
        pub country_id: i32,
        pub active_since: DateTime<Utc>,
    }

    #[derive(Clone, Debug, Queryable)]
    pub struct Country {
        pub id: i32,
    }

    impl<'a> juniper_eager_loading::LoadFrom<Country, CountryUsersArgs<'a>> for User {
        type Error = diesel::result::Error;
        type Context = super::Context;

        fn load(
            countries: &[Country],
            field_args: &CountryUsersArgs<'a>,
            ctx: &Self::Context,
        ) -> Result<Vec<Self>, Self::Error> {
            use crate::db_schema::users::dsl::*;
            use diesel::pg::expression::dsl::any;

            let country_ids = countries
                .iter()
                .map(|country| country.id)
                .collect::<Vec<_>>();

            users
                .filter(country_id.eq(any(country_ids)))
                .filter(active_since.gt(&field_args.active_since()))
                .load::<User>(&ctx.db)
        }
    }
}

pub struct Query;

impl QueryFields for Query {
    fn field_countries(
        &self,
        executor: &Executor<'_, Context>,
        trail: &QueryTrail<'_, Country, Walked>,
    ) -> FieldResult<Vec<Country>> {
        let ctx = executor.context();
        let country_models = db_schema::countries::table.load::<models::Country>(&ctx.db)?;
        let country = Country::eager_load_each(&country_models, ctx, trail)?;

        Ok(country)
    }
}

pub struct Context {
    db: PgConnection,
}

impl juniper::Context for Context {}

#[derive(Clone, EagerLoading)]
#[eager_loading(context = Context, error = diesel::result::Error)]
pub struct User {
    user: models::User,
}

impl UserFields for User {
    fn field_id(&self, executor: &Executor<'_, Context>) -> FieldResult<&i32> {
        Ok(&self.user.id)
    }
}

#[derive(Clone, EagerLoading)]
#[eager_loading(context = Context, error = diesel::result::Error)]
pub struct Country {
    country: models::Country,

    #[has_many(
        root_model_field = user,
        field_arguments = CountryUsersArgs,
     // ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ The important line
    )]
    users: HasMany<User>,
}

impl CountryFields for Country {
    fn field_id(&self, executor: &Executor<'_, Context>) -> FieldResult<&i32> {
        Ok(&self.country.id)
    }

    fn field_users(
        &self,
        executor: &Executor<'_, Context>,
        trail: &QueryTrail<'_, User, Walked>,
        _active_since: DateTime<Utc>,
    ) -> FieldResult<&Vec<User>> {
        self.users.try_unwrap().map_err(From::from)
    }
}

fn main() {}
