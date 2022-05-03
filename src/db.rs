//! A database abstraction around SQLite for Obnam.
//!
//! This abstraction provided the bare minimum that Obnam needs, while
//! trying to be as performant as possible, especially for inserting
//! rows. Only data types needed by Obnam are supported.
//!
//! Note that this abstraction is entirely synchronous. This is for
//! simplicity, as SQLite only allows one write at a time.

use crate::fsentry::FilesystemEntry;
use rusqlite::{params, types::ToSqlOutput, CachedStatement, Connection, OpenFlags, Row, ToSql};
use std::collections::HashSet;
use std::convert::TryFrom;
use std::path::{Path, PathBuf};

/// A database.
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Create a new database file for an empty database.
    ///
    /// The database can be written to.
    pub fn create<P: AsRef<Path>>(filename: P) -> Result<Self, DatabaseError> {
        if filename.as_ref().exists() {
            return Err(DatabaseError::Exists(filename.as_ref().to_path_buf()));
        }
        let flags = OpenFlags::SQLITE_OPEN_CREATE | OpenFlags::SQLITE_OPEN_READ_WRITE;
        let conn = Connection::open_with_flags(filename, flags)?;
        conn.execute("BEGIN", params![])?;
        Ok(Self { conn })
    }

    /// Open an existing database file in read only mode.
    pub fn open<P: AsRef<Path>>(filename: P) -> Result<Self, DatabaseError> {
        let flags = OpenFlags::SQLITE_OPEN_READ_ONLY;
        let conn = Connection::open_with_flags(filename, flags)?;
        Ok(Self { conn })
    }

    /// Close an open database, committing any changes to disk.
    pub fn close(self) -> Result<(), DatabaseError> {
        self.conn.execute("COMMIT", params![])?;
        self.conn
            .close()
            .map_err(|(_, err)| DatabaseError::Rusqlite(err))?;
        Ok(())
    }

    /// Create a table in the database.
    pub fn create_table(&self, table: &Table) -> Result<(), DatabaseError> {
        let sql = sql_statement::create_table(table);
        self.conn.execute(&sql, params![])?;
        Ok(())
    }

    /// Create an index in the database.
    pub fn create_index(
        &self,
        name: &str,
        table: &Table,
        column: &str,
    ) -> Result<(), DatabaseError> {
        let sql = sql_statement::create_index(name, table, column);
        self.conn.execute(&sql, params![])?;
        Ok(())
    }

    /// Insert a row in a table.
    pub fn insert(&mut self, table: &Table, values: &[Value]) -> Result<(), DatabaseError> {
        let mut stmt = self.conn.prepare_cached(table.insert())?;
        assert!(table.has_columns(values));
        // The ToSql trait implementation for Obnam values can't ever
        // fail, so we don't handle the error case in the parameter
        // creation below.
        stmt.execute(rusqlite::params_from_iter(values.iter().map(|v| {
            v.to_sql()
                .expect("conversion of Obnam value to SQLite value failed unexpectedly")
        })))?;
        Ok(())
    }

    /// Return an iterator for all rows in a table.
    pub fn all_rows<T>(
        &self,
        table: &Table,
        rowfunc: &'static dyn Fn(&Row) -> Result<T, rusqlite::Error>,
    ) -> Result<SqlResults<T>, DatabaseError> {
        let sql = sql_statement::select_all_rows(table);
        SqlResults::new(
            &self.conn,
            &sql,
            None,
            Box::new(|stmt, _| {
                let iter = stmt.query_map(params![], |row| rowfunc(row))?;
                let iter = iter.map(|x| match x {
                    Ok(t) => Ok(t),
                    Err(e) => Err(DatabaseError::Rusqlite(e)),
                });
                Ok(Box::new(iter))
            }),
        )
    }

    /// Return rows that have a given value in a given column.
    ///
    /// This is simplistic, but for Obnam, it provides all the SQL
    /// SELECT ... WHERE that's needed, and there's no point in making
    /// this more generic than is needed.
    pub fn some_rows<T>(
        &self,
        table: &Table,
        value: &Value,
        rowfunc: &'static dyn Fn(&Row) -> Result<T, rusqlite::Error>,
    ) -> Result<SqlResults<T>, DatabaseError> {
        assert!(table.has_column(value));
        let sql = sql_statement::select_some_rows(table, value.name());
        SqlResults::new(
            &self.conn,
            &sql,
            Some(OwnedValue::from(value)),
            Box::new(|stmt, value| {
                let iter = stmt.query_map(params![value], |row| rowfunc(row))?;
                let iter = iter.map(|x| match x {
                    Ok(t) => Ok(t),
                    Err(e) => Err(DatabaseError::Rusqlite(e)),
                });
                Ok(Box::new(iter))
            }),
        )
    }
}

/// Possible errors from a database.
#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    /// An error from the rusqlite crate.
    #[error(transparent)]
    Rusqlite(#[from] rusqlite::Error),

    /// The database being created already exists.
    #[error("Database {0} already exists")]
    Exists(PathBuf),
}

// A pointer to a "fallible iterator" over values of type `T`, which is to say it's an iterator
// over values of type `Result<T, DatabaseError>`. The iterator is only valid for the
// lifetime 'stmt.
//
// The fact that it's a pointer (`Box<dyn ...>`) means we don't care what the actual type of
// the iterator is, and who produces it.
type SqlResultsIterator<'stmt, T> = Box<dyn Iterator<Item = Result<T, DatabaseError>> + 'stmt>;

// A pointer to a function which, when called on a prepared SQLite statement, would create
// a "fallible iterator" over values of type `ItemT`. (See above for an explanation of what
// a "fallible iterator" is.)
//
// The iterator is only valid for the lifetime of the associated SQLite statement; we
// call this lifetime 'stmt, and use it both both on the reference and the returned iterator.
//
// Now we're in a pickle: all named lifetimes have to be declared _somewhere_, but we can't add
// 'stmt to the signature of `CreateIterFn` because then we'll have to specify it when we
// define the function. Obviously, at that point we won't yet have a `Statement`, and thus we
// would have no idea what its lifetime is going to be. So we can't put the 'stmt lifetime into
// the signature of `CreateIterFn`.
//
// That's what `for<'stmt>` is for. This is a so-called ["higher-rank trait bound"][hrtb], and
// it enables us to say that a function is valid for *some* lifetime 'stmt that we pass into it
// at the call site. It lets Rust continue to track lifetimes even though `CreateIterFn`
// interferes by "hiding" the 'stmt lifetime from its signature.
//
// [hrtb]: https://doc.rust-lang.org/nomicon/hrtb.html
type CreateIterFn<'conn, ItemT> = Box<
    dyn for<'stmt> Fn(
        &'stmt mut CachedStatement<'conn>,
        &Option<OwnedValue>,
    ) -> Result<SqlResultsIterator<'stmt, ItemT>, DatabaseError>,
>;

/// An iterator over rows from a query.
pub struct SqlResults<'conn, ItemT> {
    stmt: CachedStatement<'conn>,
    value: Option<OwnedValue>,
    create_iter: CreateIterFn<'conn, ItemT>,
}

impl<'conn, ItemT> SqlResults<'conn, ItemT> {
    fn new(
        conn: &'conn Connection,
        statement: &str,
        value: Option<OwnedValue>,
        create_iter: CreateIterFn<'conn, ItemT>,
    ) -> Result<Self, DatabaseError> {
        let stmt = conn.prepare_cached(statement)?;
        Ok(Self {
            stmt,
            value,
            create_iter,
        })
    }

    /// Create an iterator over results.
    pub fn iter(&'_ mut self) -> Result<SqlResultsIterator<'_, ItemT>, DatabaseError> {
        (self.create_iter)(&mut self.stmt, &self.value)
    }
}

/// Describe a table in a row.
pub struct Table {
    table: String,
    columns: Vec<Column>,
    insert: Option<String>,
    column_names: HashSet<String>,
}

impl Table {
    /// Create a new table description without columns.
    ///
    /// The table description is not "built". You must add columns and
    /// then call the [`build`] method.
    pub fn new(table: &str) -> Self {
        Self {
            table: table.to_string(),
            columns: vec![],
            insert: None,
            column_names: HashSet::new(),
        }
    }

    /// Append a column.
    pub fn column(mut self, column: Column) -> Self {
        self.column_names.insert(column.name().to_string());
        self.columns.push(column);
        self
    }

    /// Finish building the table description.
    pub fn build(mut self) -> Self {
        assert!(self.insert.is_none());
        self.insert = Some(sql_statement::insert(&self));
        self
    }

    fn has_columns(&self, values: &[Value]) -> bool {
        assert!(self.insert.is_some());
        for v in values.iter() {
            if !self.column_names.contains(v.name()) {
                return false;
            }
        }
        true
    }

    fn has_column(&self, value: &Value) -> bool {
        assert!(self.insert.is_some());
        self.column_names.contains(value.name())
    }

    fn insert(&self) -> &str {
        assert!(self.insert.is_some());
        self.insert.as_ref().unwrap()
    }

    /// What is the name of the table?
    pub fn name(&self) -> &str {
        &self.table
    }

    /// How many columns does the table have?
    pub fn num_columns(&self) -> usize {
        self.columns.len()
    }

    /// What are the names of the columns in the table?
    pub fn column_names(&self) -> impl Iterator<Item = &str> {
        self.columns.iter().map(|c| c.name())
    }

    /// Return SQL column definitions for the table.
    pub fn column_definitions(&self) -> String {
        let mut ret = String::new();
        for c in self.columns.iter() {
            if !ret.is_empty() {
                ret.push(',');
            }
            ret.push_str(c.name());
            ret.push(' ');
            ret.push_str(c.typename());
        }
        ret
    }
}

/// A column in a table description.
pub enum Column {
    /// An integer primary key.
    PrimaryKey(String),
    /// An integer.
    Int(String),
    /// A text string.
    Text(String),
    /// A binary string.
    Blob(String),
    /// A boolean.
    Bool(String),
}

impl Column {
    fn name(&self) -> &str {
        match self {
            Self::PrimaryKey(name) => name,
            Self::Int(name) => name,
            Self::Text(name) => name,
            Self::Blob(name) => name,
            Self::Bool(name) => name,
        }
    }

    fn typename(&self) -> &str {
        match self {
            Self::PrimaryKey(_) => "INTEGER PRIMARY KEY",
            Self::Int(_) => "INTEGER",
            Self::Text(_) => "TEXT",
            Self::Blob(_) => "BLOB",
            Self::Bool(_) => "BOOLEAN",
        }
    }

    /// Create an integer primary key column.
    pub fn primary_key(name: &str) -> Self {
        Self::PrimaryKey(name.to_string())
    }

    /// Create an integer column.
    pub fn int(name: &str) -> Self {
        Self::Int(name.to_string())
    }

    /// Create a text string column.
    pub fn text(name: &str) -> Self {
        Self::Text(name.to_string())
    }

    /// Create a binary string column.
    pub fn blob(name: &str) -> Self {
        Self::Blob(name.to_string())
    }

    /// Create a boolean column.
    pub fn bool(name: &str) -> Self {
        Self::Bool(name.to_string())
    }
}

/// Type of plain integers that can be stored.
pub type DbInt = u64;

/// A value in a named column.
#[derive(Debug)]
pub enum Value<'a> {
    /// An integer primary key.
    PrimaryKey(&'a str, DbInt),
    /// An integer.
    Int(&'a str, DbInt),
    /// A text string.
    Text(&'a str, &'a str),
    /// A binary string.
    Blob(&'a str, &'a [u8]),
    /// A boolean.
    Bool(&'a str, bool),
}

impl<'a> Value<'a> {
    /// What column should store this value?
    pub fn name(&self) -> &str {
        match self {
            Self::PrimaryKey(name, _) => name,
            Self::Int(name, _) => name,
            Self::Text(name, _) => name,
            Self::Blob(name, _) => name,
            Self::Bool(name, _) => name,
        }
    }

    /// Create an integer primary key value.
    pub fn primary_key(name: &'a str, value: DbInt) -> Self {
        Self::PrimaryKey(name, value)
    }

    /// Create an integer value.
    pub fn int(name: &'a str, value: DbInt) -> Self {
        Self::Int(name, value)
    }

    /// Create a text string value.
    pub fn text(name: &'a str, value: &'a str) -> Self {
        Self::Text(name, value)
    }

    /// Create a binary string value.
    pub fn blob(name: &'a str, value: &'a [u8]) -> Self {
        Self::Blob(name, value)
    }

    /// Create a boolean value.
    pub fn bool(name: &'a str, value: bool) -> Self {
        Self::Bool(name, value)
    }
}

#[allow(clippy::useless_conversion)]
impl<'a> ToSql for Value<'a> {
    // The trait defines to_sql to return a Result. However, for our
    // particular case, to_sql can't ever fail. We only store values
    // in types for which conversion always succeeds: integer,
    // boolean, text, and blob. _For us_, the caller need never worry
    // that the conversion fails, but we can't express that in the
    // type system.
    fn to_sql(&self) -> Result<rusqlite::types::ToSqlOutput, rusqlite::Error> {
        use rusqlite::types::ValueRef;
        let v = match self {
            Self::PrimaryKey(_, v) => ValueRef::Integer(
                i64::try_from(*v)
                    .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?,
            ),
            Self::Int(_, v) => ValueRef::Integer(
                i64::try_from(*v)
                    .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?,
            ),
            Self::Bool(_, v) => ValueRef::Integer(
                i64::try_from(*v)
                    .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?,
            ),
            Self::Text(_, v) => ValueRef::Text(v.as_ref()),
            Self::Blob(_, v) => ValueRef::Blob(v),
        };
        Ok(ToSqlOutput::Borrowed(v))
    }
}

/// Like a Value, but owns the data.
pub enum OwnedValue {
    /// An integer primary key.
    PrimaryKey(String, DbInt),
    /// An integer.
    Int(String, DbInt),
    /// A text string.
    Text(String, String),
    /// A binary string.
    Blob(String, Vec<u8>),
    /// A boolean.
    Bool(String, bool),
}

impl From<&Value<'_>> for OwnedValue {
    fn from(v: &Value) -> Self {
        match *v {
            Value::PrimaryKey(name, v) => Self::PrimaryKey(name.to_string(), v),
            Value::Int(name, v) => Self::Int(name.to_string(), v),
            Value::Text(name, v) => Self::Text(name.to_string(), v.to_string()),
            Value::Blob(name, v) => Self::Blob(name.to_string(), v.to_vec()),
            Value::Bool(name, v) => Self::Bool(name.to_string(), v),
        }
    }
}

impl ToSql for OwnedValue {
    #[allow(clippy::useless_conversion)]
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput> {
        use rusqlite::types::Value;
        let v = match self {
            Self::PrimaryKey(_, v) => Value::Integer(
                i64::try_from(*v)
                    .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?,
            ),
            Self::Int(_, v) => Value::Integer(
                i64::try_from(*v)
                    .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?,
            ),
            Self::Bool(_, v) => Value::Integer(
                i64::try_from(*v)
                    .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?,
            ),
            Self::Text(_, v) => Value::Text(v.to_string()),
            Self::Blob(_, v) => Value::Blob(v.to_vec()),
        };
        Ok(ToSqlOutput::Owned(v))
    }
}

impl rusqlite::types::ToSql for FilesystemEntry {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        let json = serde_json::to_string(self)
            .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?;
        let json = rusqlite::types::Value::Text(json);
        Ok(ToSqlOutput::Owned(json))
    }
}

mod sql_statement {
    use super::Table;

    pub fn create_table(table: &Table) -> String {
        format!(
            "CREATE TABLE {} ({})",
            table.name(),
            table.column_definitions()
        )
    }

    pub fn create_index(name: &str, table: &Table, column: &str) -> String {
        format!("CREATE INDEX {} ON {} ({})", name, table.name(), column,)
    }

    pub fn insert(table: &Table) -> String {
        format!(
            "INSERT INTO {} ({}) VALUES ({})",
            table.name(),
            &column_names(table),
            placeholders(table.column_names().count())
        )
    }

    pub fn select_all_rows(table: &Table) -> String {
        format!("SELECT * FROM {}", table.name())
    }

    pub fn select_some_rows(table: &Table, column: &str) -> String {
        format!("SELECT * FROM {} WHERE {} = ?", table.name(), column)
    }

    fn column_names(table: &Table) -> String {
        table.column_names().collect::<Vec<&str>>().join(",")
    }

    fn placeholders(num_columns: usize) -> String {
        let mut s = String::new();
        for _ in 0..num_columns {
            if !s.is_empty() {
                s.push(',');
            }
            s.push('?');
        }
        s
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::path::Path;
    use tempfile::tempdir;

    fn get_bar(row: &rusqlite::Row) -> Result<DbInt, rusqlite::Error> {
        row.get("bar")
    }

    fn table() -> Table {
        Table::new("foo").column(Column::int("bar")).build()
    }

    fn create_db(file: &Path) -> Database {
        let table = table();
        let db = Database::create(file).unwrap();
        db.create_table(&table).unwrap();
        db
    }

    fn open_db(file: &Path) -> Database {
        Database::open(file).unwrap()
    }

    fn insert(db: &mut Database, value: DbInt) {
        let table = table();
        db.insert(&table, &[Value::int("bar", value)]).unwrap();
    }

    fn values(db: Database) -> Vec<DbInt> {
        let table = table();
        let mut rows = db.all_rows(&table, &get_bar).unwrap();
        let iter = rows.iter().unwrap();
        let mut values = vec![];
        for x in iter {
            values.push(x.unwrap());
        }
        values
    }

    #[test]
    fn creates_db() {
        let tmp = tempdir().unwrap();
        let filename = tmp.path().join("test.db");
        let db = Database::create(&filename).unwrap();
        db.close().unwrap();
        let _ = Database::open(&filename).unwrap();
    }

    #[test]
    fn inserts_row() {
        let tmp = tempdir().unwrap();
        let filename = tmp.path().join("test.db");
        let mut db = create_db(&filename);
        insert(&mut db, 42);
        db.close().unwrap();

        let db = open_db(&filename);
        let values = values(db);
        assert_eq!(values, vec![42]);
    }

    #[test]
    fn inserts_many_rows() {
        const N: DbInt = 1000;

        let tmp = tempdir().unwrap();
        let filename = tmp.path().join("test.db");
        let mut db = create_db(&filename);
        for i in 0..N {
            insert(&mut db, i);
        }
        db.close().unwrap();

        let db = open_db(&filename);
        let values = values(db);
        assert_eq!(values.len() as DbInt, N);

        let mut expected = vec![];
        for i in 0..N {
            expected.push(i);
        }
        assert_eq!(values, expected);
    }
}
