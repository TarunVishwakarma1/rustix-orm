use crate::connection::Connection;
use crate::model::SQLModel;
use crate::error::RustixError;

pub struct QueryBuilder {
    filters: Vec<(String, Vec<Box<dyn std::fmt::Debug>>)>,
    order_by_field: Option<String>,
    order_asc: bool,
    limit_val: Option<usize>,
    offset_val: Option<usize>,
}

impl QueryBuilder {
    pub fn new() -> Self {
        QueryBuilder {
            filters: Vec::new(),
            order_by_field: None,
            order_asc: true,
            limit_val: None,
            offset_val: None,
        }
    }
    
    pub fn filter<T>(mut self, condition: &str, params: &[T]) -> Self
where
    T: std::fmt::Debug + Clone + 'static,
{
    let boxed_params = params
        .iter()
        .map(|p| Box::new(p.clone()) as Box<dyn std::fmt::Debug>)
        .collect();
    self.filters.push((condition.to_string(), boxed_params));
    self
}
    
    pub fn order_by(mut self, field: &str, asc: bool) -> Self {
        self.order_by_field = Some(field.to_string());
        self.order_asc = asc;
        self
    }
    
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit_val = Some(limit);
        self
    }
    
    pub fn offset(mut self, offset: usize) -> Self {
        self.offset_val = Some(offset);
        self
    }
    
    pub fn find_all<T: SQLModel>(self, conn: &Connection) -> Result<Vec<T>, RustixError> {
        // Build SQL from the query components
        let mut sql = format!("SELECT * FROM {}", T::table_name());
        
        if !self.filters.is_empty() {
            sql.push_str(" WHERE ");
            for (i, (condition, _)) in self.filters.iter().enumerate() {
                if i > 0 {
                    sql.push_str(" AND ");
                }
                sql.push_str(condition);
            }
        }
        
        if let Some(field) = self.order_by_field {
            sql.push_str(&format!(" ORDER BY {} {}", field, if self.order_asc { "ASC" } else { "DESC" }));
        }
        
        if let Some(limit) = self.limit_val {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        
        if let Some(offset) = self.offset_val {
            sql.push_str(&format!(" OFFSET {}", offset));
        }
        
        println!("Generated SQL: {}", sql);
        
        // In a real implementation, this would execute the SQL and map results
        Ok(Vec::new())
    }
}