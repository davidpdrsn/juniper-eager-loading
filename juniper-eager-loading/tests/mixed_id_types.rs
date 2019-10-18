#![allow(unused_variables, unused_imports, dead_code, unused_mut)]

mod helpers;

use assert_json_diff::assert_json_include;
use helpers::StatsHash;
use juniper::{EmptyMutation, Executor, FieldResult, ID};
use juniper_eager_loading::{prelude::*, EagerLoading, HasManyThrough, HasOne};
use juniper_from_schema::graphql_schema;
use serde_json::{json, Value};

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
        visitedCountries: [Country!]!
    }

    type Country {
        id: ID! @juniper(ownership: "owned")
    }
}

mod models {
    #[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
    pub struct User {
        pub id: i32,
        pub country_id: i64,
    }

    #[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
    pub struct Country {
        pub id: i64,
    }

    #[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
    pub struct Visit {
        pub person_id: i32,
        pub country_id: i64,
    }

    impl juniper_eager_loading::LoadFrom<i32> for User {
        type Error = Box<dyn std::error::Error>;
        type Connection = super::Db;

        fn load(ids: &[i32], _: &(), db: &Self::Connection) -> Result<Vec<Self>, Self::Error> {
            let models = db
                .users
                .all_values()
                .into_iter()
                .filter(|value| ids.contains(&value.id))
                .cloned()
                .collect::<Vec<_>>();
            Ok(models)
        }
    }

    impl juniper_eager_loading::LoadFrom<i64> for Country {
        type Error = Box<dyn std::error::Error>;
        type Connection = super::Db;

        fn load(ids: &[i64], _: &(), db: &Self::Connection) -> Result<Vec<Self>, Self::Error> {
            let countries = db
                .countries
                .all_values()
                .into_iter()
                .filter(|value| ids.contains(&value.id))
                .cloned()
                .collect::<Vec<_>>();
            Ok(countries)
        }
    }

    impl juniper_eager_loading::LoadFrom<User> for Visit {
        type Error = Box<dyn std::error::Error>;
        type Connection = super::Db;

        fn load(users: &[User], _: &(), db: &Self::Connection) -> Result<Vec<Self>, Self::Error> {
            let user_ids = users.iter().map(|user| user.id).collect::<Vec<_>>();
            let visits = db
                .visits
                .iter()
                .filter(|visit| user_ids.contains(&visit.person_id))
                .cloned()
                .collect::<Vec<_>>();
            Ok(visits)
        }
    }

    impl juniper_eager_loading::LoadFrom<Visit> for Country {
        type Error = Box<dyn std::error::Error>;
        type Connection = super::Db;

        fn load(visits: &[Visit], _: &(), db: &Self::Connection) -> Result<Vec<Self>, Self::Error> {
            let country_ids = visits
                .iter()
                .map(|visit| visit.country_id)
                .collect::<Vec<_>>();
            let countries = db
                .countries
                .all_values()
                .into_iter()
                .filter(|country| country_ids.contains(&country.id))
                .cloned()
                .collect::<Vec<_>>();
            Ok(countries)
        }
    }
}

pub struct Db {
    users: StatsHash<i32, models::User>,
    countries: StatsHash<i64, models::Country>,
    visits: Vec<models::Visit>,
}

pub struct Context {
    db: Db,
}

impl juniper::Context for Context {}

pub struct Query;

impl QueryFields for Query {
    fn field_users<'a>(
        &self,
        executor: &Executor<'a, Context>,
        trail: &QueryTrail<'a, User, Walked>,
    ) -> FieldResult<Vec<User>> {
        let db = &executor.context().db;

        let mut user_models = db
            .users
            .all_values()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        user_models.sort_by_key(|user| user.id);

        let mut users = User::from_db_models(&user_models);
        User::eager_load_all_children_for_each(&mut users, &user_models, db, trail)?;

        Ok(users)
    }
}

// The default values are commented out
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug, EagerLoading)]
#[eager_loading(connection = "Db", error = "Box<dyn std::error::Error>")]
pub struct User {
    user: models::User,

    #[has_one(default)]
    country: HasOne<Country>,

    #[has_many_through(join_model = "models::Visit", foreign_key_field = "person_id")]
    visited_countries: HasManyThrough<Country>,
}

impl UserFields for User {
    fn field_id(&self, _executor: &Executor<'_, Context>) -> FieldResult<&i32> {
        Ok(&self.user.id)
    }

    fn field_country(
        &self,
        _executor: &Executor<'_, Context>,
        _trail: &QueryTrail<'_, Country, Walked>,
    ) -> FieldResult<&Country> {
        Ok(self.country.try_unwrap()?)
    }

    fn field_visited_countries(
        &self,
        _executor: &Executor<'_, Context>,
        _trail: &QueryTrail<'_, Country, Walked>,
    ) -> FieldResult<&Vec<Country>> {
        Ok(self.visited_countries.try_unwrap()?)
    }
}

// #[derive(Clone, Eq, PartialEq, Debug)]
#[derive(Clone, Eq, PartialEq, Debug, Ord, PartialOrd, EagerLoading)]
#[eager_loading(
    model = "models::Country",
    connection = "Db",
    id = "i64",
    error = "Box<dyn std::error::Error>",
    root_model_field = "country"
)]
pub struct Country {
    country: models::Country,
}

impl CountryFields for Country {
    fn field_id(&self, _executor: &Executor<'_, Context>) -> FieldResult<ID> {
        Ok(self.country.id.to_string().into())
    }
}

#[test]
fn loading_users_and_associations() {
    let mut countries = StatsHash::new("countries");
    let mut users = StatsHash::new("users");

    let country = models::Country { id: 10 };

    countries.insert(country.id, country.clone());

    users.insert(
        1,
        models::User {
            id: 1,
            country_id: country.id,
        },
    );

    let db = Db {
        users,
        countries,
        visits: vec![],
    };

    let (json, counts) = run_query(
        r#"
        query Test {
            users {
                id
                country {
                    id
                }
            }
        }
    "#,
        db,
    );

    assert_json_include!(
        expected: json!({
            "users": [
                {
                    "id": 1,
                    "country": {
                        "id": country.id.to_string(),
                    },
                },
            ]
        }),
        actual: json.clone(),
    );

    assert_eq!(1, counts.user_reads);
    assert_eq!(1, counts.country_reads);
}

#[test]
fn has_many_through_fkey() {
    let mut countries = StatsHash::new("countries");
    let mut users = StatsHash::new("users");
    let mut visits = vec![];

    let country = models::Country { id: 10 };
    countries.insert(country.id, country.clone());

    let user = models::User {
        id: 1,
        country_id: country.id,
    };
    users.insert(1, user.clone());

    visits.push(models::Visit {
        country_id: country.id,
        person_id: user.id,
    });

    let db = Db {
        users,
        countries,
        visits,
    };

    let (json, counts) = run_query(
        r#"
        query Test {
            users {
                id
                visitedCountries {
                    id
                }
            }
        }
    "#,
        db,
    );

    assert_json_include!(
        expected: json!({
            "users": [
                {
                    "id": 1,
                    "visitedCountries": [
                        {
                            "id": country.id.to_string(),
                        }
                    ]
                },
            ]
        }),
        actual: json.clone(),
    );

    assert_eq!(1, counts.user_reads);
    assert_eq!(1, counts.country_reads);
}

struct DbStats {
    user_reads: usize,
    country_reads: usize,
}

fn run_query(query: &str, db: Db) -> (Value, DbStats) {
    let ctx = Context { db };

    let (result, errors) = juniper::execute(
        query,
        None,
        &Schema::new(Query, EmptyMutation::new()),
        &juniper::Variables::new(),
        &ctx,
    )
    .unwrap();

    if !errors.is_empty() {
        panic!(
            "GraphQL errors\n{}",
            serde_json::to_string_pretty(&errors).unwrap()
        );
    }

    let json: Value = serde_json::from_str(&serde_json::to_string(&result).unwrap()).unwrap();

    println!("{}", serde_json::to_string_pretty(&json).unwrap());

    (
        json,
        DbStats {
            user_reads: ctx.db.users.reads_count(),
            country_reads: ctx.db.countries.reads_count(),
        },
    )
}
