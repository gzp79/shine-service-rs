use proc_macro::TokenStream;

mod sqlx_interpolate;

#[proc_macro]
pub fn sql(input: TokenStream) -> TokenStream {
    sqlx_interpolate::query(input)
}

#[proc_macro]
pub fn sql_expr(input: TokenStream) -> TokenStream {
    sqlx_interpolate::query_expr(input)
}
