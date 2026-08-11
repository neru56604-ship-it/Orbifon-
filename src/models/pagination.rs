use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct PaginationQuery {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}
