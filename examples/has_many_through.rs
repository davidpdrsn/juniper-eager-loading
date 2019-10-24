#![allow(unused_variables, unused_imports, dead_code)]

#[macro_use]
extern crate diesel;

use juniper::{Executor, FieldResult};
use juniper_eager_loading::{prelude::*, EagerLoading, HasManyThrough};
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
        companies: [Company!]!
    }

    type Company {
        id: Int!
    }
}

mod db_schema {
    table! {
        users {
            id -> Integer,
        }
    }

    table! {
        companies {
            id -> Integer,
        }
    }

    table! {
        employments {
            id -> Integer,
            user_id -> Integer,
            company_id -> Integer,
        }
    }
}

mod models {
    use diesel::prelude::*;

    #[derive(Clone, Debug, Queryable)]
    pub struct User {
        pub id: i32,
    }

    #[derive(Clone, Debug, Queryable)]
    pub struct Company {
        pub id: i32,
    }

    #[derive(Clone, Debug, Queryable)]
    pub struct Employment {
        pub id: i32,
        pub user_id: i32,
        pub company_id: i32,
    }

    impl juniper_eager_loading::LoadFrom<Employment> for Company {
        type Error = diesel::result::Error;
        type Connection = PgConnection;

        fn load(
            employments: &[Employment],
            _field_args: &(),
            db: &Self::Connection,
        ) -> Result<Vec<Self>, Self::Error> {
            use crate::db_schema::companies::dsl::*;
            use diesel::pg::expression::dsl::any;

            let company_ids = employments
                .iter()
                .map(|employent| employent.company_id)
                .collect::<Vec<_>>();

            companies
                .filter(id.eq(any(company_ids)))
                .load::<Company>(db)
        }
    }

    impl juniper_eager_loading::LoadFrom<User> for Employment {
        type Error = diesel::result::Error;
        type Connection = PgConnection;

        fn load(
            users: &[User],
            _field_args: &(),
            db: &Self::Connection,
        ) -> Result<Vec<Self>, Self::Error> {
            use crate::db_schema::employments::dsl::*;
            use diesel::pg::expression::dsl::any;

            let user_ids = users.iter().map(|user| user.id).collect::<Vec<_>>();

            employments
                .filter(user_id.eq(any(user_ids)))
                .load::<Employment>(db)
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
        let country_models = db_schema::users::table.load::<models::User>(db)?;
        let mut country = User::from_db_models(&country_models);
        User::eager_load_all_children_for_each(&mut country, &country_models, db, trail)?;

        Ok(country)
    }
}

pub struct Context {
    db: PgConnection,
}

impl juniper::Context for Context {}

#[derive(Clone, EagerLoading)]
#[eager_loading(connection = "PgConnection", error = "diesel::result::Error")]
pub struct User {
    user: models::User,

    #[has_many_through(join_model = "models::Employment")]
    companies: HasManyThrough<Company>,
}

impl UserFields for User {
    fn field_id(&self, executor: &Executor<'_, Context>) -> FieldResult<&i32> {
        Ok(&self.user.id)
    }

    fn field_companies(
        &self,
        executor: &Executor<'_, Context>,
        trail: &QueryTrail<'_, Company, Walked>,
    ) -> FieldResult<&Vec<Company>> {
        self.companies.try_unwrap().map_err(From::from)
    }
}

#[derive(Clone, EagerLoading)]
#[eager_loading(connection = "PgConnection", error = "diesel::result::Error")]
pub struct Company {
    company: models::Company,
}

impl CompanyFields for Company {
    fn field_id(&self, executor: &Executor<'_, Context>) -> FieldResult<&i32> {
        Ok(&self.company.id)
    }
}

fn main() {}
