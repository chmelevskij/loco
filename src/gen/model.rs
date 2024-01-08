use std::{collections::HashMap, env::current_dir};

use chrono::Utc;
use duct::cmd;
use lazy_static::lazy_static;
use rrgen::RRgen;
use serde_json::json;

use crate::app::Hooks;

const MODEL_T: &str = include_str!("templates/model.t");
const MODEL_TEST_T: &str = include_str!("templates/model_test.t");

use super::collect_messages;
use crate::{errors::Error, Result};

/// skipping some fields from the generated models.
/// For example, the `created_at` and `updated_at` fields are automatically
/// generated by the Loco app and should be given
pub const IGNORE_FIELDS: &[&str] = &["created_at", "updated_at", "create_at", "update_at"];

lazy_static! {
    static ref TYPEMAP: HashMap<&'static str, &'static str> = HashMap::from([
        ("uuid", "uuid"),
        ("string", "string_null"),
        ("string!", "string"),
        ("string^", "string_uniq"),
        ("text", "text_null"),
        ("text!", "text"),
        ("tiny_integer", "tiny_integer_null"),
        ("tiny_integer!", "tiny_integer"),
        ("tiny_integer^", "tiny_integer_uniq"),
        ("small_integer", "small_integer_null"),
        ("small_integer!", "small_integer"),
        ("small_integer^", "small_integer_uniq"),
        ("int", "integer_null"),
        ("int!", "integer"),
        ("int^", "integer_uniq"),
        ("big_integer", "big_integer_null"),
        ("big_integer!", "big_integer"),
        ("big_integer^", "big_integer_uniq"),
        ("float", "float_null"),
        ("float!", "float"),
        ("double", "double_null"),
        ("double!", "double"),
        ("decimal", "decimal_null"),
        ("decimal!", "decimal"),
        ("decimal_len", "decimal_len_null"),
        ("decimal_len!", "decimal_len"),
        ("bool", "bool_null"),
        ("bool!", "bool"),
        ("tstz", "timestamptz_null"),
        ("tstz!", "timestamptz"),
        ("date", "date_null"),
        ("date!", "date"),
        ("ts", "timestamp_null"),
        ("ts!", "timestamp"),
        ("json", "json_null"),
        ("json!", "json"),
        ("jsonb", "jsonb_null"),
        ("jsonb!", "jsonb"),
    ]);
}

pub fn generate<H: Hooks>(
    rrgen: &RRgen,
    name: &str,
    is_link: bool,
    fields: &[(String, String)],
) -> Result<String> {
    let pkg_name: &str = H::app_name();
    let ts = Utc::now();

    let mut columns = Vec::new();
    let mut references = Vec::new();
    for (fname, ftype) in fields {
        if IGNORE_FIELDS.contains(&fname.as_str()) {
            tracing::warn!(
                field = fname,
                "note that a redundant field was specified, it is already generated automatically"
            );
            continue;
        }
        if ftype == "references" {
            let fkey = format!("{fname}_id");
            columns.push((fkey.clone(), "integer"));
            // user, user_id
            references.push((fname, fkey));
        } else {
            let schema_type = TYPEMAP.get(ftype.as_str()).ok_or_else(|| {
                Error::Message(format!(
                    "type: {} not found. try any of: {:?}",
                    ftype,
                    TYPEMAP.keys()
                ))
            })?;
            columns.push((fname.to_string(), *schema_type));
        }
    }

    let vars = json!({"name": name, "ts": ts, "pkg_name": pkg_name, "is_link": is_link, "columns": columns, "references": references});
    let res1 = rrgen.generate(MODEL_T, &vars)?;
    let res2 = rrgen.generate(MODEL_TEST_T, &vars)?;

    let cwd = current_dir()?;
    let _ = cmd!("cargo", "loco", "db", "migrate",)
        .stderr_to_stdout()
        .dir(cwd.as_path())
        .run()?;
    let _ = cmd!("cargo", "loco", "db", "entities",)
        .stderr_to_stdout()
        .dir(cwd.as_path())
        .run()?;

    let messages = collect_messages(vec![res1, res2]);
    Ok(messages)
}
