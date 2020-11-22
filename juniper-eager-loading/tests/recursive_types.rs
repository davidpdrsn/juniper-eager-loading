#![allow(unused_variables, unused_imports, dead_code, unused_mut)]

mod helpers;

use assert_json_diff::assert_json_include;
use helpers::StatsHash;
use juniper::{EmptyMutation, Executor, FieldResult, ID};
use juniper_eager_loading::{
    prelude::*, EagerLoading, HasManyThrough, HasOne, LoadChildrenOutput, LoadFrom, OptionHasOne,
};
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
        parent: User!
        grandParent: User @juniper(ownership: "as_ref")
    }
}

mod models {
    use juniper_eager_loading::LoadFrom;

    #[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
    pub struct User {
        pub id: i32,
        pub parent_id: i32,
        pub grand_parent_id: Option<i32>,
    }

    impl LoadFrom<i32> for User {
        type Error = Box<dyn std::error::Error>;
        type Context = super::Context;

        fn load(ids: &[i32], _: &(), ctx: &Self::Context) -> Result<Vec<Self>, Self::Error> {
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
}

pub struct Db {
    users: StatsHash<i32, models::User>,
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
        let ctx = executor.context();

        let mut user_models = ctx
            .db
            .users
            .all_values()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        user_models.sort_by_key(|user| user.id);

        let users = User::eager_load_each(&user_models, ctx, trail)?;

        Ok(users)
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug, EagerLoading)]
#[eager_loading(context = Context, error = Box<dyn std::error::Error>)]
pub struct User {
    user: models::User,

    #[has_one(root_model_field = user)]
    parent: HasOne<Box<User>>,

    #[option_has_one(root_model_field = user)]
    grand_parent: OptionHasOne<Box<User>>,
}

impl UserFields for User {
    fn field_id<'a>(&self, _: &Executor<'a, Context>) -> FieldResult<&i32> {
        Ok(&self.user.id)
    }

    fn field_parent<'a>(
        &self,
        executor: &Executor<'a, Context>,
        trail: &QueryTrail<'a, User, Walked>,
    ) -> FieldResult<&User> {
        Ok(self.parent.try_unwrap()?)
    }

    fn field_grand_parent<'a>(
        &self,
        executor: &Executor<'a, Context>,
        trail: &QueryTrail<'a, User, Walked>,
    ) -> FieldResult<Option<&User>> {
        let grand_parent = self
            .grand_parent
            .try_unwrap()?
            .as_ref()
            .map(|boxed| &**boxed);

        Ok(grand_parent)
    }
}

#[test]
fn loading_recursive_type() {
    let mut users = StatsHash::new("users");

    users.insert(
        1,
        models::User {
            id: 1,
            parent_id: 1,
            grand_parent_id: Some(1),
        },
    );

    let db = Db { users };

    let (json, counts) = run_query(
        r#"
        query Test {
            users {
                id
                parent {
                    id
                    parent {
                        id
                        grandParent {
                            id
                        }
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
                    "parent": {
                        "id": 1,
                        "parent": {
                            "id": 1,
                            "grandParent": {
                                "id": 1,
                            },
                        },
                    },
                },
            ]
        }),
        actual: json.clone(),
    );
}

struct DbStats {
    user_reads: usize,
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
        },
    )
}
