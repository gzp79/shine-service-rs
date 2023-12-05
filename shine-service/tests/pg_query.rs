use postgres_from_row::FromRow;
use shine_service::{pg_query, service::create_postgres_pool};
use shine_test::test;
use std::env;

#[derive(FromRow)]
struct SelectRow {
    one: i32,
    two: i32,
    data: String,
    text: String,
}

pg_query!( TestQuery1 =>
    in = data: &str;
    out = SelectRow;
    sql = r#"
        SELECT 1 as one, 2 as two, 'str' as text, $1 as data
    "#
);

pg_query!( TestQuery2 =>
    in = data: &str;
    out = one: i32;
    sql = r#"
        SELECT 1 as one, 2 as two, 'str' as text, $1 as data
    "#
);

pg_query!( TestQuery2Fail =>
    in = data: &str;
    out = oneFail: i32;
    sql = r#"
        SELECT 1 as one, 2 as two, 'str' as text, $1 as data
    "#
);

pg_query!( TestQuery3 =>
    in = data: &str;
    sql = r#"
        SELECT $1 as data
    "#
);

#[test]
async fn test_pg_query_struct() {
    match env::var("SHINE_TEST_PG_CNS") {
        Ok(cns) => {
            let pool = create_postgres_pool(&cns).await.unwrap();
            let c1 = pool.get().await.unwrap();
            let stmt1 = TestQuery1::new(&c1).await.unwrap();
            let stmt2 = TestQuery2::new(&c1).await.unwrap();
            let stmt2b = TestQuery2Fail::new(&c1).await.unwrap();
            let stmt3 = TestQuery3::new(&c1).await.unwrap();


            let p1 = stmt1.query_one(&c1, &"data").await.unwrap();
            assert_eq!(p1.one, 1);
            assert_eq!(p1.two, 2);
            assert_eq!(p1.data, "data");
            assert_eq!(p1.text, "str");

            let p2 = stmt2.query_one(&c1, &"data").await.unwrap();
            assert_eq!(p2, 1);

            stmt3.execute(&c1, &"data").await.unwrap();

            let p2b = stmt2b.query_one(&c1, &"data").await;
            assert_eq!(p2b.unwrap_err().to_string(), "invalid column `oneFail`");
        }

        _ => log::warn!("Skipping test_stored_statements"),
    }
}
