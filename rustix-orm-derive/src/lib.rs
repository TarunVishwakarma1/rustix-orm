extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, Attribute, Data, DeriveInput, Expr, Fields, Ident, Meta, MetaNameValue,
};

#[proc_macro_derive(Model, attributes(model))]
pub fn derive_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let table_name = extract_table_name(&input.attrs)
        .unwrap_or_else(|| format!("{}s", name.to_string().to_lowercase()));

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            syn::Fields::Named(fields) => &fields.named,
            _ => panic!("Only named fields are supported"),
        },
        _ => panic!("Model can only be derived for structs"),
    };

    let mut primary_key_field: Option<Ident> = None;
    let mut field_sql_defs = Vec::new();

    for field in fields {
        let field_ident = field.ident.clone().unwrap();
        let field_name = field_ident.to_string();
        let mut column_name = field_name.clone();
        let mut is_primary_key = false;
        let mut has_default = false;
        let mut default_value = String::new();
        let mut is_nullable = false;

        for attr in &field.attrs {
            if !attr.path().is_ident("model") {
                continue;
            }

            let parsed = attr.parse_args_with(
                syn::punctuated::Punctuated::<Meta, syn::token::Comma>::parse_terminated,
            );

            if let Ok(items) = parsed {
                for meta in items {
                    match meta {
                        Meta::Path(path) => {
                            if path.is_ident("primary_key") {
                                is_primary_key = true;
                                primary_key_field = Some(field_ident.clone());
                            } else if path.is_ident("nullable") {
                                is_nullable = true;
                            }
                        }
                        Meta::NameValue(MetaNameValue { path, value, .. }) => {
                            if path.is_ident("column") {
                                if let Expr::Lit(expr_lit) = value {
                                    if let syn::Lit::Str(lit_str) = expr_lit.lit {
                                        column_name = lit_str.value();
                                    }
                                }
                            } else if path.is_ident("default") {
                                if let Expr::Lit(expr_lit) = value {
                                    if let syn::Lit::Str(lit_str) = expr_lit.lit {
                                        has_default = true;
                                        default_value = lit_str.value();
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        let column_name_literal = column_name.clone();
        let default_literal = default_value.clone();

        let sql_def = quote! {
            {
                let mut part = format!("{} {}", #column_name_literal, match db_type {
                    crate::DatabaseType::PostgreSQL => "INTEGER",
                    crate::DatabaseType::MySQL => "INT",
                    crate::DatabaseType::SQLite => "INTEGER",
                });

                if #is_primary_key {
                    part.push_str(" PRIMARY KEY");
                }

                if !#is_nullable && !#is_primary_key {
                    part.push_str(" NOT NULL");
                }

                if #has_default {
                    part.push_str(&format!(" DEFAULT {}", #default_literal));
                }

                part
            }
        };

        field_sql_defs.push(sql_def);
    }

    let pk_ident = primary_key_field.unwrap_or_else(|| Ident::new("id", name.span()));

    let expanded = quote! {
        impl SQLModel for #name {
            fn table_name() -> String {
                #table_name.to_string()
            }

            fn primary_key_field() -> String {
                stringify!(#pk_ident).to_string()
            }

            fn primary_key_value(&self) -> Option<i32> {
                self.#pk_ident
            }

            fn set_primary_key(&mut self, id: i32) {
                self.#pk_ident = Some(id);
            }

            fn create_table_sql(db_type: &crate::DatabaseType) -> String {
                let mut sql = format!("CREATE TABLE IF NOT EXISTS {} (", Self::table_name());

                let fields = vec![
                    #(#field_sql_defs),*
                ];

                sql.push_str(&fields.join(", "));
                sql.push(')');
                sql
            }
        }
    };

    TokenStream::from(expanded)
}

fn extract_table_name(attrs: &[Attribute]) -> Option<String> {
    for attr in attrs {
        if !attr.path().is_ident("model") {
            continue;
        }

        let parsed = attr.parse_args_with(
            syn::punctuated::Punctuated::<Meta, syn::token::Comma>::parse_terminated,
        );

        if let Ok(items) = parsed {
            for meta in items {
                if let Meta::NameValue(MetaNameValue { path, value, .. }) = meta {
                    if path.is_ident("table") {
                        if let Expr::Lit(expr_lit) = value {
                            if let syn::Lit::Str(lit_str) = expr_lit.lit {
                                return Some(lit_str.value());
                            }
                        }
                    }
                }
            }
        }
    }
    None
}
