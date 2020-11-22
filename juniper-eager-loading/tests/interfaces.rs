#![allow(unused_variables, unused_imports, dead_code, unused_mut)]

mod helpers;

use assert_json_diff::{assert_json_eq, assert_json_include};
use helpers::{SortedExtension, StatsHash};
use juniper::{Executor, FieldError, FieldResult};
use juniper_eager_loading::{
    prelude::*, EagerLoading, HasMany, HasManyThrough, HasOne, OptionHasOne,
};
use juniper_from_schema::graphql_schema;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{borrow::Borrow, collections::HashMap, hash::Hash};

graphql_schema! {
    schema {
      query: Query
    }

    type Query {
      search: [HasCountry!]! @juniper(ownership: "owned")
    }

    type User implements HasCountry {
        id: Int!
        country: Country!
    }

    type City implements HasCountry {
        id: Int!
        country: Country!
    }

    interface HasCountry {
        country: Country!
    }

    type Country {
        id: Int!
    }

}

mod models {
    use juniper_eager_loading::LoadFrom;

    #[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
    pub struct User {
        pub id: i32,
        pub country_id: i32,
    }

    #[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
    pub struct City {
        pub id: i32,
        pub country_id: i32,
    }

    #[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
    pub struct Country {
        pub id: i32,
    }

    impl LoadFrom<i32> for Country {
        type Error = Box<dyn std::error::Error>;
        type Context = super::Context;

        fn load(ids: &[i32], _: &(), ctx: &Self::Context) -> Result<Vec<Self>, Self::Error> {
            let models = ctx
                .db
                .countries
                .all_values()
                .into_iter()
                .filter(|value| ids.contains(&value.id))
                .cloned()
                .collect::<Vec<_>>();
            Ok(models)
        }
    }
}

pub struct Db {
    users: StatsHash<i32, models::User>,
    cities: StatsHash<i32, models::City>,
    countries: StatsHash<i32, models::Country>,
}

pub struct Context {
    db: Db,
}

impl juniper::Context for Context {}

pub struct Query;

impl QueryFields for Query {
    fn field_search<'a>(
        &self,
        executor: &Executor<'a, Context>,
        trail: &QueryTrail<'a, HasCountry, Walked>,
    ) -> FieldResult<Vec<HasCountry>> {
        let ctx = executor.context();

        let mut user_models = ctx
            .db
            .users
            .all_values()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let users = User::eager_load_each(&user_models, &ctx, &trail.downcast())?;

        let mut city_models = ctx
            .db
            .cities
            .all_values()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let cities = City::eager_load_each(&city_models, &ctx, &trail.downcast())?;

        let mut has_countries = vec![];
        has_countries.extend(users.into_iter().map(HasCountry::from).collect::<Vec<_>>());
        has_countries.extend(cities.into_iter().map(HasCountry::from).collect::<Vec<_>>());

        Ok(has_countries)
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug, EagerLoading)]
#[eager_loading(context = Context, error = Box<dyn std::error::Error>)]
pub struct User {
    user: models::User,
    #[has_one(default)]
    country: HasOne<Country>,
}

impl UserFields for User {
    fn field_id<'a>(&self, _: &Executor<'a, Context>) -> FieldResult<&i32> {
        Ok(&self.user.id)
    }

    fn field_country<'a>(
        &self,
        executor: &Executor<'a, Context>,
        trail: &QueryTrail<'a, Country, Walked>,
    ) -> FieldResult<&Country> {
        Ok(self.country.try_unwrap()?)
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug, EagerLoading)]
#[eager_loading(context = Context, error = Box<dyn std::error::Error>)]
pub struct City {
    city: models::City,
    #[has_one(default)]
    country: HasOne<Country>,
}

impl CityFields for City {
    fn field_id<'a>(&self, _: &Executor<'a, Context>) -> FieldResult<&i32> {
        Ok(&self.city.id)
    }

    fn field_country<'a>(
        &self,
        executor: &Executor<'a, Context>,
        trail: &QueryTrail<'a, Country, Walked>,
    ) -> FieldResult<&Country> {
        Ok(self.country.try_unwrap()?)
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug, EagerLoading)]
#[eager_loading(context = Context, error = Box<dyn std::error::Error>)]
pub struct Country {
    country: models::Country,
}

impl CountryFields for Country {
    fn field_id<'a>(&self, _: &Executor<'a, Context>) -> FieldResult<&i32> {
        Ok(&self.country.id)
    }
}

#[test]
fn loading_users_and_associations() {
    let mut countries = StatsHash::new("countries");
    let country = models::Country { id: 10 };
    countries.insert(country.id, country.clone());

    let mut users = StatsHash::new("users");
    let user = models::User {
        id: 10,
        country_id: country.id,
    };
    users.insert(user.id, user.clone());

    let mut cities = StatsHash::new("cities");
    let city = models::City {
        id: 10,
        country_id: country.id,
    };
    cities.insert(city.id, city.clone());

    let db = Db {
        users,
        countries,
        cities,
    };

    let (json, counts) = run_query(
        r#"
        query Test {
            search {
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
            "search": [
                { "country": { "id": country.id } },
                { "country": { "id": country.id } },
            ]
        }),
        actual: json.clone(),
    );

    assert_eq!(1, counts.user_reads);
    assert_eq!(1, counts.city_reads);
    assert_eq!(2, counts.country_reads);
}

struct DbStats {
    user_reads: usize,
    city_reads: usize,
    country_reads: usize,
}

fn run_query(query: &str, db: Db) -> (Value, DbStats) {
    let ctx = Context { db };

    let (result, errors) = juniper::execute(
        query,
        None,
        &Schema::new(Query, juniper::EmptyMutation::new()),
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
            city_reads: ctx.db.cities.reads_count(),
        },
    )
}
