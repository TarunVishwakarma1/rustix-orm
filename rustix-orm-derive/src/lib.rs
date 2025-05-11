use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, Attribute, Data, DeriveInput, Expr, Ident, Meta, MetaNameValue,
    Type, TypePath,
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
    let mut field_names = Vec::new();
    let mut field_to_sql_values = Vec::new();
    let mut field_from_row = Vec::new();
    let mut field_idents = Vec::new();
    let mut field_str_names = Vec::new();

    for field in fields {
        let field_ident = field.ident.clone().unwrap();
        let field_ident_str = field_ident.to_string();
        let field_name = field_ident.to_string();
        let mut column_name = field_name.clone();
        let mut is_primary_key = false;
        let mut has_default = false;
        let mut default_value = String::new();
        let mut is_nullable = false;
        let mut custom_type = None;

        field_idents.push(field_ident.clone());
        field_str_names.push(field_ident_str);

        // Process field attributes
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
                            } else if path.is_ident("sql_type") {
                                if let Expr::Lit(expr_lit) = value {
                                    if let syn::Lit::Str(lit_str) = expr_lit.lit {
                                        custom_type = Some(lit_str.value());
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        field_names.push(column_name.clone());

        // Generate field values extraction for ToSql
        let field_to_sql_value = quote! {
            Box::new(self.#field_ident.clone()) as Box<dyn rustix_orm::ToSqlConvert>
        };
        field_to_sql_values.push(field_to_sql_value);

        // Generate from_row conversion for this field
        let is_option = is_nullable || is_option_type(&field.ty);
        let field_from_json = generate_from_json(&field_ident, &column_name, &field.ty, is_option);
        field_from_row.push(field_from_json);

        let column_name_literal = column_name.clone();
        let default_literal = default_value.clone();

        // Generate SQL type from Rust type
        let sql_type = if let Some(custom) = custom_type {
            quote! { SqlType::Custom(#custom.to_string()) }
        } else {
            let rust_type = &field.ty;
            generate_sql_type(rust_type)
        };

        let sql_def = quote! {
            {
                let mut part = format!("{} {}", #column_name_literal, match db_type {
                    rustix_orm::DatabaseType::PostgreSQL => #sql_type.pg_type().to_string(), // Changed
                    rustix_orm::DatabaseType::MySQL => #sql_type.mysql_type().to_string(),     // Changed
                    rustix_orm::DatabaseType::SQLite => #sql_type.sqlite_type().to_string(),    // Changed
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

    // Convert field_names to static string literals
    let field_name_literals: Vec<_> = field_names.iter().map(|name| {
        let name_str = name.as_str();
        quote! { #name_str }
    }).collect();

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

            fn create_table_sql(db_type: &rustix_orm::DatabaseType) -> String { // Changed
                let mut sql = format!("CREATE TABLE IF NOT EXISTS \"{}\" (", Self::table_name());

                let fields = vec![
                    #(#field_sql_defs),*
                ];

                sql.push_str(&fields.join(", "));
                sql.push(')');
                sql
            }

            fn field_names() -> Vec<&'static str> {
                vec![#(#field_name_literals),*]
            }

            fn to_sql_field_values(&self) -> Vec<Box<dyn rustix_orm::ToSqlConvert>> {
                vec![
                    #(#field_to_sql_values),*
                ]
            }

            fn from_row(row: &serde_json::Value) -> Result<Self, rustix_orm::RustixError> { // Changed
                if !row.is_object() {
                    return Err(rustix_orm::RustixError::DeserializationError( // Changed
                        "Row is not a JSON object".to_string()
                    ));
                }

                let obj = row.as_object().unwrap();

                Ok(Self {
                    #(#field_from_row),*
                })
            }
        }
    };

    TokenStream::from(expanded)
}

// Helper function to determine if a type is an Option<T>
fn is_option_type(ty: &Type) -> bool {
    if let Type::Path(TypePath { path, .. }) = ty {
        if let Some(segment) = path.segments.last() {
            return segment.ident == "Option";
        }
    }
    false
}

// Generate code to extract a field value from a JSON object
fn generate_from_json(field_ident: &Ident, column_name: &str, _field_type: &Type, is_optional: bool) -> proc_macro2::TokenStream {
    let column_literal = column_name;

    if is_optional {
        quote! {
            #field_ident: if let Some(val) = obj.get(#column_literal) {
                if val.is_null() {
                    None
                } else {
                    match serde_json::from_value(val.clone()) {
                        Ok(v) => Some(v),
                        Err(e) => return Err(rustix_orm::RustixError::DeserializationError(
                            format!("Failed to deserialize field {}: {}", #column_literal, e)
                        )),
                    }
                }
            } else {
                None
            }
        }
    } else {
        quote! {
            #field_ident: if let Some(val) = obj.get(#column_literal) {
                match serde_json::from_value(val.clone()) {
                    Ok(v) => v,
                    Err(e) => return Err(rustix_orm::RustixError::DeserializationError(
                        format!("Failed to deserialize field {}: {}", #column_literal, e)
                    )),
                }
            } else {
                return Err(rustix_orm::RustixError::DeserializationError(
                    format!("Missing required field: {}", #column_literal)
                ));
            }
        }
    }
}

// Maps Rust types to SQL types
fn generate_sql_type(rust_type: &Type) -> proc_macro2::TokenStream {
    match rust_type {
        Type::Path(TypePath { path, .. }) => {
            let segment = path.segments.last().unwrap();
            let ident = &segment.ident;
            let type_name = ident.to_string();

            if type_name == "Option" {
                // Handle Option<T>
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(arg) = args.args.first() {
                        if let syn::GenericArgument::Type(inner_type) = arg {
                            return generate_sql_type(inner_type);
                        }
                    }
                }
                quote! { SqlType::Text }
            } else {
                match type_name.as_str() {
                    "i8" | "i16" | "i32" | "u8" | "u16" | "u32" => quote! { SqlType::Integer },
                    "i64" | "u64" => quote! { SqlType::BigInt },
                    "f32" | "f64" => quote! { SqlType::Float },
                    "bool" => quote! { SqlType::Boolean },
                    "String" | "str" => quote! { SqlType::Text },
                    "Vec" => {
                        // Check if it's Vec<u8> for binary data
                        if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                            if let Some(arg) = args.args.first() {
                                if let syn::GenericArgument::Type(Type::Path(TypePath { path, .. })) = arg {
                                    if let Some(seg) = path.segments.last() {
                                        if seg.ident == "u8" {
                                            return quote! { SqlType::Blob };
                                        }
                                    }
                                }
                            }
                        }
                        quote! { SqlType::Blob }
                    },
                    // Add more type mappings as needed
                    "NaiveDate" => quote! { SqlType::Date },
                    "NaiveTime" => quote! { SqlType::Time },
                    "NaiveDateTime" | "DateTime" => quote! { SqlType::DateTime },
                    "Uuid" => quote! { SqlType::Text },
                    _ => quote! { SqlType::Text }, // Default to TEXT for unknown types
                }
            }
        }
        _ => quote! { SqlType::Text }, // Default for complex types
    }
}

// Extract table name from attributes
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