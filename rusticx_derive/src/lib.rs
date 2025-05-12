use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, Attribute, Data, DeriveInput, Expr, Ident, Meta, MetaNameValue, Type,
    TypePath,
};

/// Derives the `SQLModel` trait for a struct, allowing it to be used as a database model.
///
/// This macro automatically generates the necessary implementations for the `SQLModel`
/// trait based on the struct's fields and the attributes applied to them.
///
/// # Usage
///
/// Apply `#[derive(Model)]` to your struct definition. You can also use `#[model(...)]`
/// attributes on the struct itself and on individual fields to configure the model
/// mapping and behavior.
///
/// ```rust
/// use rusticx_derive::Model; // Assuming the macro is in a crate named rusticx_derive
/// use uuid::Uuid; // Assuming you use the 'uuid' crate
/// use chrono::NaiveDateTime; // Assuming you use the 'chrono' crate
///
/// #[derive(Model, Debug, serde::Serialize, serde::Deserialize)]
/// #[model(table = "my_users")] // Optional: specify a custom table name
/// struct User {
///     #[model(primary_key, auto_increment)] // Marks 'id' as primary key with auto-increment
///     // #[model(primary_key, uuid)] // Alternatively, for UUID primary keys
///     id: Option<i32>, // Use Option<i32> for auto-increment, Uuid for uuid
///
///     name: String, // Maps to a text/varchar column
///
///     #[model(column = "user_age")] // Optional: specify a custom column name
///     age: i32, // Maps to an integer column
///
///     #[model(nullable)] // Marks the 'email' column as nullable
///     email: Option<String>,
///
///     #[model(default = "'active'")] // Sets a default value for the 'status' column
///     status: String,
///
///     #[model(sql_type = "JSONB")] // Specify a custom SQL type
///     metadata: serde_json::Value,
///
///     #[model(skip)] // This field will be ignored by the ORM
///     temp_data: String,
///
///     #[model(auto_increment)] // Only valid on primary_key fields, will be ignored otherwise
///     another_id: i32,
///
///     #[model(uuid)] // Can be used on non-primary key UUID fields if needed
///     unique_id: Uuid,
///
///     created_at: NaiveDateTime, // Maps to a datetime column
/// }
/// ```
///
/// # Struct Attributes (`#[model(...)]` on the struct)
///
/// * `#[model(table = "custom_name")]`: Specifies the database table name for this model.
///     Defaults to the struct name converted to lowercase and pluralized (e.g., `User` -> `users`).
///
/// # Field Attributes (`#[model(...)]` on fields)
///
/// * `#[model(primary_key)]`: Designates this field as the primary key for the table.
///     Exactly one field should be marked as the primary key.
/// * `#[model(column = "custom_name")]`: Specifies the database column name for this field.
///     Defaults to the field name converted to lowercase.
/// * `#[model(default = "SQL_DEFAULT_VALUE")]`: Sets a SQL default value for the column.
///     The value is inserted directly into the SQL `CREATE TABLE` statement. Use
///     appropriate quoting for string literals (e.g., `"'active'"`).
/// * `#[model(nullable)]`: Explicitly marks the column as nullable (`NULL` in SQL).
///     Fields with `Option<T>` type are automatically treated as nullable. This attribute
///     is useful for non-Option types that should still allow `NULL`.
/// * `#[model(sql_type = "SQL_TYPE_STRING")]`: Specifies a custom SQL data type for the column.
///     This overrides the default type mapping based on the Rust type.
/// * `#[model(skip)]`: Excludes this field from the generated SQL model definition (CREATE TABLE,
///     INSERT, UPDATE) and from deserialization (`from_row`).
/// * `#[model(auto_increment)]`: Applicable only to `primary_key` fields. Adds the
///     database-specific syntax for auto-incrementing integer primary keys (`SERIAL` or
///     `GENERATED ALWAYS AS IDENTITY` for PostgreSQL, `AUTO_INCREMENT` for MySQL,
///     `AUTOINCREMENT` for SQLite). The field type *must* be an integer type, usually `Option<i32>`.
/// * `#[model(uuid)]`: Applicable only to `primary_key` fields. Adds database-specific
///     default value generation for UUID primary keys (`gen_random_uuid()` for PostgreSQL,
///     `UUID()` for MySQL, and a standard UUID generation expression for SQLite). The field
///     type *must* be `uuid::Uuid` or `Option<uuid::Uuid>`.
///
/// # Generated SQL Types Mapping
///
/// The macro attempts to infer SQL types based on common Rust types:
/// * `i8`, `i16`, `i32`, `u8`, `u16`, `u32`: `INTEGER`
/// * `i64`, `u64`: `BIGINT`
/// * `f32`, `f64`: `FLOAT`
/// * `bool`: `BOOLEAN`
/// * `String`, `str`: `TEXT`
/// * `Uuid` (from `uuid` crate): `TEXT` (UUIDs are typically stored as text or byte arrays)
/// * `NaiveDate` (from `chrono` crate): `DATE`
/// * `NaiveTime` (from `chrono` crate): `TIME`
/// * `NaiveDateTime`, `DateTime` (from `chrono` crate): `DATETIME` or `TIMESTAMP` depending on DB
/// * `Vec<u8>`: `BLOB`
/// * `Option<T>`: The underlying type `T`'s mapping is used, and the column is marked nullable.
///
/// You can override this mapping using `#[model(sql_type = "...")]`.
///
/// # Requirements
///
/// * The derived struct must have named fields.
/// * The struct must derive `serde::Deserialize` for `from_row` to work.
/// * If using `Uuid` or `chrono` types, ensure the respective crates are in your `Cargo.toml`.
/// * If using the `uuid` attribute on a primary key, the field type must be `Uuid` or `Option<Uuid>`.
/// * If using the `auto_increment` attribute on a primary key, the field type must be an integer type, typically `Option<i32>`.
#[proc_macro_derive(Model, attributes(model))]
pub fn derive_model(input: TokenStream) -> TokenStream {
    // Parse the input token stream into a DeriveInput syntax tree
    let input = parse_macro_input!(input as DeriveInput);
    // Get the name of the struct
    let name = &input.ident;

    // Extract the table name from the struct attributes. If not found,
    // default to the struct name pluralized and lowercased.
    let table_name = extract_table_name(&input.attrs)
        .unwrap_or_else(|| format!("{}", name.to_string().to_lowercase()));

    // Ensure the derived item is a struct with named fields.
    // Panic otherwise with a descriptive error message.
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            syn::Fields::Named(fields) => &fields.named,
            _ => panic!("#[derive(Model)] can only be applied to structs with named fields"),
        },
        _ => panic!("#[derive(Model)] can only be applied to structs"),
    };

    // Variables to collect information about fields for code generation
    let mut primary_key_field: Option<Ident> = None;
    let mut primary_key_type: Option<Type> = None;
    let mut _pk_is_auto_increment = false; // Track if PK is auto-increment (for generated code logic if needed)
    let mut pk_is_uuid = false; // Track if PK is UUID (for generated code logic if needed)
    let mut field_sql_defs = Vec::new(); // Collect SQL column definitions (name, type, constraints)
    let mut field_names = Vec::new(); // Collect database column names
    let mut field_to_sql_values = Vec::new(); // Collect code snippets for extracting field values for SQL binding
    let mut field_from_row = Vec::new(); // Collect code snippets for deserializing fields from a row (JSON value)
    let mut field_idents = Vec::new(); // Collect original field idents
    let mut field_str_names = Vec::new(); // Collect original field names as strings

    // Iterate over each field in the struct
    for field in fields {
        let field_ident = field.ident.clone().unwrap(); // Get the field identifier
        let field_name = field_ident.to_string(); // Get the field name as a string
        let mut column_name = field_name.clone(); // Initialize column name, defaults to field name
        let mut is_primary_key = false;
        let mut has_default = false;
        let mut default_value = String::new();
        let mut is_nullable = false; // Explicit #[model(nullable)]
        let mut custom_type = None; // #[model(sql_type = "...")]
        let mut skip = false; // #[model(skip)]
        let mut auto_increment = false; // #[model(auto_increment)]
        let mut uuid_pk = false; // #[model(uuid)] for primary key

        // Process attributes on the current field
        for attr in &field.attrs {
            // Check if the attribute is our custom #[model(...)] attribute
            if !attr.path().is_ident("model") {
                continue;
            }

            // Parse the attribute's arguments (e.g., primary_key, column="...")
            let parsed = attr.parse_args_with(
                syn::punctuated::Punctuated::<Meta, syn::token::Comma>::parse_terminated,
            );

            // Process the parsed meta items within the attribute
            if let Ok(items) = parsed {
                for meta in items {
                    match meta {
                        // Handle flag attributes like `primary_key` or `nullable`
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
                                _pk_is_auto_increment = true; // Mark PK as auto-incrementing globally
                            } else if path.is_ident("uuid") {
                                uuid_pk = true;
                                pk_is_uuid = true; // Mark PK as UUID globally
                            }
                        }
                        // Handle name-value attributes like `column = "..."` or `default = "..."`
                        Meta::NameValue(MetaNameValue { path, value, .. }) => {
                            if path.is_ident("column") {
                                if let Expr::Lit(expr_lit) = value {
                                    if let syn::Lit::Str(lit_str) = expr_lit.lit {
                                        column_name = lit_str.value(); // Set custom column name
                                    }
                                }
                            } else if path.is_ident("default") {
                                if let Expr::Lit(expr_lit) = value {
                                    if let syn::Lit::Str(lit_str) = expr_lit.lit {
                                        has_default = true;
                                        default_value = lit_str.value(); // Set default value string
                                    }
                                }
                            } else if path.is_ident("sql_type") {
                                if let Expr::Lit(expr_lit) = value {
                                    if let syn::Lit::Str(lit_str) = expr_lit.lit {
                                        custom_type = Some(lit_str.value()); // Set custom SQL type string
                                    }
                                }
                            }
                        }
                        _ => {
                            // Ignore other meta types for forward compatibility
                        }
                    }
                }
            } else {
                // Handle parsing errors for the attribute arguments
                let err = parsed.unwrap_err();
                return TokenStream::from(err.to_compile_error());
            }
        }

        // If the field is marked to be skipped, continue to the next field
        if skip {
            continue;
        }

        // Store field information for later use in generated code
        field_idents.push(field_ident.clone());
        field_str_names.push(field_name.clone());
        field_names.push(column_name.clone());

        // Generate code snippet to extract the field's value.
        // Assumes the field type implements `Clone` and can be converted to `Box<dyn rusticx::ToSqlConvert>`.
        // The `rusticx::ToSqlConvert` trait would need to handle the actual type-specific conversion.
        let field_to_sql_value = quote! {
             // Clone the field value and box it as a trait object.
             // The `rusticx::ToSqlConvert` trait should provide a method
             // to convert the underlying type to database-specific parameters.
            Box::new(self.#field_ident.clone()) as Box<dyn rusticx::ToSqlConvert>
        };
        field_to_sql_values.push(field_to_sql_value);

        // Determine if the field is semantically optional (either Option<T> or explicitly nullable)
        let is_option = is_nullable || is_option_type(&field.ty);
        // Generate code snippet to deserialize the field from a JSON value (representing a database row)
        let field_from_json = generate_from_json(&field_ident, &column_name, &field.ty, is_option);
        field_from_row.push(field_from_json);

        // Determine the SQL type definition based on custom type or Rust type mapping
        let sql_type = if let Some(custom) = custom_type {
            // If a custom SQL type is specified, use it
            quote! { rusticx::SqlType::Custom(#custom.to_string()) }
        } else {
            // Otherwise, map the Rust type to a generic SqlType enum variant
            let rust_type = &field.ty;
            generate_sql_type(rust_type) // Calls helper function for mapping
        };

        // Generate the SQL column definition string part (e.g., "name TEXT NOT NULL")
        let sql_def = quote! {
            {
                // Start with column name and its determined SQL type based on DB type
                let mut part = format!("\"{}\" {}", #column_name, match db_type {
                    rusticx::DatabaseType::PostgreSQL => #sql_type.pg_type().to_string(),
                    rusticx::DatabaseType::MySQL => #sql_type.mysql_type().to_string(),
                    rusticx::DatabaseType::SQLite => #sql_type.sqlite_type().to_string(),
                });

                // Add PRIMARY KEY constraint if applicable
                if #is_primary_key {
                    part.push_str(" PRIMARY KEY");

                    // Add auto-increment or UUID default based on database type
                    if #auto_increment {
                        // Auto-increment specific syntax per database
                        match db_type {
                            rusticx::DatabaseType::PostgreSQL => part.push_str(" GENERATED ALWAYS AS IDENTITY"),
                            rusticx::DatabaseType::MySQL => part.push_str(" AUTO_INCREMENT"),
                            rusticx::DatabaseType::SQLite => part.push_str(" AUTOINCREMENT"),
                        }
                    } else if #uuid_pk {
                         // UUID default function specific syntax per database
                        match db_type {
                            rusticx::DatabaseType::PostgreSQL => part.push_str(" DEFAULT gen_random_uuid()"),
                            // MySQL's UUID() includes hyphens, stored as TEXT
                            rusticx::DatabaseType::MySQL => part.push_str(" DEFAULT (UUID())"),
                            // SQLite requires a custom expression for UUID generation
                            // This is a common pattern, might need a dedicated function in the crate
                            rusticx::DatabaseType::SQLite => part.push_str(" DEFAULT (lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-' || lower(hex(randomblob(2))) || '-' || lower(hex(randomblob(2))) || '-' || lower(hex(randomblob(6))))"),
                        };
                    }
                }

                // Add NOT NULL constraint if not nullable and not primary key
                // Primary keys are implicitly NOT NULL unless explicitly nullable
                if !#is_option && !#is_primary_key { // Check against is_option which considers Option<T> and #[model(nullable)]
                    part.push_str(" NOT NULL");
                }

                // Add DEFAULT value constraint if specified
                if #has_default {
                    part.push_str(&format!(" DEFAULT {}", #default_value));
                }

                part // Return the generated SQL part for this field
            }
        };

        field_sql_defs.push(sql_def); // Add the generated SQL definition to the list
    }

    // Determine the identifier for the primary key field for use in `primary_key_value` and `set_primary_key`.
    // Defaults to an identifier "id" if no field was marked as primary key (though this should ideally be a user error).
    let pk_ident = primary_key_field.unwrap_or_else(|| Ident::new("id", name.span()));

    // Collect column names as string literals for the `field_names` method
    let field_name_literals: Vec<_> = field_names.iter().map(|name| quote! { #name }).collect();

    // Generate the implementation for `primary_key_value`.
    // This needs to handle `Option<T>` and different primary key types (int vs UUID).
    let get_primary_key_code = match primary_key_type {
        Some(ref pk_type) => {
            if is_option_type(pk_type) {
                // Handle Option<T> primary keys
                if pk_is_uuid {
                    // If Option<Uuid>, clone the Uuid reference
                    quote! {
                        // Access the Option<Uuid> field and map to clone the Uuid if Some
                        self.#pk_ident.as_ref().map(|val| val.clone())
                    }
                } else {
                    // If Option<i32> or other Option<Integer>
                    // Assumes primary keys are returned as i32 by the ORM's fetch logic.
                    // This might need adjustment based on the actual ORM implementation's return type.
                    quote! {
                         // Access the Option<i32> field and map to the i32 if Some
                        self.#pk_ident.as_ref().map(|val| *val) // Changed to return the actual i32
                    }
                }
            } else {
                // Handle non-Option primary keys
                if pk_is_uuid {
                     // If Uuid (non-Option), just clone it and wrap in Some
                    quote! { Some(self.#pk_ident.clone()) }
                } else {
                    // If i32 or other Integer (non-Option)
                     // Assumes primary keys are returned as i32 by the ORM's fetch logic.
                    quote! { Some(self.#pk_ident) } // Changed to return the actual i32
                }
            }
        },
        // Default case if no primary key was explicitly marked. Assumes an 'id' field exists.
        // This case should ideally be an error or handled more robustly if PK is mandatory.
        None => {
            // Fallback logic assuming an `id` field of type `Option<i32>`
            // This is less ideal; enforcing a #[model(primary_key)] is better.
            // If `id` field doesn't exist or isn't Option<i32>, this will cause compilation errors.
            quote! {
                 // Access the Option<i32> field (assuming `id`)
                self.#pk_ident.as_ref().map(|val| *val) // Assuming id is Option<i32>
            }
        }
    };

    // Generate the implementation for `set_primary_key`.
    // This assumes the primary key field is an `Option<i32>`.
    // This needs refinement if UUID or non-Option primary keys are supported by `set_primary_key`.
    // Currently, `set_primary_key` takes `i32`, which fits `Option<i32>` PKs set after insert.
    // If PK is Uuid, this method signature might need to change in the trait.
    // Assuming for now that `set_primary_key` is only used for auto-generated *integer* IDs.
     let set_primary_key_code = quote! {
         // Set the primary key field value, assuming it's Option<i32>
        self.#pk_ident = Some(id);
    };


    // Construct the final generated code for the SQLModel implementation
    let expanded = quote! {
        // Implement the SQLModel trait for the target struct
        impl rusticx::SQLModel for #name {
            /// Returns the database table name for this model.
            ///
            /// This is derived from the struct name (pluralized and lowercased)
            /// or specified using the `#[model(table = "...")]` attribute.
            fn table_name() -> String {
                #table_name.to_string()
            }

            /// Returns the database column name of the primary key field.
            ///
            /// This is the field marked with `#[model(primary_key)]`.
            /// Defaults to "id" if no primary key is explicitly marked (less ideal).
            fn primary_key_field() -> String {
                 // Stringify the ident of the primary key field
                stringify!(#pk_ident).to_string()
            }

            /// Returns the value of the primary key field, if present.
            ///
            /// Returns `Some(value)` if the primary key is set, `None` otherwise.
            ///
            /// # Note
            /// The return type `Option<i32>` is assumed for auto-increment integer keys.
            /// If using UUIDs or other primary key types, the trait method signature
            /// might need adjustment in the `rusticx` crate.
            fn primary_key_value(&self) -> Option<i32> {
                // Execute the generated code snippet to get the PK value
                // The generated code handles Option<T> and type conversion (assumed i32 for now)
                // TODO: Refine signature to Option<Self::PkType> if PkType is added to trait
                #get_primary_key_code
            }

            /// Sets the value of the primary key field.
            ///
            /// This is typically used after an INSERT operation with an auto-generated ID.
            ///
            /// # Arguments
            ///
            /// * `id`: The integer value of the primary key.
            ///
            /// # Note
            /// This method assumes the primary key field is of type `Option<i32>`.
            /// It will cause a compilation error if the primary key field has a different type.
            /// A more generic trait method or separate methods for different PK types might be needed.
            fn set_primary_key(&mut self, id: i32) {
                 // Execute the generated code snippet to set the PK value
                #set_primary_key_code
            }

            /// Generates the SQL `CREATE TABLE` statement for this model.
            ///
            /// The statement is tailored to the specified database type (`db_type`).
            /// Includes column definitions, primary key constraints, nullability,
            /// defaults, and auto-increment/UUID syntax.
            ///
            /// # Arguments
            ///
            /// * `db_type`: The type of the database (PostgreSQL, MySQL, SQLite).
            ///
            /// # Returns
            ///
            /// A string containing the `CREATE TABLE` SQL statement.
            fn create_table_sql(db_type: &rusticx::DatabaseType) -> String {
                // Start the CREATE TABLE statement
                let mut sql = format!("CREATE TABLE IF NOT EXISTS \"{}\" (", Self::table_name());
                // Collect the generated SQL definitions for each field
                let fields = vec![#(#field_sql_defs),*];
                // Join field definitions with commas and close the statement
                sql.push_str(&fields.join(", "));
                sql.push(')');
                sql
            }

            /// Returns a vector of static strings representing the database column names
            /// for all non-skipped fields in the model.
            ///
            /// Used for constructing SELECT or INSERT statements.
            fn field_names() -> Vec<&'static str> {
                 // Return a vector of string literals for column names
                vec![#(#field_name_literals),*]
            }

            /// Returns a vector of boxed trait objects (`ToSqlConvert`) representing
            /// the values of all non-skipped fields in the model.
            ///
            /// Used for binding values in INSERT or UPDATE statements.
            /// Assumes field types implement `Clone` and can be converted to `ToSqlConvert`.
            fn to_sql_field_values(&self) -> Vec<Box<dyn rusticx::ToSqlConvert>> {
                 // Return a vector of boxed trait objects for field values
                vec![#(#field_to_sql_values),*]
            }

            /// Deserializes a database row (represented as a `serde_json::Value::Object`)
            /// into an instance of the model struct.
            ///
            /// # Arguments
            ///
            /// * `row`: A reference to a `serde_json::Value`, expected to be a JSON object
            ///          where keys are column names and values are column data.
            ///
            /// # Returns
            ///
            /// Returns `Ok(Self)` on successful deserialization, or a `RusticxError`
            /// if the input is not a JSON object or if field deserialization fails.
            fn from_row(row: &serde_json::Value) -> Result<Self, rusticx::RusticxError> {
                // Ensure the input value is a JSON object
                if !row.is_object() {
                    return Err(rusticx::RusticxError::DeserializationError(
                        "Input for from_row is not a JSON object".to_string()
                    ));
                }

                // Get a reference to the JSON object
                let obj = row.as_object().unwrap(); // Safe to unwrap because we checked is_object()

                // Construct the struct instance by deserializing each field
                Ok(Self {
                    #(#field_from_row),* // Execute the generated code snippets for each field
                })
            }
        }
    };

    // Return the generated code as a TokenStream
    TokenStream::from(expanded)
}

/// Helper function to check if a given Rust type is an `Option<T>`.
fn is_option_type(ty: &Type) -> bool {
    // Check if the type is a path (like `std::option::Option`)
    if let Type::Path(TypePath { path, .. }) = ty {
        // Get the last segment of the path (e.g., `Option`)
        if let Some(segment) = path.segments.last() {
            // Check if the identifier of the last segment is "Option"
            return segment.ident == "Option";
        }
    }
    false // Not an Option type
}

/// Helper function to generate the code snippet for deserializing a single field
/// from a `serde_json::Value` object (representing a database row).
///
/// Handles both optional (`Option<T>`) and required fields.
///
/// # Arguments
///
/// * `field_ident`: The identifier of the struct field.
/// * `column_name`: The database column name corresponding to the field.
/// * `_field_type`: The Rust type of the field (used implicitly by `serde_json::from_value`).
/// * `is_optional`: Boolean indicating if the field is `Option<T>` or marked nullable.
///
/// # Returns
///
/// A `proc_macro2::TokenStream` containing the code to deserialize the field.
fn generate_from_json(field_ident: &Ident, column_name: &str, _field_type: &Type, is_optional: bool) -> proc_macro2::TokenStream {
    // Use the column name as the key to look up the value in the JSON object
    let column_literal = column_name;

    if is_optional {
        // Code for optional fields (Option<T> or #[model(nullable)])
        quote! {
            #field_ident: if let Some(val) = obj.get(#column_literal) {
                // If the key exists, check if the value is null
                if val.is_null() {
                    None // If null, set field to None
                } else {
                    // If not null, attempt to deserialize the value
                    match serde_json::from_value(val.clone()) {
                        Ok(v) => Some(v), // If successful, wrap in Some
                        Err(e) => return Err(rusticx::RusticxError::DeserializationError(
                            format!("Failed to deserialize field `{}`: {}", #column_literal, e)
                        )), // If deserialization fails, return an error
                    }
                }
            } else {
                // If the key does not exist in the JSON object, treat as None
                // This handles cases where a nullable column is not included in the query result
                None
            }
        }
    } else {
        // Code for required fields (non-Option and not #[model(nullable)])
        quote! {
            #field_ident: if let Some(val) = obj.get(#column_literal) {
                // If the key exists, attempt to deserialize the value
                 match serde_json::from_value(val.clone()) {
                    Ok(v) => v, // If successful, use the value
                    Err(e) => return Err(rusticx::RusticxError::DeserializationError(
                        format!("Failed to deserialize field `{}`: {}", #column_literal, e)
                    )), // If deserialization fails, return an error
                }
            } else {
                // If the key does not exist for a required field, return an error
                return Err(rusticx::RusticxError::DeserializationError(
                    format!("Missing required field: `{}`", #column_literal)
                ));
            }
        }
    }
}

/// Helper function to map a Rust type to a generic `SqlType` enum variant.
///
/// This mapping is used to determine the database column type in the `CREATE TABLE` statement,
/// which is then translated to the database-specific syntax by the `SqlType` methods.
///
/// # Arguments
///
/// * `rust_type`: The `syn::Type` of the Rust field.
///
/// # Returns
///
/// A `proc_macro2::TokenStream` representing the corresponding `rusticx::SqlType` variant.
///
/// # Panics
///
/// Panics if the Rust type is not recognized or supported by the mapping.
fn generate_sql_type(rust_type: &Type) -> proc_macro2::TokenStream {
    // Only support path types (like `i32`, `String`, `Option<T>`, etc.)
    match rust_type {
        Type::Path(TypePath { path, .. }) => {
            // Get the last segment of the path
            let segment = path.segments.last().unwrap();
            let ident = &segment.ident;
            let type_name = ident.to_string();

            // Handle Option<T> recursively: get the inner type's mapping
            if type_name == "Option" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(arg) = args.args.first() {
                        if let syn::GenericArgument::Type(inner_type) = arg {
                            // Recursively call for the inner type
                            return generate_sql_type(inner_type);
                        }
                    }
                }
                // Panic if Option type has invalid arguments
                panic!("Invalid Option<T> type specification for field: {}", quote!{#rust_type});
            }

            // Map common Rust types to SqlType variants
            match type_name.as_str() {
                "i8" | "i16" | "i32" | "u8" | "u16" | "u32" => quote! { rusticx::SqlType::Integer },
                "i64" | "u64" => quote! { rusticx::SqlType::BigInt },
                "f32" | "f64" => quote! { rusticx::SqlType::Float },
                "bool" => quote! { rusticx::SqlType::Boolean },
                // Map String/str to Text
                "String" | "str" => quote! { rusticx::SqlType::Text },
                // Map Uuid (from `uuid` crate) to Text (common storage, can be overridden)
                "Uuid" => quote! { rusticx::SqlType::Text },
                // Map chrono date/time types
                "NaiveDate" => quote! { rusticx::SqlType::Date },
                "NaiveTime" => quote! { rusticx::SqlType::Time },
                "NaiveDateTime" | "DateTime" => quote! { rusticx::SqlType::DateTime },
                // Map Vec<u8> to Blob
                "Vec" => {
                    if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                        if let Some(arg) = args.args.first() {
                            if let syn::GenericArgument::Type(Type::Path(TypePath { path, .. })) = arg {
                                if let Some(seg) = path.segments.last() {
                                    if seg.ident == "u8" {
                                        return quote! { rusticx::SqlType::Blob };
                                    }
                                }
                            }
                        }
                    }
                    // Fallback for other Vec types, treat as Blob (might need refinement)
                    quote! { rusticx::SqlType::Blob }
                }
                // Panic for unknown types
                _ => panic!("Unknown or unsupported Rust type for SQL mapping: `{}`. Consider using #[model(sql_type = \"...\")]", quote!{#rust_type}),
            }
        }
        // Panic for other complex types (arrays, tuples, pointers, etc.)
        _ => panic!("Unsupported complex type for SQL mapping: `{}`. Only simple path types and Option<T> are automatically mapped. Consider using #[model(sql_type = \"...\")]", quote!{#rust_type}),
    }
}

/// Helper function to extract the custom table name from the struct-level `#[model(table = "...")]` attribute.
///
/// # Arguments
///
/// * `attrs`: A slice of `syn::Attribute` applied to the struct.
///
/// # Returns
///
/// An `Option<String>` containing the custom table name if found, otherwise `None`.
fn extract_table_name(attrs: &[Attribute]) -> Option<String> {
    // Iterate through all attributes on the struct
    for attr in attrs {
        // Check if the attribute is our custom #[model(...)] attribute
        if !attr.path().is_ident("model") {
            continue;
        }

        // Parse the attribute's arguments
        let parsed = attr.parse_args_with(
            syn::punctuated::Punctuated::<Meta, syn::token::Comma>::parse_terminated,
        );

        // Process the parsed meta items
        if let Ok(items) = parsed {
            for meta in items {
                // Check for the `table = "..."` name-value pair
                if let Meta::NameValue(MetaNameValue { path, value, .. }) = meta {
                    if path.is_ident("table") {
                        // If found, extract the string literal value
                        if let Expr::Lit(expr_lit) = value {
                            if let syn::Lit::Str(lit_str) = expr_lit.lit {
                                return Some(lit_str.value()); // Return the extracted table name
                            }
                        }
                    }
                }
            }
        } else {
             // Log or handle parsing errors for struct attributes if necessary
             // For simplicity, we ignore errors here and let subsequent logic handle missing name
             let _ = parsed.unwrap_err(); // Consume the error
        }
    }
    None // No custom table name found
}