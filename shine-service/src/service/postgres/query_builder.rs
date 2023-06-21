use tokio_postgres::types::ToSql;

pub trait AndWhere<const N: usize> {
    fn into_statement(self, builder: &mut QueryBuilder<'_>);
}

impl<F> AndWhere<0> for F
where
    F: FnOnce() -> String,
{
    fn into_statement(self, builder: &mut QueryBuilder<'_>) {
        let and_condition = (self)();
        if let Some(condition) = &mut builder.condition {
            condition.push_str(" AND ");
            condition.push_str(&and_condition);
        } else {
            builder.condition = Some(and_condition);
        }
    }
}

impl<F> AndWhere<1> for F
where
    F: FnOnce(usize) -> String,
{
    fn into_statement(self, builder: &mut QueryBuilder<'_>) {
        let and_condition = (self)(builder.bind_id);
        if let Some(condition) = &mut builder.condition {
            condition.push_str(" AND ");
            condition.push_str(&and_condition);
        } else {
            builder.condition = Some(and_condition);
        }
        builder.bind_id += 1;
    }
}

impl<F> AndWhere<2> for F
where
    F: FnOnce(usize, usize) -> String,
{
    fn into_statement(self, builder: &mut QueryBuilder<'_>) {
        let and_condition = (self)(builder.bind_id, builder.bind_id + 1);
        if let Some(condition) = &mut builder.condition {
            condition.push_str(" AND ");
            condition.push_str(&and_condition);
        } else {
            builder.condition = Some(and_condition);
        }
        builder.bind_id += 2;
    }
}

impl<F> AndWhere<3> for F
where
    F: FnOnce(usize, usize, usize) -> String,
{
    fn into_statement(self, builder: &mut QueryBuilder<'_>) {
        let and_condition = (self)(builder.bind_id, builder.bind_id + 1, builder.bind_id + 2);
        if let Some(condition) = &mut builder.condition {
            condition.push_str(" AND ");
            condition.push_str(&and_condition);
        } else {
            builder.condition = Some(and_condition);
        }
        builder.bind_id += 3;
    }
}

pub struct QueryBuilder<'a> {
    params: Vec<&'a (dyn ToSql + Sync)>,
    bind_id: usize,
    select: String,
    condition: Option<String>,
    order_by: Option<String>,
    limit: Option<usize>,
}

impl<'a> QueryBuilder<'a> {
    pub fn new<S: ToString>(select: S) -> Self {
        Self {
            params: Vec::new(),
            bind_id: 1,
            select: select.to_string(),
            condition: None,
            order_by: None,
            limit: None,
        }
    }

    pub fn and_where<F, const N: usize>(&mut self, condition: F, p: [&'a (dyn ToSql + Sync); N])
    where
        F: AndWhere<N>,
    {
        condition.into_statement(self);
        self.params.extend_from_slice(&p);
    }

    pub fn order_by(&mut self, order: &str) {
        if let Some(order_by) = &mut self.order_by {
            order_by.push_str(", ");
            order_by.push_str(order);
        } else {
            self.order_by = Some(order.into());
        }
    }

    pub fn limit(&mut self, limit: usize) {
        assert!(self.limit.is_none());
        self.limit = Some(limit);
    }

    pub fn build(self) -> (String, Vec<&'a (dyn ToSql + Sync)>) {
        let mut stmt = self.select;
        if let Some(condition) = self.condition {
            stmt.push_str(" WHERE ");
            stmt.push_str(&condition);
        }
        if let Some(order_by) = self.order_by {
            stmt.push_str(" ORDER BY ");
            stmt.push_str(&order_by);
        }
        if let Some(limit) = self.limit {
            stmt.push_str(" LIMIT ");
            stmt.push_str(&limit.to_string());
        }

        (stmt, self.params)
    }
}
