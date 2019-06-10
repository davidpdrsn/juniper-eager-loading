use juniper::{Executor, FieldResult};
use juniper_eager_loading::{prelude::*, EagerLoading, HasOne};
use juniper_from_schema::graphql_schema;
use std::error::Error;

graphql_schema! {
    schema { query: Query }

    type Query { noop: Boolean! @juniper(ownership: "owned") }

    type User {
        id: Int!
        country: Country!
    }

    type Country {
        id: Int!
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

    fn field_country(
        &self,
        executor: &Executor<'_, Context>,
        trail: &QueryTrail<'_, Country, Walked>,
    ) -> FieldResult<&Country> {
        unimplemented!()
    }
}

impl CountryFields for Country {
    fn field_id(&self, executor: &Executor<'_, Context>) -> FieldResult<&i32> {
        unimplemented!()
    }
}

mod models {
    use super::DbConnection;

    #[derive(Clone)]
    pub struct User {
        pub id: i32,
        pub country_id: i32,
    }

    #[derive(Clone)]
    pub struct Country {
        pub id: i32,
    }

    impl juniper_eager_loading::LoadFrom<i32> for Country {
        type Error = Box<dyn std::error::Error>;
        type Connection = DbConnection;

        fn load(employments: &[i32], db: &Self::Connection) -> Result<Vec<Self>, Self::Error> {
            unimplemented!()
        }
    }
}

#[derive(Clone, EagerLoading)]
#[eager_loading(connection = "DbConnection", error = "Box<dyn std::error::Error>")]
pub struct User {
    user: models::User,

    // these are the defaults. `#[has_one(default)]` would also work here.
    #[has_one(
        foreign_key_field = "country_id",
        model = "models::Country",
        root_model_field = "country",
        graphql_field = "country"
    )]
    country: HasOne<Country>,
}

#[derive(Clone, EagerLoading)]
#[eager_loading(connection = "DbConnection", error = "Box<dyn std::error::Error>")]
pub struct Country {
    country: models::Country,
}

fn main() {}
