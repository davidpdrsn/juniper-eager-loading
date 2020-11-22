#![allow(unused_variables, unused_imports, dead_code)]
#![allow(clippy::let_unit_value)]

#[macro_use]
extern crate diesel;

use juniper::{Executor, FieldResult};
use juniper_eager_loading::{prelude::*, EagerLoading, HasMany, LoadChildrenOutput, LoadFrom};
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
        users: [User!]!
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

    impl juniper_eager_loading::LoadFrom<Country> for User {
        type Error = diesel::result::Error;
        type Context = super::Context;

        fn load(
            countries: &[Country],
            _field_args: &(),
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
        let countries = Country::eager_load_each(&country_models, ctx, trail)?;

        Ok(countries)
    }
}

pub struct Context {
    db: PgConnection,
}

impl juniper::Context for Context {}

#[derive(Clone)]
pub struct User {
    user: models::User,
}

impl UserFields for User {
    fn field_id(&self, executor: &Executor<'_, Context>) -> FieldResult<&i32> {
        Ok(&self.user.id)
    }
}

#[derive(Clone)]
pub struct Country {
    country: models::Country,
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
    ) -> FieldResult<&Vec<User>> {
        self.users.try_unwrap().map_err(From::from)
    }
}

impl EagerLoading for User {
    type Model = models::User;
    type Id = i32;
    type Context = Context;
    type Error = diesel::result::Error;

    fn new_from_model(model: &Self::Model) -> Self {
        Self {
            user: model.clone(),
        }
    }

    fn eager_load_each(
        models: &[Self::Model],
        ctx: &Self::Context,
        trail: &QueryTrail<'_, Self, Walked>,
    ) -> Result<Vec<Self>, Self::Error> {
        Ok(Vec::new())
    }
}

impl EagerLoading for Country {
    type Model = models::Country;
    type Id = i32;
    type Context = Context;
    type Error = diesel::result::Error;

    fn new_from_model(model: &Self::Model) -> Self {
        Self {
            country: model.clone(),
            users: Default::default(),
        }
    }

    fn eager_load_each(
        models: &[Self::Model],
        ctx: &Self::Context,
        trail: &QueryTrail<'_, Self, Walked>,
    ) -> Result<Vec<Self>, Self::Error> {
        let mut nodes = Self::from_db_models(models);
        if let Some(child_trail) = trail.users().walk() {
            let field_args = trail.users_args();

            EagerLoadChildrenOfType::<
                User,
                EagerLoadingContextCountryForUsers,
                _,
            >::eager_load_children(&mut nodes, models, ctx, &child_trail, &field_args)?;
        }

        Ok(nodes)
    }
}

struct EagerLoadingContextCountryForUsers;

impl<'a> EagerLoadChildrenOfType<'a, User, EagerLoadingContextCountryForUsers, ()> for Country {
    type FieldArguments = ();

    fn load_children(
        models: &[Self::Model],
        field_args: &Self::FieldArguments,
        ctx: &Self::Context,
    ) -> Result<LoadChildrenOutput<models::User, ()>, Self::Error> {
        let child_models: Vec<models::User> = LoadFrom::load(&models, field_args, ctx)?;
        Ok(LoadChildrenOutput::ChildModels(child_models))
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

    fn association(node: &mut Self) -> &mut dyn Association<User> {
        &mut node.users
    }
}

fn main() {}
