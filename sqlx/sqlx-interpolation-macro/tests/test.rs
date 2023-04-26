use sqlx_interpolation_macro::sql;

mod sqlx_interpolation {
    pub trait Expr {
        fn raw(self) -> String;
    }

    impl<T: ToString> Expr for T {
        fn raw(self) -> String {
            format!("E[{}]", self.to_string())
        }
    }

    /*impl<'a, T: ToString> Expr for &'a T {
        fn raw(self) -> String {
            format!("E[{}]", self.to_string())
        }
    }*/

    pub mod expr {
        use super::Expr;

        pub struct RawSql(pub String);

        impl Expr for RawSql {
            fn raw(self) -> String {
                self.0
            }
        }
    }

    /// Mock QueryBuilder for the generated code
    #[derive(Default)]
    pub struct QueryBuilder(String);

    impl QueryBuilder {
        pub fn new() -> QueryBuilder {
            QueryBuilder(String::new())
        }

        #[must_use]
        #[allow(clippy::should_implement_trait)]
        pub fn add<T: Expr>(mut self, expr: T) -> QueryBuilder {
            self.0.push_str(&expr.raw());
            self
        }

        pub fn to_query(&self) -> &str {
            &self.0
        }
    }
}

#[test]
fn test() {
    use self::sqlx_interpolation::QueryBuilder;

    assert_eq!(sql!(QueryBuilder::new(), "raw string").to_query(), "raw string");
    assert_eq!(sql!(QueryBuilder::new(), "$$").to_query(), "$");
    assert_eq!(sql!(QueryBuilder::new(), "${123}").to_query(), "E[123]");
    assert_eq!(sql!(QueryBuilder::new(), "$!{123}").to_query(), "123");
    assert_eq!(
        sql!(QueryBuilder::new(), "prefix ${123} postfix").to_query(),
        "prefix E[123] postfix"
    );
    assert_eq!(
        sql!(QueryBuilder::new(), "prefix $!{123} postfix").to_query(),
        "prefix 123 postfix"
    );
    assert_eq!(
        sql!(QueryBuilder::new(), "prefix ${&123} postfix").to_query(),
        "prefix E[123] postfix"
    );
    assert_eq!(
        sql!(QueryBuilder::new(), "prefix ${{{}{{{}}{}123}}} p").to_query(),
        "prefix E[123] p"
    );

    assert_eq!(
        sql!(
            QueryBuilder::new(),
            "pre"
                + "fix ${{{}{"
                + "{{}}{}123}}"
                + "} p"
                + "make it a multiline text1 "
                + "make it a multiline text2 "
                + "make it a multiline text3 "
                + "make it a multiline text4"
        )
        .to_query(),
        "prefix E[123] pmake it a multiline text1 make it a multiline text2 make it a multiline text3 make it a multiline text4"
    );

    let i = 123;
    assert_eq!(sql!(QueryBuilder::new(), "${i}").to_query(), "E[123]");
    assert_eq!(sql!(QueryBuilder::new(), "${&i}").to_query(), "E[123]");

    assert_eq!(
        sql!(QueryBuilder::new(), "PI = ${std::f64::consts::PI}").to_query(),
        format!("PI = E[{:?}]", std::f64::consts::PI)
    );

    let world = "世界";
    assert_eq!(
        sql!(QueryBuilder::new(), "ハロー ${world}").to_query(),
        format!("ハロー E[{world}]")
    );

    assert_eq!(
        sql!(QueryBuilder::new(), "PI = ${ 1.0_f64.atan() * 4.0 }").to_query(),
        format!("PI = E[{:?}]", 1.0_f64.atan() * 4.0)
    );

    assert_eq!(
        sql!(QueryBuilder::new(), "t*t = ${ let t = 123; t * t }").to_query(),
        "t*t = E[15129]"
    );
    assert_eq!(
        sql!(QueryBuilder::new(), "t*t = ${ { let t = 123; t * t } }").to_query(),
        "t*t = E[15129]"
    );
    assert_eq!(
        sql!(QueryBuilder::new(), "t*t = $!{ let t = 123; t * t }").to_query(),
        "t*t = 15129"
    );
    assert_eq!(
        sql!(QueryBuilder::new(), "t*t = $!{ { let t = 123; t * t } }").to_query(),
        "t*t = 15129"
    );
}
