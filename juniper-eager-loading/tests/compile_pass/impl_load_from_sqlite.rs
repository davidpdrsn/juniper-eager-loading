#[macro_use]
extern crate diesel;

use static_assertions::assert_impl_all;
use diesel::prelude::*;
use juniper_eager_loading::{LoadFrom, impl_load_from_for_diesel_mysql};

table! {
    users (id) {
        id -> Integer,
    }
}

table! {
    companies (id) {
        id -> Integer,
    }
}

table! {
    employments (id) {
        id -> Integer,
        user_id -> Integer,
        company_id -> Integer,
    }
}

#[derive(Queryable)]
struct User {
    id: i32,
}

#[derive(Queryable)]
struct Company {
    id: i32,
}

#[derive(Queryable)]
struct Employment {
    id: i32,
    user_id: i32,
    company_id: i32,
}

struct Context {
    db: SqliteConnection,
}

impl Context {
    fn db(&self) -> &SqliteConnection {
        &self.db
    }
}

impl_load_from_for_diesel_mysql! {
    (
        error = diesel::result::Error,
        context = Context,
    ) => {
        i32 -> (users, User),
        i32 -> (companies, Company),
        i32 -> (employments, Employment),
        User.id -> (employments.user_id, Employment),
        Company.id -> (employments.company_id, Employment),
        Employment.company_id -> (companies.id, Company),
        Employment.user_id -> (users.id, User),
    }
}

assert_impl_all!(User: LoadFrom<i32>, LoadFrom<Employment>, LoadFrom<Employment>);
assert_impl_all!(Company: LoadFrom<i32>);
assert_impl_all!(Employment: LoadFrom<i32>, LoadFrom<User>, LoadFrom<Company>);

fn main() {}
