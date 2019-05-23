use assert_json_diff::{assert_json_eq, assert_json_include};
use juniper::{Executor, FieldResult};
use juniper_eager_loading::{prelude::*, Cache, DbEdge, EagerLoading, OptionDbEdge, VecDbEdge};
use juniper_from_schema::{graphql_schema, Walked};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{borrow::Borrow, collections::HashMap, hash::Hash};

graphql_schema! {
    schema {
      query: Query
      mutation: Mutation
    }

    type Query {
      users: [User!]! @juniper(ownership: "owned")
    }

    type Mutation {
      noop: Boolean!
    }

    type User {
        id: Int!
        country: Country!
        city: City
    }

    type Country {
        id: Int!
        cities: [City!]!
    }

    type City {
        id: Int!
        country: Country!
    }
}

mod models {
    #[derive(Clone, Debug)]
    pub struct User {
        pub id: i32,
        pub country_id: i32,
        pub city_id: Option<i32>,
    }

    #[derive(Clone, Debug)]
    pub struct Country {
        pub id: i32,
        pub city_ids: Vec<i32>,
    }

    #[derive(Clone, Debug)]
    pub struct City {
        pub id: i32,
        pub country_id: i32,
    }

    impl juniper_eager_loading::LoadFromIds for Country {
        type Id = i32;
        type Error = Box<dyn std::error::Error>;
        type Connection = super::Db;

        fn load(ids: &[Self::Id], db: &Self::Connection) -> Result<Vec<Self>, Self::Error> {
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

    impl juniper_eager_loading::LoadFromIds for City {
        type Id = i32;
        type Error = Box<dyn std::error::Error>;
        type Connection = super::Db;

        fn load(ids: &[Self::Id], db: &Self::Connection) -> Result<Vec<Self>, Self::Error> {
            let countries = db
                .cities
                .all_values()
                .into_iter()
                .filter(|value| ids.contains(&value.id))
                .cloned()
                .collect::<Vec<_>>();
            Ok(countries)
        }
    }
}

pub struct Db {
    users: StatsHash<i32, models::User>,
    countries: StatsHash<i32, models::Country>,
    cities: StatsHash<i32, models::City>,
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

        let mut cache = Cache::new();
        User::eager_load_all_children_for_each(&mut users, &user_models, db, trail, &mut cache)?;

        Ok(users)
    }
}

pub struct Mutation;

impl MutationFields for Mutation {
    fn field_noop(&self, executor: &Executor<'_, Context>) -> FieldResult<&bool> {
        Ok(&true)
    }
}

#[derive(Clone, Debug, EagerLoading)]
#[eager_loading(
    model = "models::User",
    id = "i32",
    connection = "Db",
    error = "Box<dyn std::error::Error>",
    root_model_field = "user"
)]
pub struct User {
    user: models::User,
    #[eager_loading(foreign_key_field = "country_id", model = "models::Country")]
    country: DbEdge<Country>,
    #[eager_loading(foreign_key_field = "city_id", model = "models::City")]
    city: OptionDbEdge<City>,
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
        Ok(self.country.try_unwrap()?)
    }

    fn field_city(
        &self,
        executor: &Executor<'_, Context>,
        trail: &QueryTrail<'_, City, Walked>,
    ) -> FieldResult<&Option<City>> {
        Ok(self.city.try_unwrap()?)
    }
}

#[derive(Clone, Debug, EagerLoading)]
#[eager_loading(
    model = "models::Country",
    connection = "Db",
    error = "Box<dyn std::error::Error>",
    root_model_field = "country"
)]
pub struct Country {
    country: models::Country,

    #[eager_loading(
        foreign_key_field = "city_ids",
        root_model_field = "city",
        model = "models::City"
    )]
    cities: VecDbEdge<City>,
}

impl CountryFields for Country {
    fn field_id(&self, executor: &Executor<'_, Context>) -> FieldResult<&i32> {
        Ok(&self.country.id)
    }

    fn field_cities(
        &self,
        executor: &Executor<'_, Context>,
        trail: &QueryTrail<'_, City, Walked>,
    ) -> FieldResult<&Vec<City>> {
        Ok(self.cities.try_unwrap()?)
    }
}

#[derive(Clone, Debug, EagerLoading)]
#[eager_loading(
    model = "models::City",
    id = "i32",
    connection = "Db",
    error = "Box<dyn std::error::Error>",
    root_model_field = "city"
)]
pub struct City {
    city: models::City,
    #[eager_loading(foreign_key_field = "country_id", model = "models::Country")]
    country: DbEdge<Country>,
}

impl CityFields for City {
    fn field_id(&self, executor: &Executor<'_, Context>) -> FieldResult<&i32> {
        Ok(&self.city.id)
    }

    fn field_country(
        &self,
        executor: &Executor<'_, Context>,
        trail: &QueryTrail<'_, Country, Walked>,
    ) -> FieldResult<&Country> {
        Ok(self.country.try_unwrap()?)
    }
}

#[test]
fn loading_users() {
    let mut countries = StatsHash::default();
    let mut cities = StatsHash::default();
    let mut users = StatsHash::default();

    let mut country = models::Country {
        id: 10,
        city_ids: vec![],
    };
    let country_id = country.id;

    let other_city = models::City { id: 30, country_id };
    country.city_ids.push(other_city.id);

    countries.insert(country_id, country);

    users.insert(
        1,
        models::User {
            id: 1,
            country_id,
            city_id: None,
        },
    );
    users.insert(
        2,
        models::User {
            id: 2,
            country_id,
            city_id: None,
        },
    );

    let db = Db {
        users,
        countries,
        cities,
    };
    let (json, counts) = run_query("query Test { users { id } }", db);

    assert_eq!(1, counts.user_reads);
    assert_eq!(0, counts.country_reads);
    assert_eq!(0, counts.city_reads);

    assert_json_include!(
        expected: json!({
            "users": [
                { "id": 1 },
                { "id": 2 },
            ]
        }),
        actual: json,
    );
}

#[test]
fn loading_users_and_associations() {
    let mut countries = StatsHash::default();
    let mut cities = StatsHash::default();
    let mut users = StatsHash::default();

    let mut country = models::Country {
        id: 10,
        city_ids: vec![],
    };
    let country_id = country.id;

    let other_city = models::City { id: 30, country_id };
    let other_city_id = other_city.id;
    country.city_ids.push(other_city.id);

    countries.insert(country_id, country);

    let city = models::City { id: 20, country_id };
    let city_id = city.id;
    cities.insert(city_id, city);
    cities.insert(other_city.id, other_city);

    users.insert(
        1,
        models::User {
            id: 1,
            country_id,
            city_id: Some(other_city_id),
        },
    );
    users.insert(
        2,
        models::User {
            id: 2,
            country_id,
            city_id: Some(city_id),
        },
    );
    users.insert(
        3,
        models::User {
            id: 3,
            country_id,
            city_id: Some(city_id),
        },
    );
    users.insert(
        4,
        models::User {
            id: 4,
            country_id,
            city_id: None,
        },
    );
    users.insert(
        5,
        models::User {
            id: 5,
            country_id,
            city_id: Some(999),
        },
    );

    let db = Db {
        users,
        countries,
        cities,
    };

    let (json, counts) = run_query(
        r#"
        query Test {
            users {
                id
                country {
                    id
                    cities {
                        id
                    }
                }
                city { id }
            }
        }
    "#,
        db,
    );

    assert_json_eq!(
        json!({
            "users": [
                { "id": 1, "country": { "id": 10, "cities": [{ "id": 30 }] }, "city": { "id": 30 } },
                { "id": 2, "country": { "id": 10, "cities": [{ "id": 30 }] }, "city": { "id": 20 } },
                { "id": 3, "country": { "id": 10, "cities": [{ "id": 30 }] }, "city": { "id": 20 } },
                { "id": 4, "country": { "id": 10, "cities": [{ "id": 30 }] }, "city": null },
                { "id": 5, "country": { "id": 10, "cities": [{ "id": 30 }] }, "city": null },
            ]
        }),
        json,
    );

    assert_eq!(1, counts.user_reads);
    assert_eq!(1, counts.country_reads);
    assert_eq!(2, counts.city_reads);
}

#[test]
fn test_caching() {
    let mut users = StatsHash::default();
    let mut countries = StatsHash::default();
    let mut cities = StatsHash::default();

    let mut country = models::Country {
        id: 1,
        city_ids: vec![],
    };

    let city = models::City {
        id: 2,
        country_id: country.id,
    };

    country.city_ids.push(city.id);

    let user = models::User {
        id: 3,
        country_id: country.id,
        city_id: Some(city.id),
    };

    users.insert(user.id, user);
    countries.insert(country.id, country);
    cities.insert(city.id, city);

    let db = Db {
        users,
        countries,
        cities,
    };

    let (json, counts) = run_query(
        r#"
        query Test {
            users {
                id
                country {
                    id
                    cities {
                        id
                        country { id }
                    }
                }
                city {
                    id
                    country { id }
                }
            }
        }
    "#,
        db,
    );

    assert_json_eq!(
        json!({
            "users": [
                {
                    "id": 3,
                    "city": {
                        "id": 2,
                        "country": { "id": 1 }
                    },
                    "country": {
                        "id": 1,
                        "cities": [
                            {
                                "id": 2,
                                "country": { "id": 1 }
                            },
                        ],
                    },
                },
            ]
        }),
        json,
    );

    assert_eq!(1, counts.user_reads);
    assert_eq!(1, counts.country_reads);
    assert_eq!(1, counts.city_reads);
}

struct DbStats {
    user_reads: usize,
    country_reads: usize,
    city_reads: usize,
}

fn run_query(query: &str, db: Db) -> (Value, DbStats) {
    let ctx = Context { db };

    let (result, errors) = juniper::execute(
        query,
        None,
        &Schema::new(Query, Mutation),
        &juniper::Variables::new(),
        &ctx,
    )
    .unwrap();

    if !errors.is_empty() {
        panic!("GraphQL errors: {:?}", errors);
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

struct StatsHash<K: Hash + Eq, V>(HashMap<K, V>, AtomicUsize);

impl<K: Hash + Eq, V> Default for StatsHash<K, V> {
    fn default() -> Self {
        StatsHash(HashMap::default(), AtomicUsize::default())
    }
}

impl<K: Hash + Eq, V> StatsHash<K, V> {
    fn get<Q>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: ?Sized + Hash + Eq,
    {
        self.increment_reads_count();
        self.0.get(k)
    }

    pub fn get_mut<Q: ?Sized>(&mut self, k: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.increment_reads_count();
        self.0.get_mut(k)
    }

    fn all_values(&self) -> Vec<&V> {
        self.increment_reads_count();
        self.0.iter().map(|(_, v)| v).collect()
    }

    fn reads_count(&self) -> usize {
        self.1.load(Ordering::SeqCst)
    }

    fn insert(&mut self, k: K, v: V) -> Option<V> {
        self.0.insert(k, v)
    }

    fn increment_reads_count(&self) {
        self.1.fetch_add(1, Ordering::SeqCst);
    }
}
