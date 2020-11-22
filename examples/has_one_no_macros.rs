#![allow(unused_variables, unused_imports, dead_code)]
#![allow(clippy::let_unit_value)]

#[macro_use]
extern crate diesel;

use futures::executor::block_on;
use futures::future::{ready, Ready};
use juniper::{Executor, FieldResult};
use juniper_eager_loading::{prelude::*, EagerLoading, HasOne, LoadChildrenOutput, LoadFrom};
use juniper_from_schema::graphql_schema;
use std::error::Error;
use std::future::Future;
use std::pin::Pin;

// the examples all use Diesel, but this library is data store agnostic
use diesel::prelude::*;

graphql_schema! {
    schema {
      query: Query
    }

    type Query {
      users: [User!]! @juniper(ownership: "owned")
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
    use futures::future::{ready, Ready};

    #[derive(Clone, Debug, Queryable)]
    pub struct User {
        pub id: i32,
        pub country_id: i32,
    }

    #[derive(Clone, Debug, Queryable)]
    pub struct Country {
        pub id: i32,
    }

    impl<'a> juniper_eager_loading::LoadFrom<'a, i32> for Country {
        type Error = diesel::result::Error;
        type Context = super::Context;
        type Future = Ready<Result<Vec<Self>, Self::Error>>;

        fn load(ids: &'a [i32], _field_args: &'a (), ctx: &'a Self::Context) -> Self::Future {
            use crate::db_schema::countries::dsl::*;
            use diesel::pg::expression::dsl::any;

            ready(countries.filter(id.eq(any(ids))).load::<Country>(&ctx.db))
        }
    }
}

pub struct Query;

impl QueryFields for Query {
    fn field_users(
        &self,
        executor: &Executor<'_, Context>,
        trail: &QueryTrail<'_, User, Walked>,
    ) -> FieldResult<Vec<User>> {
        block_on(async move {
            let ctx = executor.context();
            let user_models = db_schema::users::table.load::<models::User>(&ctx.db)?;
            let users = User::eager_load_each(&user_models, ctx, trail).await?;

            Ok(users)
        })
    }
}

pub struct Context {
    db: PgConnection,
}

impl juniper::Context for Context {}

#[derive(Clone)]
pub struct User {
    user: models::User,
    country: HasOne<Country>,
}

impl UserFields for User {
    fn field_id(&self, executor: &Executor<'_, Context>) -> FieldResult<&i32> {
        Ok(&self.user.id)
    }

    fn field_country(
        &self,
        executor: &Executor<'_, Context>,
        trail: &QueryTrail<'_, Country, Walked>,
    ) -> FieldResult<&Country> {
        self.country.try_unwrap().map_err(From::from)
    }
}

#[derive(Clone)]
pub struct Country {
    country: models::Country,
}

impl CountryFields for Country {
    fn field_id(&self, executor: &Executor<'_, Context>) -> FieldResult<&i32> {
        Ok(&self.country.id)
    }
}

type BoxedFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + 'a>>;

impl<'a> EagerLoading<'a> for User {
    type Model = models::User;
    type Id = i32;
    type Context = Context;
    type Error = diesel::result::Error;
    type EagerLoadEachFuture = BoxedFuture<'a, Result<Vec<Self>, Self::Error>>;

    fn new_from_model(model: &Self::Model) -> Self {
        Self {
            user: model.clone(),
            country: Default::default(),
        }
    }

    fn eager_load_each(
        models: &'a [models::User],
        ctx: &'a Self::Context,
        trail: &'a QueryTrail<'_, Self, Walked>,
    ) -> Self::EagerLoadEachFuture {
        Box::pin(async move {
            let mut nodes = Self::from_db_models(models);
            if let Some(child_trail) = trail.country().walk() {
                let field_args = trail.country_args();

                EagerLoadChildrenOfType::<
                    Country,
                    EagerLoadingContextUserForCountry,
                _>::eager_load_children(&mut nodes, models, ctx, &child_trail, &field_args).await?;
            }
            Ok(nodes)
        })
    }
}

struct EagerLoadingContextUserForCountry;

impl<'a> EagerLoadChildrenOfType<'a, Country, EagerLoadingContextUserForCountry, ()> for User {
    type FieldArguments = ();
    type LoadChildrenFuture =
        BoxedFuture<'a, Result<LoadChildrenOutput<models::Country>, diesel::result::Error>>;

    fn load_children(
        models: &'a [models::User],
        field_args: &'a Self::FieldArguments,
        ctx: &'a Self::Context,
    ) -> Self::LoadChildrenFuture {
        Box::pin(async move {
            let ids = models
                .iter()
                .map(|model| model.country_id)
                .collect::<Vec<_>>();
            let ids = juniper_eager_loading::unique(ids);

            let child_models = models::Country::load(&ids, field_args, ctx).await?;

            let child_models: Vec<models::Country> =
                models::Country::load(&ids, field_args, ctx).await?;
            Ok::<_, Self::Error>(LoadChildrenOutput::ChildModels(child_models))
        })
    }

    fn is_child_of(
        node: &User,
        child: &Country,
        _join_model: &(),
        _field_args: &Self::FieldArguments,
        _ctx: &Self::Context,
    ) -> bool {
        node.user.country_id == child.country.id
    }

    fn association(node: &mut Self) -> &mut dyn Association<Country> {
        &mut node.country
    }
}

impl<'a> EagerLoading<'a> for Country {
    type Model = models::Country;
    type Id = i32;
    type Context = Context;
    type Error = diesel::result::Error;
    type EagerLoadEachFuture = Ready<Result<Vec<Self>, Self::Error>>;

    fn new_from_model(model: &Self::Model) -> Self {
        Self {
            country: model.clone(),
        }
    }

    fn eager_load_each(
        models: &'a [models::Country],
        ctx: &'a Self::Context,
        trail: &'a QueryTrail<'_, Country, Walked>,
    ) -> Self::EagerLoadEachFuture {
        ready(Ok(Vec::new()))
    }
}

fn main() {}
