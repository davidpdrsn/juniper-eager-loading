#![allow(unused_variables, unused_imports, dead_code)]
#![allow(clippy::let_unit_value)]

#[macro_use]
extern crate diesel;

use async_trait::async_trait;
use juniper::{Executor, FieldResult};
use juniper_eager_loading::{prelude::*, EagerLoading, HasOne, LoadChildrenOutput, LoadFrom};
use juniper_from_schema::graphql_schema;
use std::{error::Error, sync::Mutex};

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
    use async_trait::async_trait;
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

    #[async_trait]
    impl juniper_eager_loading::LoadFrom<i32> for Country {
        type Error = diesel::result::Error;
        type Context = super::Context;

        async fn load(
            ids: &[i32],
            _field_args: &(),
            ctx: &Self::Context,
        ) -> Result<Vec<Self>, Self::Error> {
            use crate::db_schema::countries::dsl::*;
            use diesel::pg::expression::dsl::any;

            countries
                .filter(id.eq(any(ids)))
                .load::<Country>(&*ctx.db.lock().unwrap())
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
        futures::executor::block_on(async move {
            let ctx = executor.context();
            let user_models =
                db_schema::users::table.load::<models::User>(&*ctx.db.lock().unwrap())?;
            let users = User::eager_load_each(user_models, ctx, trail).await?;

            Ok(users)
        })
    }
}

pub struct Context {
    db: Mutex<PgConnection>,
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

#[async_trait]
impl<'a> EagerLoading<'a> for User {
    type Model = models::User;
    type Id = i32;
    type Context = Context;
    type Error = diesel::result::Error;

    fn new_from_model(model: &Self::Model) -> Self {
        Self {
            user: model.clone(),
            country: Default::default(),
        }
    }

    async fn eager_load_each(
        models: Vec<models::User>,
        ctx: &Self::Context,
        trail: &QueryTrail<'_, Self, Walked>,
    ) -> Result<Vec<Self>, Self::Error> {
        let mut nodes = Self::from_db_models(&models);

        if let Some(child_trail) = trail.country().walk() {
            let field_args = trail.country_args();

            EagerLoadChildrenOfType::<
                Country,
                EagerLoadingContextUserForCountry,
            _>::eager_load_children(&mut nodes, &models, ctx, &child_trail, &field_args).await?;
        }

        Ok(nodes)
    }
}

struct EagerLoadingContextUserForCountry;

#[async_trait]
impl<'a> EagerLoadChildrenOfType<'a, Country, EagerLoadingContextUserForCountry, ()> for User {
    type FieldArguments = ();

    async fn load_children(
        models: &[models::User],
        field_args: &Self::FieldArguments,
        ctx: &Context,
    ) -> Result<LoadChildrenOutput<models::Country, ()>, diesel::result::Error> {
        let ids = models
            .iter()
            .map(|model| model.country_id)
            .collect::<Vec<_>>();
        let ids = juniper_eager_loading::unique(ids);

        let child_models: Vec<models::Country> = LoadFrom::load(&ids, field_args, ctx).await?;

        Ok(LoadChildrenOutput::ChildModels(child_models))
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

#[async_trait]
impl<'a> EagerLoading<'a> for Country {
    type Model = models::Country;
    type Id = i32;
    type Context = Context;
    type Error = diesel::result::Error;

    fn new_from_model(model: &Self::Model) -> Self {
        Self {
            country: model.clone(),
        }
    }

    async fn eager_load_each(
        models: Vec<Self::Model>,
        ctx: &Self::Context,
        trail: &QueryTrail<'_, Self, Walked>,
    ) -> Result<Vec<Self>, Self::Error> {
        Ok(Vec::new())
    }
}

fn main() {}
