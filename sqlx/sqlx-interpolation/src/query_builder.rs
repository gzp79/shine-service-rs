use crate::{DBBuilderError, DBKind};
use chrono::{DateTime, Utc};
use sqlx::{
    database::HasArguments,
    query::{Query, QueryAs},
    Database, Encode, FromRow, Type,
};

pub trait SqlBuilderExpression<'q> {
    fn add_to_query<'a>(self, query: &'a mut QueryBuilder<'q>) -> &'a mut QueryBuilder<'q>;
}

/// Store a variable to bind into sql.
/// It is safe for sql injection vulnerability
#[derive(Debug)]
pub enum Value<'a> {
    Bool(bool),
    OptionBool(Option<bool>),
    I32(i32),
    OptionI32(Option<i32>),
    String(String),
    OptionString(Option<String>),
    Str(&'a str),
    OptionStr(Option<&'a str>),
    DateTimeUtc(DateTime<Utc>),
    OptionDateTimeUtc(DateTime<Utc>),
    Binary(&'a [u8]),
    OptionBinary(Option<&'a [u8]>),
}

macro_rules! impl_value_expr {
    ($id:ident : $t:ty => $e:expr) => {
        impl<'q> SqlBuilderExpression<'q> for $t {
            fn add_to_query<'a>(self, query: &'a mut QueryBuilder<'q>) -> &'a mut QueryBuilder<'q> {
                let $id = self;
                query.value($e)
            }
        }
    };
}

impl_value_expr!(v: bool => Value::Bool(v));
impl_value_expr!(v: Option<bool> => Value::OptionBool(v));
impl_value_expr!(v: &'q bool => Value::Bool(*v));
impl_value_expr!(v: Option<&'q bool> => Value::OptionBool(v.cloned()));
impl_value_expr!(v: i32 => Value::I32(v));
impl_value_expr!(v: Option<i32> => Value::OptionI32(v));
impl_value_expr!(v: &'q i32 => Value::I32(*v));
impl_value_expr!(v: Option<&'q i32> => Value::OptionI32(v.cloned()));

impl_value_expr!(v: &'q str => Value::Str(v));
impl_value_expr!(v: &'q &'q str => Value::Str(v)); // helper to resolve iteration over slice of str as it returns &&
impl_value_expr!(v: Option<&'q str> => Value::OptionStr(v));
impl_value_expr!(v: &'q Option<&'q str> => Value::OptionStr(v.as_deref()));

impl_value_expr!(v: &'q String => Value::Str(v.as_str()));
impl_value_expr!(v: Option<&'q String> => Value::OptionStr(v.map(String::as_str)));
impl_value_expr!(v: &'q Option<String> => Value::OptionStr(v.as_deref()));
impl_value_expr!(v: &'q Option<&'q String> => Value::OptionStr(v.map(String::as_str)));

impl_value_expr!(v: &'q [u8] => Value::Binary(v));
impl_value_expr!(v: &'q Vec<u8> => Value::Binary(&v[..]));
impl_value_expr!(v: Option<&'q [u8]> => Value::OptionBinary(v));
impl_value_expr!(v: Option<&'q Vec<u8>> => Value::OptionBinary(v.map(|v| &v[..])));

impl_value_expr!(v: String => Value::String(v));
impl_value_expr!(v: Option<String> => Value::OptionString(v));

impl_value_expr!(v: DateTime<Utc> => Value::DateTimeUtc(v));
impl_value_expr!(v: &'q DateTime<Utc> => Value::DateTimeUtc(v.to_owned()));

pub struct QueryBuilder<'q> {
    kind: DBKind,
    query: String,
    binding_id: u32,
    arguments: Vec<Value<'q>>,
}

impl<'q> QueryBuilder<'q> {
    pub fn new(kind: DBKind) -> QueryBuilder<'q> {
        QueryBuilder {
            kind,
            query: String::default(),
            binding_id: 1,
            arguments: Vec::new(),
        }
    }

    pub fn from_raw_sql(kind: DBKind, sql: &str) -> QueryBuilder<'q> {
        let mut query = QueryBuilder::new(kind);
        query.sql(sql);
        query
    }

    pub fn kind(&self) -> DBKind {
        self.kind
    }

    /// Add a raw sql. See `RawSql` for details.
    pub(crate) fn sql(&mut self, sql: &str) -> &mut Self {
        let sql = sql.trim();
        if !sql.is_empty() {
            self.query += " ";
            self.query += sql;
        }
        self
    }

    /// Add a bound variable, See `Value` for details.
    pub(crate) fn value(&mut self, value: Value<'q>) -> &mut Self {
        self.query = format!("{} ${}", self.query, self.binding_id);
        self.binding_id += 1;
        self.arguments.push(value);
        self
    }

    #[allow(clippy::should_implement_trait)]
    pub fn add<E: SqlBuilderExpression<'q>>(&mut self, expr: E) -> &mut Self {
        expr.add_to_query(self)
    }

    pub fn into_raw(self) -> Result<String, DBBuilderError> {
        if self.arguments.is_empty() {
            log::trace!("sql: {}", self.query);
            Ok(self.query)
        } else {
            Err(DBBuilderError::QueryIsNotRaw)
        }
    }

    pub fn to_query<'a, DB>(&'a self) -> Query<'a, DB, <DB as HasArguments<'a>>::Arguments>
    where
        DB: Database,
        bool: Encode<'a, DB> + Type<DB>,
        Option<bool>: Encode<'a, DB> + Type<DB>,
        i32: Encode<'a, DB> + Type<DB>,
        Option<i32>: Encode<'a, DB> + Type<DB>,
        String: Encode<'a, DB> + Type<DB>,
        Option<String>: Encode<'a, DB> + Type<DB>,
        &'a str: Encode<'a, DB> + Type<DB>,
        Option<&'a str>: Encode<'a, DB> + Type<DB>,
        DateTime<Utc>: Encode<'a, DB> + Type<DB>,
        Option<DateTime<Utc>>: Encode<'a, DB> + Type<DB>,
        &'a [u8]: Encode<'a, DB> + Type<DB>,
        Option<&'a [u8]>: Encode<'a, DB> + Type<DB>,
    {
        log::trace!("sql:\n  {}\n  vars:\n  {:#?}", self.query, self.arguments);
        let mut query = sqlx::query::<DB>(&self.query);
        for val in &self.arguments {
            query = match val {
                Value::Bool(v) => query.bind(v),
                Value::OptionBool(v) => query.bind(v),
                Value::I32(v) => query.bind(v),
                Value::OptionI32(v) => query.bind(v),
                Value::String(v) => query.bind(v),
                Value::Str(v) => query.bind(v),
                Value::OptionString(v) => query.bind(v),
                Value::OptionStr(v) => query.bind(v),
                Value::DateTimeUtc(v) => query.bind(v),
                Value::OptionDateTimeUtc(v) => query.bind(v),
                Value::Binary(v) => query.bind(v),
                Value::OptionBinary(v) => query.bind(v),
            };
        }
        query
    }

    pub fn to_query_as<'a, DB, O>(&'a self) -> QueryAs<'a, DB, O, <DB as HasArguments<'a>>::Arguments>
    where
        DB: Database,
        for<'o> O: FromRow<'o, <DB as Database>::Row>,
        bool: Encode<'a, DB> + Type<DB>,
        Option<bool>: Encode<'a, DB> + Type<DB>,
        i32: Encode<'a, DB> + Type<DB>,
        Option<i32>: Encode<'a, DB> + Type<DB>,
        String: Encode<'a, DB> + Type<DB>,
        Option<String>: Encode<'a, DB> + Type<DB>,
        &'a str: Encode<'a, DB> + Type<DB>,
        Option<&'a str>: Encode<'a, DB> + Type<DB>,
        DateTime<Utc>: Encode<'a, DB> + Type<DB>,
        Option<DateTime<Utc>>: Encode<'a, DB> + Type<DB>,
        &'a [u8]: Encode<'a, DB> + Type<DB>,
        Option<&'a [u8]>: Encode<'a, DB> + Type<DB>,
    {
        log::trace!("sql: {}, vars: {:?}", self.query, self.arguments);
        let mut query = sqlx::query_as::<DB, O>(&self.query);
        for val in &self.arguments {
            query = match val {
                Value::Bool(v) => query.bind(v),
                Value::OptionBool(v) => query.bind(v),
                Value::I32(v) => query.bind(v),
                Value::OptionI32(v) => query.bind(v),
                Value::String(v) => query.bind(v),
                Value::Str(v) => query.bind(v),
                Value::OptionString(v) => query.bind(v),
                Value::OptionStr(v) => query.bind(v),
                Value::DateTimeUtc(v) => query.bind(v),
                Value::OptionDateTimeUtc(v) => query.bind(v),
                Value::Binary(v) => query.bind(v),
                Value::OptionBinary(v) => query.bind(v),
            };
        }
        query
    }
}
