#![allow(unused_variables, unused_imports, dead_code, unused_mut)]

mod helpers;

use assert_json_diff::{assert_json_eq, assert_json_include};
use helpers::{SortedExtension, StatsHash};
use juniper::{Executor, FieldError, FieldResult};
use juniper_eager_loading::{
    prelude::*, EagerLoading, HasMany, HasManyThrough, HasOne, OptionHasOne,
};
use juniper_from_schema::graphql_schema;
use models::{CityId, CompanyId, CountryId, EmploymentId, IssueId, UserId};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{borrow::Borrow, collections::HashMap, hash::Hash};

graphql_schema! {
    schema {
      query: Query
      mutation: Mutation
    }

    type Query {
      user(id: Int!): User! @juniper(ownership: "owned")
      users: [User!]! @juniper(ownership: "owned")
    }

    type Mutation {
      noop: Boolean!
    }

    type User {
        id: Int!
        country: Country!
        city: City
        employments: [Employment!]! @juniper(ownership: "owned")
        companies: [Company!]! @juniper(ownership: "owned")
        issues: [Issue!]! @juniper(ownership: "owned")
        primaryEmployment: Employment @juniper(ownership: "owned")
        primaryCompany: Company @juniper(ownership: "owned")
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
        name: String!
    }

    type Employment {
        id: Int!
        user: User!
        company: Company!
    }

    type Issue {
        id: Int!
        title: String!
        reviewer: User
    }
}

mod models {
    macro_rules! make_model_ids {
        ( $($name:ident),* ) => {
            $(
                #[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Clone, Copy, Hash)]
                pub struct $name(i32);

                impl From<i32> for $name {
                    fn from(id: i32) -> $name {
                        $name(id)
                    }
                }

                impl std::ops::Deref for $name {
                    type Target = i32;

                    fn deref(&self) -> &i32 {
                        &self.0
                    }
                }
            )*
        }
    }

    make_model_ids!(UserId, CountryId, CityId, CompanyId, EmploymentId, IssueId);

    #[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
    pub struct User {
        pub id: UserId,
        pub country_id: CountryId,
        pub city_id: Option<CityId>,
    }

    #[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
    pub struct Country {
        pub id: CountryId,
    }

    #[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
    pub struct City {
        pub id: CityId,
        pub country_id: CountryId,
    }

    #[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
    pub struct Company {
        pub id: CompanyId,
        pub name: String,
    }

    #[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
    pub struct Employment {
        pub id: EmploymentId,
        pub user_id: UserId,
        pub company_id: CompanyId,
        pub primary: bool,
    }

    impl Employment {
        pub fn primary(&self, _: &super::Context) -> bool {
            self.primary
        }
    }

    #[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
    pub struct Issue {
        pub id: IssueId,
        pub title: String,
        pub reviewer_id: Option<UserId>,
    }

    impl juniper_eager_loading::LoadFrom<CountryId> for Country {
        type Error = Box<dyn std::error::Error>;
        type Context = super::Context;

        fn load(ids: &[CountryId], _: &(), ctx: &Self::Context) -> Result<Vec<Self>, Self::Error> {
            let countries = ctx
                .db
                .countries
                .all_values()
                .into_iter()
                .filter(|value| ids.contains(&value.id))
                .cloned()
                .collect::<Vec<_>>();
            Ok(countries)
        }
    }

    impl juniper_eager_loading::LoadFrom<CityId> for City {
        type Error = Box<dyn std::error::Error>;
        type Context = super::Context;

        fn load(ids: &[CityId], _: &(), ctx: &Self::Context) -> Result<Vec<Self>, Self::Error> {
            let countries = ctx
                .db
                .cities
                .all_values()
                .into_iter()
                .filter(|value| ids.contains(&value.id))
                .cloned()
                .collect::<Vec<_>>();
            Ok(countries)
        }
    }

    impl juniper_eager_loading::LoadFrom<UserId> for User {
        type Error = Box<dyn std::error::Error>;
        type Context = super::Context;

        fn load(ids: &[UserId], _: &(), ctx: &Self::Context) -> Result<Vec<Self>, Self::Error> {
            let models = ctx
                .db
                .users
                .all_values()
                .into_iter()
                .filter(|value| ids.contains(&value.id))
                .cloned()
                .collect::<Vec<_>>();
            Ok(models)
        }
    }

    impl juniper_eager_loading::LoadFrom<CompanyId> for Company {
        type Error = Box<dyn std::error::Error>;
        type Context = super::Context;

        fn load(ids: &[CompanyId], _: &(), ctx: &Self::Context) -> Result<Vec<Self>, Self::Error> {
            let models = ctx
                .db
                .companies
                .all_values()
                .into_iter()
                .filter(|value| ids.contains(&value.id))
                .cloned()
                .collect::<Vec<_>>();
            Ok(models)
        }
    }

    impl juniper_eager_loading::LoadFrom<EmploymentId> for Employment {
        type Error = Box<dyn std::error::Error>;
        type Context = super::Context;

        fn load(
            ids: &[EmploymentId],
            _: &(),
            ctx: &Self::Context,
        ) -> Result<Vec<Self>, Self::Error> {
            let models = ctx
                .db
                .employments
                .all_values()
                .into_iter()
                .filter(|value| ids.contains(&value.id))
                .cloned()
                .collect::<Vec<_>>();
            Ok(models)
        }
    }

    impl juniper_eager_loading::LoadFrom<IssueId> for Issue {
        type Error = Box<dyn std::error::Error>;
        type Context = super::Context;

        fn load(ids: &[IssueId], _: &(), ctx: &Self::Context) -> Result<Vec<Self>, Self::Error> {
            let models = ctx
                .db
                .issues
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
        type Context = super::Context;

        fn load(
            countries: &[Country],
            _: &(),
            ctx: &Self::Context,
        ) -> Result<Vec<Self>, Self::Error> {
            let country_ids = countries
                .iter()
                .map(|country| country.id)
                .collect::<Vec<_>>();
            let mut cities = ctx
                .db
                .cities
                .all_values()
                .into_iter()
                .filter(|city| country_ids.contains(&city.country_id))
                .cloned()
                .collect::<Vec<_>>();
            Ok(cities)
        }
    }

    impl juniper_eager_loading::LoadFrom<User> for Employment {
        type Error = Box<dyn std::error::Error>;
        type Context = super::Context;

        fn load(users: &[User], _: &(), ctx: &Self::Context) -> Result<Vec<Self>, Self::Error> {
            let user_ids = users.iter().map(|user| user.id).collect::<Vec<_>>();
            let employments = ctx
                .db
                .employments
                .all_values()
                .into_iter()
                .filter(|employment| user_ids.contains(&employment.user_id))
                .cloned()
                .collect::<Vec<_>>();
            Ok(employments)
        }
    }

    impl juniper_eager_loading::LoadFrom<Employment> for Company {
        type Error = Box<dyn std::error::Error>;
        type Context = super::Context;

        fn load(
            employments: &[Employment],
            _: &(),
            ctx: &Self::Context,
        ) -> Result<Vec<Self>, Self::Error> {
            let company_ids = employments
                .iter()
                .map(|employment| employment.company_id)
                .collect::<Vec<_>>();

            let employments = ctx
                .db
                .companies
                .all_values()
                .into_iter()
                .filter(|company| company_ids.contains(&company.id))
                .cloned()
                .collect::<Vec<_>>();

            Ok(employments)
        }
    }

    impl juniper_eager_loading::LoadFrom<User> for Issue {
        type Error = Box<dyn std::error::Error>;
        type Context = super::Context;

        fn load(users: &[User], _: &(), ctx: &Self::Context) -> Result<Vec<Self>, Self::Error> {
            let user_ids = users.iter().map(|user| Some(user.id)).collect::<Vec<_>>();
            let issues = ctx
                .db
                .issues
                .all_values()
                .into_iter()
                .filter(|issue| user_ids.contains(&issue.reviewer_id))
                .cloned()
                .collect::<Vec<_>>();
            Ok(issues)
        }
    }
}

pub struct Db {
    users: StatsHash<UserId, models::User>,
    countries: StatsHash<CountryId, models::Country>,
    cities: StatsHash<CityId, models::City>,
    companies: StatsHash<CompanyId, models::Company>,
    employments: StatsHash<EmploymentId, models::Employment>,
    issues: StatsHash<IssueId, models::Issue>,
}

pub struct Context {
    db: Db,
}

impl juniper::Context for Context {}

pub struct Query;

impl QueryFields for Query {
    fn field_user<'a>(
        &self,
        executor: &Executor<'a, Context>,
        trail: &QueryTrail<'a, User, Walked>,
        id: i32,
    ) -> FieldResult<User> {
        let ctx = executor.context();

        let user_model = ctx
            .db
            .users
            .get(&UserId::from(id))
            .ok_or("User not found")?
            .clone();
        let user = User::new_from_model(&user_model);
        let user = User::eager_load_all_children(user, &[user_model], ctx, trail)?;
        Ok(user)
    }

    fn field_users<'a>(
        &self,
        executor: &Executor<'a, Context>,
        trail: &QueryTrail<'a, User, Walked>,
    ) -> FieldResult<Vec<User>> {
        let ctx = executor.context();

        let mut user_models = ctx
            .db
            .users
            .all_values()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        user_models.sort_by_key(|user| user.id);

        let mut users = User::from_db_models(&user_models);
        User::eager_load_all_children_for_each(&mut users, &user_models, ctx, trail)?;

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
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug, EagerLoading)]
#[eager_loading(
    error = "Box<dyn std::error::Error>",
    context = "Context",
    // model = "models::User",
    // id = "i32",
    // root_model_field = "user"
)]
pub struct User {
    user: models::User,

    // #[has_one(
    //     foreign_key_field = "country_id",
    //     root_model_field = "country"
    // )]
    #[has_one(default)]
    country: HasOne<Country>,

    // #[has_one(
    //     foreign_key_field = "city_id",
    //     root_model_field = "city"
    // )]
    #[option_has_one(default)]
    city: OptionHasOne<City>,

    #[has_many(root_model_field = "employment")]
    employments: HasMany<Employment>,

    #[has_many_through(
        // model_field = "company",
        // join_model_field = "employment"
        join_model = "models::Employment",
    )]
    companies: HasManyThrough<Company>,

    #[has_many(
        root_model_field = "issue",
        foreign_key_field = "reviewer_id",
        foreign_key_optional
    )]
    issues: HasMany<Issue>,

    #[has_many(
        root_model_field = "employment",
        graphql_field = "primaryEmployment",
        predicate_method = "primary"
    )]
    primary_employments: HasMany<Employment>,

    #[has_many_through(
        join_model = "models::Employment",
        graphql_field = "primaryCompany",
        predicate_method = "primary"
    )]
    primary_companies: HasManyThrough<Company>,
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
        _executor: &Executor<'_, Context>,
        _trail: &QueryTrail<'_, Employment, Walked>,
    ) -> FieldResult<Vec<Employment>> {
        Ok(self.employments.try_unwrap()?.clone().sorted())
    }

    fn field_companies(
        &self,
        _executor: &Executor<'_, Context>,
        _trail: &QueryTrail<'_, Company, Walked>,
    ) -> FieldResult<Vec<Company>> {
        Ok(self.companies.try_unwrap()?.clone().sorted())
    }

    fn field_issues(
        &self,
        _executor: &Executor<'_, Context>,
        _trail: &QueryTrail<'_, Issue, Walked>,
    ) -> FieldResult<Vec<Issue>> {
        Ok(self.issues.try_unwrap()?.clone().sorted())
    }

    fn field_primary_employment(
        &self,
        executor: &Executor<'_, Context>,
        _trail: &QueryTrail<'_, Employment, Walked>,
    ) -> FieldResult<Option<Employment>> {
        let employments = self.primary_employments.try_unwrap()?;

        match employments.len() {
            0 => Ok(None),
            1 => {
                let employment = employments[0].clone();
                Ok(Some(employment))
            }
            n => panic!("more than one primary employment: {}", n),
        }
    }

    fn field_primary_company(
        &self,
        executor: &Executor<'_, Context>,
        _trail: &QueryTrail<'_, Company, Walked>,
    ) -> FieldResult<Option<Company>> {
        let companies = self.primary_companies.try_unwrap()?;

        match companies.len() {
            0 => Ok(None),
            1 => {
                let company = companies[0].clone();
                Ok(Some(company))
            }
            n => panic!("more than one primary company: {}", n),
        }
    }
}

// #[derive(Clone, Eq, PartialEq, Debug)]
#[derive(Clone, Eq, PartialEq, Debug, Ord, PartialOrd, EagerLoading)]
#[eager_loading(
    model = "models::Country",
    context = "Context",
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

#[derive(Clone, Eq, PartialEq, Debug, Ord, PartialOrd, EagerLoading)]
#[eager_loading(
    model = "models::City",
    id = "i32",
    context = "Context",
    error = "Box<dyn std::error::Error>",
    root_model_field = "city"
)]
pub struct City {
    city: models::City,
    #[has_one(foreign_key_field = "country_id", root_model_field = "country")]
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

#[derive(Clone, Eq, PartialEq, Debug, Ord, PartialOrd, EagerLoading)]
#[eager_loading(context = "Context", error = "Box<dyn std::error::Error>")]
pub struct Company {
    company: models::Company,
}

impl CompanyFields for Company {
    fn field_id(&self, _executor: &Executor<'_, Context>) -> FieldResult<&i32> {
        Ok(&self.company.id)
    }

    fn field_name(&self, _executor: &Executor<'_, Context>) -> FieldResult<&String> {
        Ok(&self.company.name)
    }
}

#[derive(Clone, Eq, PartialEq, Debug, Ord, PartialOrd, EagerLoading)]
#[eager_loading(context = "Context", error = "Box<dyn std::error::Error>")]
pub struct Employment {
    employment: models::Employment,
    #[has_one(default)]
    user: HasOne<User>,
    #[has_one(default)]
    company: HasOne<Company>,
}

impl EmploymentFields for Employment {
    fn field_id(&self, _executor: &Executor<'_, Context>) -> FieldResult<&i32> {
        Ok(&self.employment.id)
    }

    fn field_user(
        &self,
        _executor: &Executor<'_, Context>,
        _trail: &QueryTrail<'_, User, Walked>,
    ) -> FieldResult<&User> {
        Ok(self.user.try_unwrap()?)
    }

    fn field_company(
        &self,
        _executor: &Executor<'_, Context>,
        _trail: &QueryTrail<'_, Company, Walked>,
    ) -> FieldResult<&Company> {
        Ok(self.company.try_unwrap()?)
    }
}

#[derive(Clone, Eq, PartialEq, Debug, Ord, PartialOrd, EagerLoading)]
#[eager_loading(context = "Context", error = "Box<dyn std::error::Error>")]
pub struct Issue {
    issue: models::Issue,
    #[option_has_one(root_model_field = "user")]
    reviewer: OptionHasOne<User>,
}

impl IssueFields for Issue {
    fn field_id(&self, _executor: &Executor<'_, Context>) -> FieldResult<&i32> {
        Ok(&self.issue.id)
    }

    fn field_title(&self, _executor: &Executor<'_, Context>) -> FieldResult<&String> {
        Ok(&self.issue.title)
    }

    fn field_reviewer(
        &self,
        _executor: &Executor<'_, Context>,
        _trail: &QueryTrail<'_, User, Walked>,
    ) -> FieldResult<&Option<User>> {
        Ok(self.reviewer.try_unwrap()?)
    }
}

#[test]
fn loading_user() {
    let mut countries = StatsHash::new("countries");
    let cities = StatsHash::new("cities");
    let mut users = StatsHash::new("users");

    let mut country = models::Country {
        id: CountryId::from(10),
    };
    let country_id = country.id;

    let other_city = models::City {
        id: CityId::from(30),
        country_id,
    };

    countries.insert(country_id, country);

    users.insert(
        UserId::from(1),
        models::User {
            id: UserId::from(1),
            country_id,
            city_id: None,
        },
    );
    users.insert(
        UserId::from(2),
        models::User {
            id: UserId::from(2),
            country_id,
            city_id: None,
        },
    );

    let db = Db {
        users,
        countries,
        cities,
        employments: StatsHash::new("employments"),
        companies: StatsHash::new("companies"),
        issues: StatsHash::new("issues"),
    };
    let (json, counts) = run_query("query Test { user(id: 1) { id } }", db);

    assert_eq!(1, counts.user_reads);
    assert_eq!(0, counts.country_reads);
    assert_eq!(0, counts.city_reads);

    assert_json_include!(
        expected: json!({
            "user": { "id": 1 },
        }),
        actual: json,
    );
}

#[test]
fn loading_users() {
    let mut countries = StatsHash::new("countries");
    let cities = StatsHash::new("cities");
    let mut users = StatsHash::new("users");

    let mut country = models::Country {
        id: CountryId::from(10),
    };
    let country_id = country.id;

    let other_city = models::City {
        id: CityId::from(30),
        country_id,
    };

    countries.insert(country_id, country);

    users.insert(
        UserId::from(1),
        models::User {
            id: UserId::from(1),
            country_id,
            city_id: None,
        },
    );
    users.insert(
        UserId::from(2),
        models::User {
            id: UserId::from(2),
            country_id,
            city_id: None,
        },
    );

    let db = Db {
        users,
        countries,
        cities,
        employments: StatsHash::new("employments"),
        companies: StatsHash::new("companies"),
        issues: StatsHash::new("issues"),
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

    let country = models::Country {
        id: CountryId::from(10),
    };

    countries.insert(country.id, country.clone());

    let city = models::City {
        id: CityId::from(20),
        country_id: country.id,
    };
    cities.insert(city.id, city.clone());

    let other_city = models::City {
        id: CityId::from(30),
        country_id: country.id,
    };
    cities.insert(other_city.id, other_city.clone());

    users.insert(
        UserId::from(1),
        models::User {
            id: UserId::from(1),
            country_id: country.id,
            city_id: Some(other_city.id),
        },
    );
    users.insert(
        UserId::from(2),
        models::User {
            id: UserId::from(2),
            country_id: country.id,
            city_id: Some(city.id),
        },
    );
    users.insert(
        UserId::from(3),
        models::User {
            id: UserId::from(3),
            country_id: country.id,
            city_id: Some(city.id),
        },
    );
    users.insert(
        UserId::from(4),
        models::User {
            id: UserId::from(4),
            country_id: country.id,
            city_id: None,
        },
    );
    users.insert(
        UserId::from(5),
        models::User {
            id: UserId::from(5),
            country_id: country.id,
            city_id: Some(CityId::from(999)),
        },
    );

    let db = Db {
        users,
        countries,
        cities,
        employments: StatsHash::new("employments"),
        companies: StatsHash::new("companies"),
        issues: StatsHash::new("issue"),
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
                    "city": { "id": *other_city.id },
                    "country": {
                        "id": *country.id,
                        "cities": [
                            // the order of the citites doesn't matter
                            {},
                            {},
                        ],
                    },
                },
                {
                    "id": 2,
                    "city": { "id": *city.id }
                },
                {
                    "id": 3,
                    "city": { "id": *city.id }
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
        assert!([city.id, other_city.id].contains(&CityId::from(id)));
    }

    assert_eq!(1, counts.user_reads);
    assert_eq!(1, counts.country_reads);
    assert_eq!(2, counts.city_reads);
}

#[test]
fn test_caching() {
    let mut users = StatsHash::new("users");
    let mut countries = StatsHash::new("countries");
    let mut cities = StatsHash::new("cities");

    let mut country = models::Country {
        id: CountryId::from(1),
    };

    let city = models::City {
        id: CityId::from(2),
        country_id: country.id,
    };

    let user = models::User {
        id: UserId::from(3),
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
        employments: StatsHash::new("employments"),
        companies: StatsHash::new("companies"),
        issues: StatsHash::new("issues"),
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
    assert_eq!(3, counts.country_reads);
    assert_eq!(2, counts.city_reads);
}

#[test]
fn test_loading_has_many_through() {
    let mut cities = StatsHash::new("cities");
    let mut companies = StatsHash::new("companies");
    let mut countries = StatsHash::new("countries");
    let mut employments = StatsHash::new("employments");
    let mut users = StatsHash::new("users");

    let mut country = models::Country {
        id: CountryId::from(1),
    };
    countries.insert(country.id, country.clone());

    let mut tonsser = models::Company {
        id: CompanyId::from(2),
        name: "Tonsser".to_string(),
    };
    companies.insert(tonsser.id, tonsser.clone());

    let mut peakon = models::Company {
        id: CompanyId::from(3),
        name: "Peakon".to_string(),
    };
    companies.insert(peakon.id, peakon.clone());

    let user = models::User {
        id: UserId::from(4),
        country_id: country.id,
        city_id: None,
    };
    users.insert(user.id, user.clone());

    let mut tonsser_employment = models::Employment {
        id: EmploymentId::from(5),
        user_id: user.id,
        company_id: tonsser.id,
        primary: true,
    };
    employments.insert(tonsser_employment.id, tonsser_employment.clone());

    let mut peakon_employment = models::Employment {
        id: EmploymentId::from(6),
        user_id: user.id,
        company_id: peakon.id,
        primary: false,
    };
    employments.insert(peakon_employment.id, peakon_employment.clone());

    let db = Db {
        cities,
        companies,
        countries,
        employments,
        users,
        issues: StatsHash::new("issues"),
    };

    let (json, counts) = run_query(
        r#"
        query Test {
            users {
                id
                employments {
                    user { id }
                    company { id name }
                }
                companies { id name }
                primaryEmployment {
                    id
                }
                primaryCompany {
                    name
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
                    "id": *user.id,
                    "employments": [
                        {
                            "user": { "id": *user.id },
                            "company": { "id": *tonsser.id, "name": tonsser.name },
                        },
                        {
                            "user": { "id": *user.id },
                            "company": { "id": *peakon.id, "name": peakon.name },
                        },
                    ],
                    "companies": [
                        { "id": *tonsser.id, "name": tonsser.name },
                        { "id": *peakon.id, "name": peakon.name },
                    ],
                    "primaryEmployment": {
                        "id": *tonsser_employment.id,
                    },
                    "primaryCompany": {
                        "name": tonsser.name,
                    },
                },
            ],
        }),
        actual: json,
    );
}

#[test]
fn test_loading_has_many_fk_optional() {
    let mut countries = StatsHash::new("countries");
    let mut users = StatsHash::new("users");
    let mut issues = StatsHash::new("issues");

    let country = models::Country {
        id: CountryId::from(1),
    };
    countries.insert(country.id, country.clone());

    let user = models::User {
        id: UserId::from(2),
        country_id: country.id,
        city_id: None,
    };
    users.insert(user.id, user.clone());

    let assigned_issue = models::Issue {
        id: IssueId::from(3),
        title: "This issue is assigned to somebody".to_string(),
        reviewer_id: Some(user.id),
    };
    issues.insert(assigned_issue.id, assigned_issue.clone());

    let unassigned_issue = models::Issue {
        id: IssueId::from(4),
        title: "This issue hasn't been assigned to somebody".to_string(),
        reviewer_id: None,
    };
    issues.insert(unassigned_issue.id, unassigned_issue.clone());

    let db = Db {
        cities: StatsHash::new("cities"),
        companies: StatsHash::new("companies"),
        countries,
        employments: StatsHash::new("employments"),
        users,
        issues,
    };

    let (json, _counts) = run_query(
        r#"
        query Test {
            users {
                id
                issues {
                    id
                    title
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
                    "id": *user.id,
                    "issues": [
                        {
                            "id": *assigned_issue.id,
                            "title": assigned_issue.title,
                        },
                    ],
                },
            ],
        }),
        actual: json,
    );
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
            company_reads: ctx.db.companies.reads_count(),
            employment_reads: ctx.db.employments.reads_count(),
        },
    )
}
