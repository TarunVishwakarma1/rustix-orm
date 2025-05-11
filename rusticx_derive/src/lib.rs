use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, Attribute, Data, DeriveInput, Expr, Ident, Meta, MetaNameValue, Type,
    TypePath,
};

/// Derives the `SQLModel` trait for a struct, allowing it to be used as a database model.
///
/// This macro processes the struct's fields and their attributes to generate the necessary
/// implementations for the `SQLModel` trait, including methods for table creation, field
/// serialization, and deserialization.
#[proc_macro_derive(Model, attributes(model))]
pub fn derive_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    // Extract the table name from the model attributes or derive a default name
    let table_name = extract_table_name(&input.attrs)
        .unwrap_or_else(|| format!("{}s", name.to_string().to_lowercase()));

    // Ensure the model is a struct with named fields
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            syn::Fields::Named(fields) => &fields.named,
            _ => panic!("Only named fields are supported"),
        },
        _ => panic!("Model can only be derived for structs"),
    };

    // Initialize variables for processing fields
    let mut primary_key_field: Option<Ident> = None;
    let mut primary_key_type: Option<Type> = None;
    let mut _pk_is_auto_increment = false;
    let mut pk_is_uuid = false;
    let mut field_sql_defs = Vec::new();
    let mut field_names = Vec::new();
    let mut field_to_sql_values = Vec::new();
    let mut field_from_row = Vec::new();
    let mut field_idents = Vec::new();
    let mut field_str_names = Vec::new();

    // Process each field in the struct
    for field in fields {
        let field_ident = field.ident.clone().unwrap();
        let field_name = field_ident.to_string();
        let mut column_name = field_name.clone();
        let mut is_primary_key = false;
        let mut has_default = false;
        let mut default_value = String::new();
        let mut is_nullable = false;
        let mut custom_type = None;
        let mut skip = false;
        let mut auto_increment = false;
        let mut uuid_pk = false;

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
                                primary_key_type = Some(field.ty.clone());
                            } else if path.is_ident("nullable") {
                                is_nullable = true;
                            } else if path.is_ident("skip") {
                                skip = true;
                            } else if path.is_ident("auto_increment") {
                                auto_increment = true;
                                _pk_is_auto_increment = true;
                            } else if path.is_ident("uuid") {
                                uuid_pk = true;
                                pk_is_uuid = true;
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

        if skip {
            continue;
        }

        field_idents.push(field_ident.clone());
        field_str_names.push(field_name.clone());
        field_names.push(column_name.clone());

        // Generate field values extraction for ToSql
        let field_to_sql_value = quote! {
            Box::new(self.#field_ident.clone()) as Box<dyn rusticx::ToSqlConvert>
        };
        field_to_sql_values.push(field_to_sql_value);

        let is_option = is_nullable || is_option_type(&field.ty);
        let field_from_json = generate_from_json(&field_ident, &column_name, &field.ty, is_option);
        field_from_row.push(field_from_json);

        let sql_type = if let Some(custom) = custom_type {
            quote! { SqlType::Custom(#custom.to_string()) }
        } else {
            let rust_type = &field.ty;
            generate_sql_type(rust_type)
        };

        let sql_def = quote! {
            {
                let mut part = format!("{} {}", #column_name, match db_type {
                    rusticx::DatabaseType::PostgreSQL => #sql_type.pg_type().to_string(),
                    rusticx::DatabaseType::MySQL => #sql_type.mysql_type().to_string(),
                    rusticx::DatabaseType::SQLite => #sql_type.sqlite_type().to_string(),
                });

                if #is_primary_key {
                    part.push_str(" PRIMARY KEY");
                    
                    // Add auto-increment syntax based on database type
                    if #auto_increment {
                        match db_type {
                            rusticx::DatabaseType::PostgreSQL => part.push_str(" GENERATED ALWAYS AS IDENTITY"),
                            rusticx::DatabaseType::MySQL => part.push_str(" AUTO_INCREMENT"),
                            rusticx::DatabaseType::SQLite => part.push_str(" AUTOINCREMENT"),
                        }
                    }
                }

                if !#is_nullable && !#is_primary_key {
                    part.push_str(" NOT NULL");
                }

                if #has_default {
                    part.push_str(&format!(" DEFAULT {}", #default_value));
                } else if #uuid_pk && #is_primary_key {
                    // Add UUID default function based on database type
                    match db_type {
                        rusticx::DatabaseType::PostgreSQL => part.push_str(" DEFAULT gen_random_uuid()"),
                        rusticx::DatabaseType::MySQL => part.push_str(" DEFAULT (UUID())"),
                        rusticx::DatabaseType::SQLite => part.push_str(" DEFAULT (lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-' || lower(hex(randomblob(2))) || '-' || lower(hex(randomblob(2))) || '-' || lower(hex(randomblob(6))))"),
                    };
                }

                part
            }
        };

        field_sql_defs.push(sql_def);
    }

    let pk_ident = primary_key_field.unwrap_or_else(|| Ident::new("id", name.span()));
    let field_name_literals: Vec<_> = field_names.iter().map(|name| quote! { #name }).collect();

    // Determine primary key type and use appropriate conversion
    let get_primary_key_code = match primary_key_type {
        Some(ref pk_type) => {
            if is_option_type(pk_type) {
                if pk_is_uuid {
                    // Handle Option<Uuid>
                    quote! {
                        self.#pk_ident.as_ref().map(|val| val.clone())
                    }
                } else {
                    // Handle Option<i32> or other numeric types
                    quote! {
                        self.#pk_ident.as_ref().map(|val| *val as i32)
                    }
                }
            } else {
                // Handle non-Option types
                if pk_is_uuid {
                    quote! { Some(self.#pk_ident.clone()) }
                } else {
                    quote! { Some(self.#pk_ident as i32) }
                }
            }
        },
        None => {
            // Default to i32 if no field is marked as primary key
            quote! { 
                if let Some(id) = &self.#pk_ident {
                    Some(*id)
                } else {
                    None
                }
            }
        }
    };

    // Expanded implementation
    let expanded = quote! {
        impl SQLModel for #name {
            fn table_name() -> String {
                #table_name.to_string()
            }

            fn primary_key_field() -> String {
                stringify!(#pk_ident).to_string()
            }

            fn primary_key_value(&self) -> Option<i32> {
                #get_primary_key_code
            }

            fn set_primary_key(&mut self, id: i32) {
                self.#pk_ident = Some(id);
            }

            fn create_table_sql(db_type: &rusticx::DatabaseType) -> String {
                let mut sql = format!("CREATE TABLE IF NOT EXISTS \"{}\" (", Self::table_name());
                let fields = vec![#(#field_sql_defs),*];
                sql.push_str(&fields.join(", "));
                sql.push(')');
                sql
            }

            fn field_names() -> Vec<&'static str> {
                vec![#(#field_name_literals),*]
            }

            fn to_sql_field_values(&self) -> Vec<Box<dyn rusticx::ToSqlConvert>> {
                vec![#(#field_to_sql_values),*]
            }

            fn from_row(row: &serde_json::Value) -> Result<Self, rusticx::RusticxError> {
                if !row.is_object() {
                    return Err(rusticx::RusticxError::DeserializationError(
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

// Helper functions
fn is_option_type(ty: &Type) -> bool {
    if let Type::Path(TypePath { path, .. }) = ty {
        if let Some(segment) = path.segments.last() {
            return segment.ident == "Option";
        }
    }
    false
}

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
                        Err(e) => return Err(rusticx::RusticxError::DeserializationError(
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
                    Err(e) => return Err(rusticx::RusticxError::DeserializationError(
                        format!("Failed to deserialize field {}: {}", #column_literal, e)
                    )),
                }
            } else {
                return Err(rusticx::RusticxError::DeserializationError(
                    format!("Missing required field: {}", #column_literal)
                ));
            }
        }
    }
}

fn generate_sql_type(rust_type: &Type) -> proc_macro2::TokenStream {
    match rust_type {
        Type::Path(TypePath { path, .. }) => {
            let segment = path.segments.last().unwrap();
            let ident = &segment.ident;
            let type_name = ident.to_string();

            if type_name == "Option" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(arg) = args.args.first() {
                        if let syn::GenericArgument::Type(inner_type) = arg {
                            return generate_sql_type(inner_type);
                        }
                    }
                }
                panic!("Invalid Option<T> type in field");
            }

            match type_name.as_str() {
                "i8" | "i16" | "i32" | "u8" | "u16" | "u32" => quote! { SqlType::Integer },
                "i64" | "u64" => quote! { SqlType::BigInt },
                "f32" | "f64" => quote! { SqlType::Float },
                "bool" => quote! { SqlType::Boolean },
                "String" | "str" => quote! { SqlType::Text },
                "Uuid" => quote! { SqlType::Text },
                "NaiveDate" => quote! { SqlType::Date },
                "NaiveTime" => quote! { SqlType::Time },
                "NaiveDateTime" | "DateTime" => quote! { SqlType::DateTime },
                "Vec" => {
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
                }
                _ => panic!("Unknown or unsupported Rust type: {}", type_name),
            }
        }
        _ => panic!("Unsupported complex type for SQL mapping"),
    }
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