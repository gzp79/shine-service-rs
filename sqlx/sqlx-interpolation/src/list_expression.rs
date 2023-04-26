use crate::{QueryBuilder, SqlBuilderExpression};

/// Iterates on a slice and create an sql expression from it. The items
/// will be separated by comma in the generated sql.
impl<'q, T> SqlBuilderExpression<'q> for &'q [T]
where
    &'q T: SqlBuilderExpression<'q>,
{
    fn add_to_query<'a>(self, query: &'a mut QueryBuilder<'q>) -> &'a mut QueryBuilder<'q> {
        for (id, expr) in self.iter().enumerate() {
            if id > 0 {
                query.sql(",");
            }
            query.add(expr);
        }
        query
    }
}

/// Iterates on a slice and create an sql expression from it. The items
/// will be separated by comma in the generated sql.
impl<'q, const N: usize, T> SqlBuilderExpression<'q> for &'q [T; N]
where
    &'q T: SqlBuilderExpression<'q>,
{
    fn add_to_query<'a>(self, query: &'a mut QueryBuilder<'q>) -> &'a mut QueryBuilder<'q> {
        for (id, expr) in self.iter().enumerate() {
            if id > 0 {
                query.sql(",");
            }
            query.add(expr);
        }
        query
    }
}

/// Consume a vector and create an sql expression from it. The items
/// will be separated by comma in the generated sql.
impl<'q, T> SqlBuilderExpression<'q> for Vec<T>
where
    T: SqlBuilderExpression<'q>,
{
    fn add_to_query<'a>(self, query: &'a mut QueryBuilder<'q>) -> &'a mut QueryBuilder<'q> {
        for (id, expr) in self.into_iter().enumerate() {
            if id > 0 {
                query.sql(",");
            }
            query.add(expr);
        }
        query
    }
}
