//! Source PMA dialect entry over its authoritative PARG and declared bindings.
use parsanol::parg::bindings_artifact::PargBindingsArtifact;
use parsanol::{PargArtifact, PargError};
use serde_json::{json, Value};

fn stage_error(error: PargError) -> Value {
    match error {
        PargError::CallDepthLimitExceeded { depth, max_depth } => {
            json!({"code":"RESOURCE_LIMIT","category":"call_depth_limit","depth":depth,"max_depth":max_depth})
        }
        PargError::RecursionLimitExceeded { depth, max_depth } => {
            json!({"code":"RESOURCE_LIMIT","category":"recursion_limit","depth":depth,"max_depth":max_depth})
        }
        error => {
            json!({"code":"PARSE_ERROR","message":error.to_string(),"offset_policy":"source-pma-artifact"})
        }
    }
}
fn incomplete(message: &str) -> Value {
    json!({"code":"INCOMPLETE_PMA_TRANSLATION","message":message})
}

pub fn parse(input: &str, options: &Value) -> Result<Value, Value> {
    if options
        .as_object()
        .is_some_and(|options| !options.is_empty())
    {
        return Err(
            json!({"code":"PARSE_OPTION_ERROR","message":"PMA version1 accepts no locale or decimal parse options"}),
        );
    }
    let units: Value = serde_json::from_str(asciiunits::ARTIFACT)
        .map_err(|error| incomplete(&error.to_string()))?;
    if units["checksum"]
        != "sha256:aa22618c80dc02f3fe1edf2c798dbe34aceae5f4d84524ff98f76a2b91af151d"
    {
        return Err(incomplete(
            "PMA's source AsciiUnits artifact dependency differs from the accepted stage",
        ));
    }
    let artifact =
        PargArtifact::from_json(include_str!("pma_parse/pma.json")).map_err(stage_error)?;
    let bindings =
        PargBindingsArtifact::from_json(include_str!("pma_parse/pma.bindings.rs.json"), Some("rs"))
            .map_err(stage_error)?;
    let shape = artifact
        .parse_shape("document", input)
        .map_err(stage_error)?;
    let bound = bindings
        .apply(&artifact, "document", &shape)
        .map_err(stage_error)?;
    if input.trim_start().starts_with("diff(") {
        return Err(
            json!({"code":"PARSE_ERROR","message":"diff(q) has no Plurimath node yet (docs/pma.adoc, open decision 9)"}),
        );
    }
    let term = &bound["term"];
    if let Some(literal) = term["unit"]["literal"].as_str() {
        let mut model = crate::units_parse::parse(literal, &json!({}))?;
        model["fields"]["input_string"] = json!(input);
        return Ok(model);
    }
    Err(incomplete("PMA measured/quantity terms require their source uncertainty constructor and quantity policy; unit terms are implemented"))
}
