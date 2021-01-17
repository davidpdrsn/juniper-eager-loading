#![allow(unused_variables, unused_imports, dead_code)]

#[macro_use]
extern crate diesel;

use juniper::{Executor, FieldResult};
use juniper_eager_loading::{prelude::*, EagerLoading, HasOne};
use juniper_from_schema::graphql_schema;
use std::{error::Error, sync::Mutex};

// the examples all use Diesel, but this library is data store agnostic
use diesel::prelude::*;

graphql_schema! {
    schema {
      query: Query
    }

    type Query {
      users: [User!]! @juniper(ownership: "owned", async: true)
    }

    type User {
        id: Int!
        country: Country!
    }

    type Country {
        id: Int!
    }
}

mod db_schema {
    table! {
        users {
            id -> Integer,
            country_id -> Integer,
        }
    }

    table! {
        countries {
            id -> Integer,
        }
    }
}

mod models {
    use diesel::prelude::*;

    #[derive(Clone, Debug, Queryable)]
    pub struct User {
        pub id: i32,
        pub country_id: i32,
    }

    #[derive(Clone, Debug, Queryable)]
    pub struct Country {
        pub id: i32,
    }

    #[async_trait::async_trait]
    impl juniper_eager_loading::LoadFrom<i32> for Country {
        type Error = diesel::result::Error;
        type Context = super::Context;

        async fn load(
            ids: &[i32],
            _field_args: &(),
            ctx: &Self::Context,
        ) -> Result<Vec<Self>, Self::Error> {
            todo!()

            // use crate::db_schema::countries::dsl::*;
            // use diesel::pg::expression::dsl::any;

            // countries.filter(id.eq(any(ids))).load::<Country>(&ctx.db)
        }
    }
}

pub struct Query;

#[async_trait::async_trait]
impl QueryFields for Query {
    async fn field_users<'s, 'r, 'a>(
        &'s self,
        executor: &Executor<'r, 'a, Context>,
        trail: &QueryTrail<'r, User, Walked>,
    ) -> FieldResult<Vec<User>> {
        let ctx = executor.context();
        let user_models = db_schema::users::table.load::<models::User>(&*ctx.db.lock().unwrap())?;
        let users = User::eager_load_each(&user_models, ctx, trail).await?;

        Ok(users)
    }
}

pub struct Context {
    db: Mutex<PgConnection>,
}

impl juniper::Context for Context {}

#[derive(Clone)]
#[derive(EagerLoading)]
#[eager_loading(context = Context, error = diesel::result::Error)]
pub struct User {
    user: models::User,

    // these are the defaults. `#[has_one(default)]` would also work here.
    #[has_one(
        foreign_key_field = country_id,
        root_model_field = country,
        graphql_field = country
    )]
    country: HasOne<Country>,
}

impl UserFields for User {
    fn field_id(&self, executor: &Executor<Context>) -> FieldResult<&i32> {
        Ok(&self.user.id)
    }

    fn field_country(
        &self,
        executor: &Executor<Context>,
        trail: &QueryTrail<Country, Walked>,
    ) -> FieldResult<&Country> {
        self.country.try_unwrap().map_err(From::from)
    }
}

#[derive(Clone)]
#[derive(EagerLoading)]
#[eager_loading(context = Context, error = diesel::result::Error)]
pub struct Country {
    country: models::Country,
}

impl CountryFields for Country {
    fn field_id(&self, executor: &Executor<Context>) -> FieldResult<&i32> {
        Ok(&self.country.id)
    }
}

fn main() {}
