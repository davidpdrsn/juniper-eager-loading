#![allow(unused_variables, unused_imports, dead_code)]

use juniper::{Executor, FieldResult};
use juniper_eager_loading::{prelude::*, *};
use juniper_from_schema::graphql_schema;
use std::error::Error;

graphql_schema! {
    schema { query: Query }

    type Query { noop: Boolean! @juniper(ownership: "owned") }

    type User {
        id: Int!
        companies: [Company!]!
    }

    type Company {
        id: Int!
        employees: [User!]!
    }
}

pub struct Query;

impl QueryFields for Query {
    fn field_noop(&self, executor: &Executor<'_, Context>) -> FieldResult<bool> {
        unimplemented!()
    }
}

pub struct DbConnection;

impl DbConnection {
    fn load_all_users(&self) -> Vec<models::User> {
        unimplemented!()
    }
}

pub struct Context {
    db: DbConnection,
}

impl juniper::Context for Context {}

impl UserFields for User {
    fn field_id(&self, executor: &Executor<'_, Context>) -> FieldResult<&i32> {
        unimplemented!()
    }

    fn field_companies(
        &self,
        executor: &Executor<'_, Context>,
        trail: &QueryTrail<'_, Company, Walked>,
    ) -> FieldResult<&Vec<Company>> {
        Ok(self.companies.try_unwrap()?)
    }
}

impl CompanyFields for Company {
    fn field_id(&self, executor: &Executor<'_, Context>) -> FieldResult<&i32> {
        unimplemented!()
    }

    fn field_employees(
        &self,
        executor: &Executor<'_, Context>,
        trail: &QueryTrail<'_, User, Walked>,
    ) -> FieldResult<&Vec<User>> {
        Ok(self.employees.try_unwrap()?)
    }
}

mod models {
    use super::DbConnection;

    #[derive(Clone)]
    pub struct User {
        pub id: i32,
    }

    #[derive(Clone)]
    pub struct Company {
        pub id: i32,
    }

    #[derive(Clone)]
    pub struct Employment {
        pub id: i32,
        pub user_id: i32,
        pub company_id: i32,
    }

    impl Employment {
        pub fn a_predicate_method(&self, db: &super::DbConnection) -> bool {
            true
        }
    }

    impl juniper_eager_loading::LoadFrom<i32> for User {
        type Error = Box<dyn std::error::Error>;
        type Connection = DbConnection;

        fn load(employments: &[i32], _: &(), db: &Self::Connection) -> Result<Vec<Self>, Self::Error> {
            unimplemented!()
        }
    }

    impl juniper_eager_loading::LoadFrom<i32> for Company {
        type Error = Box<dyn std::error::Error>;
        type Connection = DbConnection;

        fn load(employments: &[i32], _: &(), db: &Self::Connection) -> Result<Vec<Self>, Self::Error> {
            unimplemented!()
        }
    }

    impl juniper_eager_loading::LoadFrom<User> for Employment {
        type Error = Box<dyn std::error::Error>;
        type Connection = DbConnection;

        fn load(employments: &[User], _: &(), db: &Self::Connection) -> Result<Vec<Self>, Self::Error> {
            unimplemented!()
        }
    }

    impl juniper_eager_loading::LoadFrom<Company> for Employment {
        type Error = Box<dyn std::error::Error>;
        type Connection = DbConnection;

        fn load(employments: &[Company], _: &(), db: &Self::Connection) -> Result<Vec<Self>, Self::Error> {
            unimplemented!()
        }
    }

    impl juniper_eager_loading::LoadFrom<Employment> for User {
        type Error = Box<dyn std::error::Error>;
        type Connection = DbConnection;

        fn load(
            employments: &[Employment],
            _: &(),
            db: &Self::Connection,
        ) -> Result<Vec<Self>, Self::Error> {
            unimplemented!()
        }
    }

    impl juniper_eager_loading::LoadFrom<Employment> for Company {
        type Error = Box<dyn std::error::Error>;
        type Connection = DbConnection;

        fn load(
            employments: &[Employment],
            _: &(),
            db: &Self::Connection,
        ) -> Result<Vec<Self>, Self::Error> {
            unimplemented!()
        }
    }
}

#[derive(Clone, EagerLoading)]
#[eager_loading(connection = "DbConnection", error = "Box<dyn std::error::Error>")]
pub struct User {
    user: models::User,

    #[has_many_through(
        join_model = "models::Employment",
        model_field = "company",
        join_model_field = "employment",
        predicate_method = "a_predicate_method",
        graphql_field = "companies"
    )]
    companies: HasManyThrough<Company>,
}

#[derive(Clone, EagerLoading)]
#[eager_loading(connection = "DbConnection", error = "Box<dyn std::error::Error>")]
pub struct Company {
    company: models::Company,

    #[has_many_through(join_model = "models::Employment")]
    employees: HasManyThrough<User>,
}

fn main() {}
