use assert_json_diff::{assert_json_eq, assert_json_include};
use juniper::{Executor, FieldResult};
use juniper_eager_loading::{prelude::*, Cache, DbEdge, EagerLoading, OptionDbEdge, VecDbEdge};
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
        employments: [Employment!]!
    }

    type Country {
        id: Int!
        cities: [City!]!
    }

    type City {
        id: Int!
        country: Country!
    }

    type Company {
        id: Int!
    }

    type Employment {
        id: Int!
        user: User!
        company: Company!
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

    #[derive(Clone, Debug)]
    pub struct Company {
        pub id: i32,
    }

    #[derive(Clone, Debug)]
    pub struct Employment {
        pub id: i32,
        pub user_id: i32,
        pub company_id: i32,
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

    impl juniper_eager_loading::LoadFromIds for User {
        type Id = i32;
        type Error = Box<dyn std::error::Error>;
        type Connection = super::Db;

        fn load(ids: &[Self::Id], db: &Self::Connection) -> Result<Vec<Self>, Self::Error> {
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

    impl juniper_eager_loading::LoadFromIds for Company {
        type Id = i32;
        type Error = Box<dyn std::error::Error>;
        type Connection = super::Db;

        fn load(ids: &[Self::Id], db: &Self::Connection) -> Result<Vec<Self>, Self::Error> {
            let models = db
                .companies
                .all_values()
                .into_iter()
                .filter(|value| ids.contains(&value.id))
                .cloned()
                .collect::<Vec<_>>();
            Ok(models)
        }
    }

    impl juniper_eager_loading::LoadFromIds for Employment {
        type Id = i32;
        type Error = Box<dyn std::error::Error>;
        type Connection = super::Db;

        fn load(ids: &[Self::Id], db: &Self::Connection) -> Result<Vec<Self>, Self::Error> {
            let models = db
                .employments
                .all_values()
                .into_iter()
                .filter(|value| ids.contains(&value.id))
                .cloned()
                .collect::<Vec<_>>();
            Ok(models)
        }
    }

    impl juniper_eager_loading::LoadFromModels<User> for Employment {
        type Error = Box<dyn std::error::Error>;
        type Connection = super::Db;

        fn load(models: &[User], db: &Self::Connection) -> Result<Vec<Employment>, Self::Error> {
            let user_ids = models.iter().map(|user| user.id).collect::<Vec<_>>();
            let employments = db
                .employments
                .all_values()
                .into_iter()
                .filter(|employment| user_ids.contains(&employment.user_id))
                .cloned()
                .collect::<Vec<_>>();
            Ok(employments)
        }
    }
}

pub struct Db {
    users: StatsHash<i32, models::User>,
    countries: StatsHash<i32, models::Country>,
    cities: StatsHash<i32, models::City>,
    companies: StatsHash<i32, models::Company>,
    employments: StatsHash<i32, models::Employment>,
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

        let mut cache = Cache::new_from::<models::User, _>(&user_models, |user| {
            let key = user.id;
            let value = user.clone();
            (key, value)
        });

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

    #[eager_loading(
        foreign_key_field = "user_id",
        model = "models::Employment",
        root_model_field = "employment",
        association_type = "many_to_many"
    )]
    employments: VecDbEdge<Employment>,
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

    fn field_employments(
        &self,
        executor: &Executor<'_, Context>,
        _trail: &QueryTrail<'_, Employment, Walked>,
    ) -> FieldResult<&Vec<Employment>> {
        Ok(self.employments.try_unwrap()?)
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
        model = "models::City",
        association_type = "one_to_many"
    )]
    cities: VecDbEdge<City>,
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
    #[eager_loading(foreign_key_field = "country_id", model = "models::Country")]
    country: DbEdge<Country>,
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

#[derive(Clone, Debug, EagerLoading)]
#[eager_loading(
    model = "models::Company",
    id = "i32",
    connection = "Db",
    error = "Box<dyn std::error::Error>",
    root_model_field = "Company"
)]
pub struct Company {
    company: models::Company,
}

impl CompanyFields for Company {
    fn field_id(&self, _executor: &Executor<'_, Context>) -> FieldResult<&i32> {
        Ok(&self.company.id)
    }
}

#[derive(Clone, Debug, EagerLoading)]
#[eager_loading(
    model = "models::Employment",
    id = "i32",
    connection = "Db",
    error = "Box<dyn std::error::Error>",
    root_model_field = "employment"
)]
pub struct Employment {
    employment: models::Employment,
    #[eager_loading(foreign_key_field = "user_id", model = "models::User")]
    user: DbEdge<User>,
    #[eager_loading(foreign_key_field = "company_id", model = "models::Company")]
    company: DbEdge<Company>,
}

impl EmploymentFields for Employment {
    fn field_id(&self, _executor: &Executor<'_, Context>) -> FieldResult<&i32> {
        Ok(&self.employment.id)
    }

    fn field_user(
        &self,
        executor: &Executor<'_, Context>,
        trail: &QueryTrail<'_, User, Walked>,
    ) -> FieldResult<&User> {
        Ok(self.user.try_unwrap()?)
    }

    fn field_company(
        &self,
        executor: &Executor<'_, Context>,
        trail: &QueryTrail<'_, Company, Walked>,
    ) -> FieldResult<&Company> {
        Ok(self.company.try_unwrap()?)
    }
}

#[test]
fn loading_users() {
    let mut countries = StatsHash::new("countries");
    let cities = StatsHash::new("cities");
    let mut users = StatsHash::new("users");
    let mut companies = StatsHash::new("companies");

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
        companies,
        employments: StatsHash::new("employments"),
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
    let mut companies = StatsHash::new("companies");

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
        companies,
        employments: StatsHash::new("employments"),
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
    let mut users = StatsHash::new("users");
    let mut countries = StatsHash::new("countries");
    let mut cities = StatsHash::new("cities");
    let mut companies = StatsHash::new("companies");

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
        companies,
        employments: StatsHash::new("employments"),
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

#[test]
fn loading_companies_for_users() {
    let mut users = StatsHash::new("users");
    let mut countries = StatsHash::new("countries");
    let mut cities = StatsHash::new("cities");
    let mut companies = StatsHash::new("companies");
    let mut employments = StatsHash::new("employments");

    let country = models::Country {
        id: 1,
        city_ids: vec![],
    };

    let company = models::Company { id: 2 };
    companies.insert(company.id, company.clone());

    let user = models::User {
        id: 3,
        country_id: country.id,
        city_id: None,
    };

    let employment = models::Employment {
        id: 4,
        user_id: user.id,
        company_id: company.id,
    };
    employments.insert(employment.id, employment.clone());

    users.insert(user.id, user.clone());
    countries.insert(country.id, country.clone());

    let db = Db {
        users,
        countries,
        cities,
        companies,
        employments,
    };

    let (json, counts) = run_query(
        r#"
        query Test {
            users {
                id
                employments {
                    id
                    company { id }
                    user { id }
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
                    "id": user.id,
                    "employments": [
                        {
                            "id": employment.id,
                            "company": { "id": company.id },
                            "user": { "id": user.id },
                        }
                    ]
                },
            ]
        }),
        json,
    );

    assert_eq!(1, counts.user_reads, "user reads");
    assert_eq!(1, counts.company_reads, "company reads");
    assert_eq!(1, counts.employment_reads, "employment reads");
}

struct DbStats {
    user_reads: usize,
    country_reads: usize,
    city_reads: usize,
    company_reads: usize,
    employment_reads: usize,
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
            company_reads: ctx.db.companies.reads_count(),
            employment_reads: ctx.db.employments.reads_count(),
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
