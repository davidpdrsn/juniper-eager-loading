use assert_json_diff::{assert_json_eq, assert_json_include};
use juniper::{Executor, FieldResult};
use juniper_eager_loading::{prelude::*, Cache, EagerLoading, HasMany, HasOne, OptionHasOne};
use juniper_from_schema::graphql_schema;
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
    }

    #[derive(Clone, Debug)]
    pub struct City {
        pub id: i32,
        pub country_id: i32,
    }

    impl juniper_eager_loading::LoadFrom<i32> for Country {
        type Error = Box<dyn std::error::Error>;
        type Connection = super::Db;

        fn load(ids: &[i32], db: &Self::Connection) -> Result<Vec<Self>, Self::Error> {
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

    impl juniper_eager_loading::LoadFrom<i32> for City {
        type Error = Box<dyn std::error::Error>;
        type Connection = super::Db;

        fn load(ids: &[i32], db: &Self::Connection) -> Result<Vec<Self>, Self::Error> {
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

    impl juniper_eager_loading::LoadFrom<i32> for User {
        type Error = Box<dyn std::error::Error>;
        type Connection = super::Db;

        fn load(ids: &[i32], db: &Self::Connection) -> Result<Vec<Self>, Self::Error> {
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

    impl juniper_eager_loading::LoadFrom<Country> for City {
        type Error = Box<dyn std::error::Error>;
        type Connection = super::Db;

        fn load(countries: &[Country], db: &Self::Connection) -> Result<Vec<Self>, Self::Error> {
            let country_ids = countries
                .iter()
                .map(|country| country.id)
                .collect::<Vec<_>>();
            let mut cities = db
                .cities
                .all_values()
                .into_iter()
                .filter(|city| country_ids.contains(&city.country_id))
                .cloned()
                .collect::<Vec<_>>();
            Ok(cities)
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

        let mut cache =
            Cache::new_from::<models::User, _>(&user_models, |user| (user.id, user.clone()));

        let mut users = User::from_db_models(&user_models);
        User::eager_load_all_children_for_each(&mut users, &user_models, db, trail, &mut cache)?;

        Ok(users)
    }
}

pub struct Mutation;

impl MutationFields for Mutation {
    fn field_noop(&self, _executor: &Executor<'_, Context>) -> FieldResult<&bool> {
        Ok(&true)
    }
}

// The default values are commented out
#[derive(Clone, Debug, EagerLoading)]
#[eager_loading(
    connection = "Db",
    error = "Box<dyn std::error::Error>",
    // model = "models::User",
    // id = "i32",
    // root_model_field = "user"
)]
pub struct User {
    user: models::User,
    // #[has_one(
    //     model = "models::Country",
    //     foreign_key_field = "country_id",
    //     root_model_field = "country"
    // )]
    #[has_one(default)]
    country: HasOne<Country>,
    // #[has_one(
    //     model = "models::City",
    //     foreign_key_field = "city_id",
    //     root_model_field = "city"
    // )]
    #[option_has_one(default)]
    city: OptionHasOne<City>,
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

    fn field_city(
        &self,
        _executor: &Executor<'_, Context>,
        _trail: &QueryTrail<'_, City, Walked>,
    ) -> FieldResult<&Option<City>> {
        Ok(self.city.try_unwrap()?)
    }
}

// #[derive(Clone, Debug)]
#[derive(Clone, Debug, EagerLoading)]
#[eager_loading(
    model = "models::Country",
    connection = "Db",
    id = "i32",
    error = "Box<dyn std::error::Error>",
    root_model_field = "country"
)]
pub struct Country {
    country: models::Country,

    #[has_many(
        root_model_field = "city",
        // association_type = "many_to_many",
    )]
    cities: HasMany<City>,
}

impl CountryFields for Country {
    fn field_id(&self, _executor: &Executor<'_, Context>) -> FieldResult<&i32> {
        Ok(&self.country.id)
    }

    fn field_cities(
        &self,
        _executor: &Executor<'_, Context>,
        _trail: &QueryTrail<'_, City, Walked>,
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
    #[has_one(
        foreign_key_field = "country_id",
        model = "models::Country",
        root_model_field = "country"
    )]
    country: HasOne<Country>,
}

impl CityFields for City {
    fn field_id(&self, _executor: &Executor<'_, Context>) -> FieldResult<&i32> {
        Ok(&self.city.id)
    }

    fn field_country(
        &self,
        _executor: &Executor<'_, Context>,
        _trail: &QueryTrail<'_, Country, Walked>,
    ) -> FieldResult<&Country> {
        Ok(self.country.try_unwrap()?)
    }
}

#[test]
fn loading_users() {
    let mut countries = StatsHash::new("countries");
    let cities = StatsHash::new("cities");
    let mut users = StatsHash::new("users");

    let mut country = models::Country { id: 10 };
    let country_id = country.id;

    let other_city = models::City { id: 30, country_id };

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
    let mut countries = StatsHash::new("countries");
    let mut cities = StatsHash::new("cities");
    let mut users = StatsHash::new("users");

    let country = models::Country { id: 10 };

    countries.insert(country.id, country.clone());

    let city = models::City {
        id: 20,
        country_id: country.id,
    };
    cities.insert(city.id, city.clone());

    let other_city = models::City {
        id: 30,
        country_id: country.id,
    };
    cities.insert(other_city.id, other_city.clone());

    users.insert(
        1,
        models::User {
            id: 1,
            country_id: country.id,
            city_id: Some(other_city.id),
        },
    );
    users.insert(
        2,
        models::User {
            id: 2,
            country_id: country.id,
            city_id: Some(city.id),
        },
    );
    users.insert(
        3,
        models::User {
            id: 3,
            country_id: country.id,
            city_id: Some(city.id),
        },
    );
    users.insert(
        4,
        models::User {
            id: 4,
            country_id: country.id,
            city_id: None,
        },
    );
    users.insert(
        5,
        models::User {
            id: 5,
            country_id: country.id,
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
                city { id }
                country {
                    id
                    cities {
                        id
                    }
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
                    "city": { "id": other_city.id },
                    "country": {
                        "id": country.id,
                        "cities": [
                            // the order of the citites doesn't matter
                            {},
                            {},
                        ],
                    },
                },
                {
                    "id": 2,
                    "city": { "id": city.id }
                },
                {
                    "id": 3,
                    "city": { "id": city.id }
                },
                {
                    "id": 4,
                    "city": null
                },
                {
                    "id": 5,
                    "city": null
                },
            ]
        }),
        actual: json.clone(),
    );

    let json_cities = json["users"][0]["country"]["cities"].as_array().unwrap();
    for json_city in json_cities {
        let id = json_city["id"].as_i64().unwrap() as i32;
        assert!([city.id, other_city.id].contains(&id));
    }

    // TODO
    assert_eq!(1, counts.user_reads);
    assert_eq!(1, counts.country_reads);
    assert_eq!(2, counts.city_reads);
}

#[test]
#[ignore]
fn test_caching() {
    let mut users = StatsHash::new("users");
    let mut countries = StatsHash::new("countries");
    let mut cities = StatsHash::new("cities");

    let mut country = models::Country { id: 1 };

    let city = models::City {
        id: 2,
        country_id: country.id,
    };

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

struct StatsHash<K: Hash + Eq, V> {
    map: HashMap<K, V>,
    count: AtomicUsize,
    name: &'static str,
}

impl<K: Hash + Eq, V> StatsHash<K, V> {
    fn new(name: &'static str) -> Self {
        StatsHash {
            map: HashMap::default(),
            count: AtomicUsize::default(),
            name,
        }
    }

    #[allow(dead_code)]
    fn get<Q>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: ?Sized + Hash + Eq,
    {
        self.increment_reads_count();
        self.map.get(k)
    }

    #[allow(dead_code)]
    pub fn get_mut<Q: ?Sized>(&mut self, k: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.increment_reads_count();
        self.map.get_mut(k)
    }

    fn all_values(&self) -> Vec<&V> {
        self.increment_reads_count();
        self.map.iter().map(|(_, v)| v).collect()
    }

    fn reads_count(&self) -> usize {
        self.count.load(Ordering::SeqCst)
    }

    fn insert(&mut self, k: K, v: V) -> Option<V> {
        self.map.insert(k, v)
    }

    fn increment_reads_count(&self) {
        self.count.fetch_add(1, Ordering::SeqCst);
    }
}
