#![allow(unused_variables, unused_imports, dead_code)]

#[macro_use]
extern crate diesel;

use juniper::{Executor, FieldResult};
use juniper_eager_loading::{prelude::*, EagerLoading, LoadChildrenOutput, LoadFrom, OptionHasOne};
use juniper_from_schema::graphql_schema;
use std::error::Error;

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
        country: Country
    }

    type Country {
        id: Int!
    }
}

mod db_schema {
    table! {
        users {
            id -> Integer,
            country_id -> Nullable<Integer>,
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
        pub country_id: Option<i32>,
    }

    #[derive(Clone, Debug, Queryable)]
    pub struct Country {
        pub id: i32,
    }

    impl juniper_eager_loading::LoadFrom<i32> for Country {
        type Error = diesel::result::Error;
        type Connection = PgConnection;

        fn load(
            ids: &[i32],
            _field_args: &(),
            db: &Self::Connection,
        ) -> Result<Vec<Self>, Self::Error> {
            use crate::db_schema::countries::dsl::*;
            use diesel::pg::expression::dsl::any;

            countries.filter(id.eq(any(ids))).load::<Country>(db)
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
        let db = &executor.context().db;
        let user_models = db_schema::users::table.load::<models::User>(db)?;
        let mut users = User::from_db_models(&user_models);
        User::eager_load_all_children_for_each(&mut users, &user_models, db, trail)?;

        Ok(users)
    }
}

pub struct Context {
    db: PgConnection,
}

impl juniper::Context for Context {}

#[derive(Clone)]
pub struct User {
    user: models::User,
    country: OptionHasOne<Country>,
}

impl UserFields for User {
    fn field_id(&self, executor: &Executor<'_, Context>) -> FieldResult<&i32> {
        Ok(&self.user.id)
    }

    fn field_country(
        &self,
        executor: &Executor<'_, Context>,
        trail: &QueryTrail<'_, Country, Walked>,
    ) -> FieldResult<&Option<Country>> {
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

impl GraphqlNodeForModel for User {
    type Model = models::User;
    type Id = i32;
    type Connection = PgConnection;
    type Error = diesel::result::Error;

    fn new_from_model(model: &Self::Model) -> Self {
        Self {
            user: model.clone(),
            country: Default::default(),
        }
    }
}

impl EagerLoadAllChildren for User {
    fn eager_load_all_children_for_each(
        nodes: &mut [Self],
        models: &[Self::Model],
        db: &Self::Connection,
        trail: &QueryTrail<'_, Self, Walked>,
    ) -> Result<(), Self::Error> {
        if let Some(child_trail) = trail.country().walk() {
            let field_args = trail.country_args();

            EagerLoadChildrenOfType::<
                Country,
                EagerLoadingContextUserForCountry,
                _
            >::eager_load_children(nodes, models, db, &child_trail, &field_args)?;
        }
        Ok(())
    }
}

struct EagerLoadingContextUserForCountry;

impl<'a> EagerLoadChildrenOfType<'a, Country, EagerLoadingContextUserForCountry, ()> for User {
    type FieldArguments = ();

    fn load_children(
        models: &[Self::Model],
        field_args: &Self::FieldArguments,
        db: &Self::Connection,
    ) -> Result<LoadChildrenOutput<<Country as GraphqlNodeForModel>::Model, ()>, Self::Error> {
        let ids = models
            .iter()
            .filter_map(|model| model.country_id)
            .map(|id| id)
            .collect::<Vec<_>>();
        let ids = juniper_eager_loading::unique(ids);

        let child_models: Vec<models::Country> = LoadFrom::load(&ids, field_args, db)?;

        Ok(LoadChildrenOutput::ChildModels(child_models))
    }

    fn is_child_of(
        node: &Self,
        child: &Country,
        join_model: &(),
        _field_args: &Self::FieldArguments,
    ) -> bool {
        node.user.country_id == Some(child.country.id)
    }

    fn association(node: &mut Self) -> &mut dyn Association<Country> {
        &mut node.country
    }
}

impl GraphqlNodeForModel for Country {
    type Model = models::Country;
    type Id = i32;
    type Connection = PgConnection;
    type Error = diesel::result::Error;

    fn new_from_model(model: &Self::Model) -> Self {
        Self {
            country: model.clone(),
        }
    }
}

impl EagerLoadAllChildren for Country {
    fn eager_load_all_children_for_each(
        nodes: &mut [Self],
        models: &[Self::Model],
        db: &Self::Connection,
        trail: &QueryTrail<'_, Self, Walked>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn main() {}
