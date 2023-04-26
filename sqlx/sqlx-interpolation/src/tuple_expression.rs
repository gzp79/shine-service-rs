use crate::{QueryBuilder, SqlBuilderExpression};

/// Implements sql binding for tuples (${(a,b,c)}) as a comma separated list ("($a,$b,$c)")
macro_rules! impl_value_tuple {
    ( IMPL => [$tuple:ty] [$($where_id:ty),*] [$($id:ident),*] ) => {
        impl<'q, $($id,)*> SqlBuilderExpression<'q> for $tuple
        where
            $( $where_id : SqlBuilderExpression<'q>,)*
        {
            #[allow(unused_assignments)]
            #[allow(non_snake_case)]
            fn add_to_query<'a>(self, query: &'a mut QueryBuilder<'q>) -> &'a mut QueryBuilder<'q> {
                let ($($id,)*) = self;
                query.sql("(");
                let mut first = true;
                $(
                    if !first {
                        query.sql(",");
                    }
                    query.add($id);
                    first = false;
                )*
                query.sql(")")
            }
        }
    };

    ( $($id:ident),* ) => {
        impl_value_tuple!( IMPL => [($($id,)*)] [$($id),*] [$($id),*] );
        impl_value_tuple!( IMPL => [&'q ($($id,)*)] [$(&'q $id),*] [$($id),*] );
    }
}

impl_value_tuple!(A0);
impl_value_tuple!(A0, A1);
impl_value_tuple!(A0, A1, A2);
impl_value_tuple!(A0, A1, A2, A3);
impl_value_tuple!(A0, A1, A2, A3, A4);
impl_value_tuple!(A0, A1, A2, A3, A4, A5);
impl_value_tuple!(A0, A1, A2, A3, A4, A5, A6);
impl_value_tuple!(A0, A1, A2, A3, A4, A5, A6, A7);
impl_value_tuple!(A0, A1, A2, A3, A4, A5, A6, A7, A8);
impl_value_tuple!(A0, A1, A2, A3, A4, A5, A6, A7, A8, A9);
impl_value_tuple!(A0, A1, A2, A3, A4, A5, A6, A7, A8, A9, A10);
