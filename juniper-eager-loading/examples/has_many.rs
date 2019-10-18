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
        cars: [Car!]!
    }

    type Car {
        id: Int!
        user: User!
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

    fn field_cars(
        &self,
        executor: &Executor<'_, Context>,
        trail: &QueryTrail<'_, Car, Walked>,
    ) -> FieldResult<&Vec<Car>> {
        Ok(self.cars.try_unwrap()?)
    }
}

impl CarFields for Car {
    fn field_id(&self, executor: &Executor<'_, Context>) -> FieldResult<&i32> {
        unimplemented!()
    }

    fn field_user(
        &self,
        executor: &Executor<'_, Context>,
        trail: &QueryTrail<'_, User, Walked>,
    ) -> FieldResult<&User> {
        Ok(self.user.try_unwrap()?)
    }
}

mod models {
    use super::DbConnection;

    #[derive(Clone)]
    pub struct User {
        pub id: i32,
    }

    #[derive(Clone)]
    pub struct Car {
        pub id: i32,
        pub user_id: i32,
    }

    impl Car {
        pub fn a_predicate_method(&self, db: &DbConnection) -> bool {
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

    impl juniper_eager_loading::LoadFrom<i32> for Car {
        type Error = Box<dyn std::error::Error>;
        type Connection = DbConnection;

        fn load(employments: &[i32], _: &(), db: &Self::Connection) -> Result<Vec<Self>, Self::Error> {
            unimplemented!()
        }
    }

    impl juniper_eager_loading::LoadFrom<User> for Car {
        type Error = Box<dyn std::error::Error>;
        type Connection = DbConnection;

        fn load(employments: &[User], _: &(), db: &Self::Connection) -> Result<Vec<Self>, Self::Error> {
            unimplemented!()
        }
    }
}

#[derive(Clone, EagerLoading)]
#[eager_loading(connection = "DbConnection", error = "Box<dyn std::error::Error>")]
pub struct User {
    user: models::User,

    #[has_many(
        root_model_field = "car",
        foreign_key_field = "user_id",
        graphql_field = "cars",
        predicate_method = "a_predicate_method"
    )]
    cars: HasMany<Car>,
}

#[derive(Clone, EagerLoading)]
#[eager_loading(connection = "DbConnection", error = "Box<dyn std::error::Error>")]
pub struct Car {
    car: models::Car,

    #[has_one(default)]
    user: HasOne<User>,
}

fn main() {}
