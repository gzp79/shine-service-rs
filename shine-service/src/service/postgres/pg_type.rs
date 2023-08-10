use tokio_postgres::types::Type;
use uuid::Uuid;

pub trait ToPGType {
    const PG_TYPE: Type;
}

impl<T> ToPGType for Option<T>
where
    T: ToPGType,
{
    const PG_TYPE: Type = <T as ToPGType>::PG_TYPE;
}

impl ToPGType for i16 {
    const PG_TYPE: Type = Type::INT2;
}

impl ToPGType for i32 {
    const PG_TYPE: Type = Type::INT4;
}

impl ToPGType for Uuid {
    const PG_TYPE: Type = Type::UUID;
}

impl ToPGType for &str {
    const PG_TYPE: Type = Type::VARCHAR;
}
