use serde_json::Value;

use crate::services::ai::{self as ai_service, FindSimilarParams, SearchByObjectParams};

use super::HeadlessContext;

pub fn search_by_object(ctx: &HeadlessContext, params: Value) -> Result<Value, String> {
    let parsed: SearchByObjectParams = serde_json::from_value(params)
        .map_err(|error| format!("Invalid search_by_object params: {error}"))?;
    let matches = ai_service::search_by_object_in_database(&ctx.db, &parsed)
        .map_err(|error| error.to_string())?;
    serde_json::to_value(matches).map_err(|error| error.to_string())
}

pub fn find_similar(ctx: &HeadlessContext, params: Value) -> Result<Value, String> {
    let parsed: FindSimilarParams = serde_json::from_value(params)
        .map_err(|error| format!("Invalid find_similar params: {error}"))?;
    let matches = ai_service::find_similar_in_database(&ctx.db, &parsed)
        .map_err(|error| error.to_string())?;
    serde_json::to_value(matches).map_err(|error| error.to_string())
}
