use proc_macro::TokenStream;
use proc_macro2::{Group, Ident, Span, TokenTree};
use quote::quote;
use std::iter::FromIterator;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, parse_quote, parse_str,
    punctuated::Punctuated,
    Expr, LitStr, Token,
};

/// Type of interpolation resolution to apply on the expression
enum Interpolation {
    /// Bind interpolation expression as a value
    Bind,
    /// Substitute the interpolation expression as a raw sql string - use with caution due to injection vulnerability
    NoBind,
}

fn rewrite_site(e: proc_macro2::TokenStream, span: Span) -> proc_macro2::TokenStream {
    proc_macro2::TokenStream::from_iter(e.into_iter().map(|tt| match tt {
        TokenTree::Ident(ident) => TokenTree::Ident(Ident::new(&ident.to_string(), span)),
        TokenTree::Group(group) => TokenTree::Group(Group::new(group.delimiter(), rewrite_site(group.stream(), span))),
        tt => tt,
    }))
}

fn string_interpolate(input: &str, call_site: Span) -> proc_macro2::TokenStream {
    let mut build_expr = Vec::<Expr>::new();

    let id_raw_sql = quote! { sqlx_interpolation::expr::RawSql };

    let s: Vec<char> = input.chars().collect();
    let mut s = &s[0..];
    let mut raw_sql = String::new();
    while !s.is_empty() {
        if s[0] != '$' {
            raw_sql.push(s[0]);
            s = &s[1..];
            continue;
        }

        // find the type of interpolation to apply
        let expr_ty = if s.starts_with(&['$', '$']) {
            // consume escaped $ ('$$')
            raw_sql.push('$');
            s = &s[2..];
            continue;
        } else if s.starts_with(&['$', '{']) {
            //interpolation with value binding
            s = &s[1..];
            Interpolation::Bind
        } else if s.starts_with(&['$', '!', '{']) {
            s = &s[2..];
            Interpolation::NoBind
        } else {
            panic!("Missing interpolation block, if you intended to add `$`, you can escape it using `$$`")
        };

        // add raw sql snippet
        if !raw_sql.is_empty() {
            build_expr.push(parse_quote! { add(#id_raw_sql(#raw_sql.to_string())) });
            raw_sql = String::new();
        }

        // find  interpolation expression: ${...}
        let mut expr = String::new();
        let mut level = 0;
        while !s.is_empty() {
            let c = s[0];
            s = &s[1..];

            if c == '}' {
                level -= 1;
                if level == 0 {
                    expr.push(c);
                    break;
                }
            } else if c == '{' {
                level += 1;
            }

            expr.push(c);
        }
        if level != 0 {
            panic!("Unclosed interpolation block: {expr}");
        }

        // add interpolation as a bound value
        let expr: Expr = parse_str(&expr).unwrap_or_else(|err| panic!("Failed to parse: `{}`: {:?}", &expr, err));
        let expr = rewrite_site(quote! { #expr }, call_site);
        match expr_ty {
            Interpolation::Bind => build_expr.push(parse_quote! { add(#expr) }),
            Interpolation::NoBind => build_expr.push(parse_quote! { add(#id_raw_sql(#expr.to_string())) }),
        };
    }

    // add final raw sql snippet
    if !raw_sql.is_empty() {
        build_expr.push(parse_quote! { add(#id_raw_sql(#raw_sql.to_string())) });
    }

    quote! {
        #(.#build_expr)*
    }
}

struct MultilineStringList {
    string: String,
    query_builder: Expr,
    call_site: Span,
}

impl Parse for MultilineStringList {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let query_builder: Expr = input.parse()?;
        let _: Token![,] = input.parse()?;
        let list = Punctuated::<LitStr, Token![+]>::parse_terminated(input)?;
        let call_site = input.span();
        let string = list.iter().map(|l| l.value()).collect::<Vec<_>>().join(" ");
        Ok(MultilineStringList {
            string,
            query_builder,
            call_site,
        })
    }
}

pub fn query(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as MultilineStringList);

    let query_builder = input.query_builder;
    let interpolate = string_interpolate(&input.string, input.call_site);

    quote! {
        #query_builder #interpolate
    }
    .into()
}

pub fn query_expr(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as MultilineStringList);

    let query_builder_builder = input.query_builder;
    let interpolate = string_interpolate(&input.string, input.call_site);

    quote! {
        {
            let mut query = sqlx_interpolation::QueryBuilder::new(#query_builder_builder);
            query #interpolate;
            query
        }
    }
    .into()
}
