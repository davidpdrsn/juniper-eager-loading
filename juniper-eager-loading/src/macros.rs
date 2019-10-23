/// This macro will implement [`LoadFrom`][] for Diesel models using the Postgres backend.
///
/// It'll use an [`= ANY`] which is only supported by Postgres.
///
/// [`LoadFrom`]: trait.LoadFrom.html
/// [`= ANY`]: http://docs.diesel.rs/diesel/expression_methods/trait.ExpressionMethods.html#method.eq_any
///
/// # Example usage
///
/// ```
/// #[macro_use]
/// extern crate diesel;
///
/// use diesel::pg::PgConnection;
/// use diesel::prelude::*;
/// use juniper_eager_loading::impl_load_from_for_diesel_pg;
/// #
/// # fn main() {}
///
/// table! {
///     users (id) {
///         id -> Integer,
///     }
/// }
///
/// table! {
///     companies (id) {
///         id -> Integer,
///     }
/// }
///
/// table! {
///     employments (id) {
///         id -> Integer,
///         user_id -> Integer,
///         company_id -> Integer,
///     }
/// }
///
/// #[derive(Queryable)]
/// struct User {
///     id: i32,
/// }
///
/// #[derive(Queryable)]
/// struct Company {
///     id: i32,
/// }
///
/// #[derive(Queryable)]
/// struct Employment {
///     id: i32,
///     user_id: i32,
///     company_id: i32,
/// }
///
/// impl_load_from_for_diesel_pg! {
///     (
///         error = diesel::result::Error,
///         connection = PgConnection,
///     ) => {
///         i32 -> (users, User),
///         i32 -> (companies, Company),
///         i32 -> (employments, Employment),
///
///         User.id -> (employments.user_id, Employment),
///         Company.id -> (employments.company_id, Employment),
///
///         Employment.company_id -> (companies.id, Company),
///         Employment.user_id -> (users.id, User),
///     }
/// }
/// ```
///
/// # Syntax
///
/// First you specify your error and connection type with
///
/// ```text
/// (
///     error = diesel::result::Error,
///     connection = PgConnection,
/// ) => {
///     // ...
/// }
/// ```
///
/// Then you define each model type you want to implement [`LoadFrom`] for and which columns and
/// tables to use. There are two possible syntaxes for different purposes.
///
/// ```text
/// i32 -> (users, User),
/// ```
///
/// The first syntax implements `LoadFrom<i32> for User`, meaning from a `Vec<i32>` we can load a
/// `Vec<User>`. It just takes the id type, the table, and the model struct.
///
/// ```text
/// User.id -> (employments.user_id, Employment),
/// ```
///
/// This syntax is required when using [`HasMany`][] and [`HasManyThrough`][]. In this case it
/// implements `LoadFrom<User> for Employment`, meaning from a `Vec<User>` we can get
/// `Vec<Employment>`. It does this by loading the users, mapping the list to the user ids,
/// then finding the employments with those ids.
///
/// [`HasMany`]: trait.HasMany.html
/// [`HasManyThrough`]: trait.HasManyThrough.html
///
/// # What gets generated
///
/// The two syntaxes generates code like this:
///
/// ```
/// # #[macro_use]
/// # extern crate diesel;
/// # use diesel::pg::PgConnection;
/// # use diesel::prelude::*;
/// # use juniper_eager_loading::impl_load_from_for_diesel_pg;
/// # fn main() {}
/// # table! {
/// #     users (id) {
/// #         id -> Integer,
/// #     }
/// # }
/// # table! {
/// #     companies (id) {
/// #         id -> Integer,
/// #     }
/// # }
/// # table! {
/// #     employments (id) {
/// #         id -> Integer,
/// #         user_id -> Integer,
/// #         company_id -> Integer,
/// #     }
/// # }
/// # #[derive(Queryable)]
/// # struct User {
/// #     id: i32,
/// # }
/// # #[derive(Queryable)]
/// # struct Company {
/// #     id: i32,
/// # }
/// # #[derive(Queryable)]
/// # struct Employment {
/// #     id: i32,
/// #     user_id: i32,
/// #     company_id: i32,
/// # }
/// // i32 -> (users, User),
/// impl juniper_eager_loading::LoadFrom<i32> for User {
///     type Error = diesel::result::Error;
///     type Connection = PgConnection;
///
///     fn load(ids: &[i32], field_args: &(), db: &Self::Connection) -> Result<Vec<Self>, Self::Error> {
///         use diesel::pg::expression::dsl::any;
///
///         users::table
///             .filter(users::id.eq(any(ids)))
///             .load::<User>(db)
///             .map_err(From::from)
///     }
/// }
///
/// // User.id -> (employments.user_id, Employment),
/// impl juniper_eager_loading::LoadFrom<User> for Employment {
///     type Error = diesel::result::Error;
///     type Connection = PgConnection;
///
///     fn load(froms: &[User], field_args: &(), db: &Self::Connection) -> Result<Vec<Self>, Self::Error> {
///         use diesel::pg::expression::dsl::any;
///
///         let from_ids = froms.iter().map(|other| other.id).collect::<Vec<_>>();
///         employments::table
///             .filter(employments::user_id.eq(any(from_ids)))
///             .load(db)
///             .map_err(From::from)
///     }
/// }
/// ```
#[macro_export]
macro_rules! impl_load_from_for_diesel_pg {
    ( $($token:tt)* ) => {
        $crate::proc_macros::impl_load_from_for_diesel_pg!($($token)*);
    }
}

/// This macro will implement [`LoadFrom`][] for Diesel models using the MySQL backend.
///
/// For more details see [`impl_load_from_for_diesel_pg`][].
///
/// [`impl_load_from_for_diesel_pg`]: macro.impl_load_from_for_diesel_pg.html
/// [`LoadFrom`]: trait.LoadFrom.html
///
/// # Example usage
///
/// ```
/// #[macro_use]
/// extern crate diesel;
///
/// use diesel::mysql::MysqlConnection;
/// use diesel::prelude::*;
/// use juniper_eager_loading::impl_load_from_for_diesel_mysql;
/// #
/// # fn main() {}
///
/// table! {
///     users (id) {
///         id -> Integer,
///     }
/// }
///
/// table! {
///     companies (id) {
///         id -> Integer,
///     }
/// }
///
/// table! {
///     employments (id) {
///         id -> Integer,
///         user_id -> Integer,
///         company_id -> Integer,
///     }
/// }
///
/// #[derive(Queryable)]
/// struct User {
///     id: i32,
/// }
///
/// #[derive(Queryable)]
/// struct Company {
///     id: i32,
/// }
///
/// #[derive(Queryable)]
/// struct Employment {
///     id: i32,
///     user_id: i32,
///     company_id: i32,
/// }
///
/// impl_load_from_for_diesel_mysql! {
///     (
///         error = diesel::result::Error,
///         connection = MysqlConnection,
///     ) => {
///         i32 -> (users, User),
///         i32 -> (companies, Company),
///         i32 -> (employments, Employment),
///
///         User.id -> (employments.user_id, Employment),
///         Company.id -> (employments.company_id, Employment),
///
///         Employment.company_id -> (companies.id, Company),
///         Employment.user_id -> (users.id, User),
///     }
/// }
/// ```
#[macro_export]
macro_rules! impl_load_from_for_diesel_mysql {
    ( $($token:tt)* ) => {
        $crate::proc_macros::impl_load_from_for_diesel_mysql!($($token)*);
    }
}

/// This macro will implement [`LoadFrom`][] for Diesel models using the SQLite backend.
///
/// For more details see [`impl_load_from_for_diesel_pg`][].
///
/// [`impl_load_from_for_diesel_pg`]: macro.impl_load_from_for_diesel_pg.html
/// [`LoadFrom`]: trait.LoadFrom.html
///
/// # Example usage
///
/// ```
/// #[macro_use]
/// extern crate diesel;
///
/// use diesel::sqlite::SqliteConnection;
/// use diesel::prelude::*;
/// use juniper_eager_loading::impl_load_from_for_diesel_sqlite;
/// #
/// # fn main() {}
///
/// table! {
///     users (id) {
///         id -> Integer,
///     }
/// }
///
/// table! {
///     companies (id) {
///         id -> Integer,
///     }
/// }
///
/// table! {
///     employments (id) {
///         id -> Integer,
///         user_id -> Integer,
///         company_id -> Integer,
///     }
/// }
///
/// #[derive(Queryable)]
/// struct User {
///     id: i32,
/// }
///
/// #[derive(Queryable)]
/// struct Company {
///     id: i32,
/// }
///
/// #[derive(Queryable)]
/// struct Employment {
///     id: i32,
///     user_id: i32,
///     company_id: i32,
/// }
///
/// impl_load_from_for_diesel_sqlite! {
///     (
///         error = diesel::result::Error,
///         connection = SqliteConnection,
///     ) => {
///         i32 -> (users, User),
///         i32 -> (companies, Company),
///         i32 -> (employments, Employment),
///
///         User.id -> (employments.user_id, Employment),
///         Company.id -> (employments.company_id, Employment),
///
///         Employment.company_id -> (companies.id, Company),
///         Employment.user_id -> (users.id, User),
///     }
/// }
/// ```
#[macro_export]
macro_rules! impl_load_from_for_diesel_sqlite {
    ( $($token:tt)* ) => {
        $crate::proc_macros::impl_load_from_for_diesel_sqlite!($($token)*);
    }
}
