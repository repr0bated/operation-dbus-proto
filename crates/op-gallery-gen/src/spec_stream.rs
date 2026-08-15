//! Assemble a spec from the model's JSONL patch stream.
//!
//! json-render's model contract is not "emit a spec object" — it is a stream of
//! RFC 6902 patch operations, one JSON object per line, applied to an empty
//! document until it becomes a spec:
//!
//! ```text
//! {"op":"add","path":"/root","value":"main"}
//! {"op":"add","path":"/elements/main","value":{"type":"card","children":["s1"]}}
//! {"op":"add","path":"/state/peers","value":[]}
//! {"op":"add","path":"/state/peers/0","value":{"id":"1"}}
//! ```
//!
//! The catalog's own prompt (`schemas/json-render/catalog.prompt.md`) is what
//! instructs the model to emit this, and that prompt is generated, so this
//! assembler exists to meet their contract rather than to define one of ours.
//! Streaming is the point of the contract: state patches follow the elements
//! that read them, so a renderer can fill a UI in as it arrives.
//!
//! Only the operations the contract uses are implemented — `add`, `replace`,
//! `remove`. An unrecognised operation is an error, not a skipped line: silently
//! dropping an operation would yield a spec the model did not describe.

use anyhow::{anyhow, bail, Result};
use serde_json::{Map, Value};

/// What the assembler found in one model turn.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StreamStats {
    /// Patch operations applied.
    pub applied: usize,
    /// Lines that were not patch operations (prose, fences, blank lines).
    pub ignored: usize,
}

/// Assemble a spec document from a JSONL patch stream.
///
/// Prose and code fences are tolerated: models drift into explaining
/// themselves, and a turn is expensive enough that discarding one over a
/// stray sentence is worse than ignoring the sentence.
pub fn assemble(content: &str) -> Result<(Value, StreamStats)> {
    let mut doc = Value::Object(Map::new());
    let mut stats = StreamStats::default();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("```") {
            continue;
        }

        let Some(op) = parse_operation(line) else {
            stats.ignored += 1;
            continue;
        };

        apply(&mut doc, &op)?;
        stats.applied += 1;
    }

    if stats.applied == 0 {
        bail!("model output contained no patch operations");
    }

    if doc.get("root").and_then(Value::as_str).is_none() {
        bail!("patch stream never set /root to a string");
    }
    if doc.get("elements").and_then(Value::as_object).is_none() {
        bail!("patch stream never set /elements to an object");
    }

    Ok((doc, stats))
}

/// One patch operation.
struct Operation {
    op: String,
    path: String,
    value: Option<Value>,
}

/// Parse a line as a patch operation, or `None` if it is not one.
fn parse_operation(line: &str) -> Option<Operation> {
    let parsed: Value = serde_json::from_str(line).ok()?;
    let object = parsed.as_object()?;
    Some(Operation {
        op: object.get("op")?.as_str()?.to_string(),
        path: object.get("path")?.as_str()?.to_string(),
        value: object.get("value").cloned(),
    })
}

fn apply(doc: &mut Value, operation: &Operation) -> Result<()> {
    let tokens = parse_pointer(&operation.path)?;

    match operation.op.as_str() {
        "add" | "replace" => {
            let value = operation
                .value
                .clone()
                .ok_or_else(|| anyhow!("{} at {} has no value", operation.op, operation.path))?;
            set(doc, &tokens, value)
        }
        "remove" => remove(doc, &tokens),
        other => bail!(
            "unsupported patch operation '{other}' at {}",
            operation.path
        ),
    }
}

/// Split a JSON Pointer into unescaped tokens.
fn parse_pointer(pointer: &str) -> Result<Vec<String>> {
    if pointer.is_empty() {
        bail!("patch targets the whole document, which would discard the spec");
    }
    if !pointer.starts_with('/') {
        bail!("'{pointer}' is not a JSON Pointer (must start with '/')");
    }

    // Order matters: ~01 must decode to "~1", so ~1 is resolved before ~0.
    Ok(pointer[1..]
        .split('/')
        .map(|token| token.replace("~1", "/").replace("~0", "~"))
        .collect())
}

fn set(doc: &mut Value, tokens: &[String], value: Value) -> Result<()> {
    let (last, parents) = tokens
        .split_last()
        .ok_or_else(|| anyhow!("empty pointer"))?;

    let mut cursor = doc;
    for (depth, token) in parents.iter().enumerate() {
        // The container a token needs depends on the *next* token: an index
        // wants an array, a name wants an object.
        let next_is_index = is_index(&tokens[depth + 1]);
        cursor = descend(cursor, token, next_is_index)?;
    }

    match cursor {
        Value::Object(map) => {
            map.insert(last.clone(), value);
            Ok(())
        }
        Value::Array(items) => {
            if last == "-" {
                items.push(value);
                return Ok(());
            }
            let index: usize = last
                .parse()
                .map_err(|_| anyhow!("'{last}' is not an array index"))?;
            match index.cmp(&items.len()) {
                std::cmp::Ordering::Less => {
                    items[index] = value;
                    Ok(())
                }
                std::cmp::Ordering::Equal => {
                    items.push(value);
                    Ok(())
                }
                // Growing an array with a gap would leave holes the model never
                // described, so it is refused rather than padded with nulls.
                std::cmp::Ordering::Greater => Err(anyhow!(
                    "index {index} is past the end of an array of {} items",
                    items.len()
                )),
            }
        }
        other => Err(anyhow!("cannot set '{last}' on a {}", kind_of(other))),
    }
}

/// Step into `token`, creating the container the next token needs.
fn descend<'a>(cursor: &'a mut Value, token: &str, next_is_index: bool) -> Result<&'a mut Value> {
    let fresh = || {
        if next_is_index {
            Value::Array(Vec::new())
        } else {
            Value::Object(Map::new())
        }
    };

    match cursor {
        Value::Object(map) => Ok(map.entry(token.to_string()).or_insert_with(fresh)),
        Value::Array(items) => {
            if token == "-" {
                items.push(fresh());
                let last = items.len() - 1;
                return Ok(&mut items[last]);
            }
            let index: usize = token
                .parse()
                .map_err(|_| anyhow!("'{token}' is not an array index"))?;
            if index == items.len() {
                items.push(fresh());
            }
            items
                .get_mut(index)
                .ok_or_else(|| anyhow!("index {index} is past the end of the array"))
        }
        other => Err(anyhow!(
            "cannot descend into '{token}': parent is a {}",
            kind_of(other)
        )),
    }
}

fn remove(doc: &mut Value, tokens: &[String]) -> Result<()> {
    let (last, parents) = tokens
        .split_last()
        .ok_or_else(|| anyhow!("empty pointer"))?;

    let mut cursor = doc;
    for token in parents {
        cursor = match cursor {
            Value::Object(map) => map
                .get_mut(token.as_str())
                .ok_or_else(|| anyhow!("remove target's parent '{token}' does not exist"))?,
            Value::Array(items) => {
                let index: usize = token
                    .parse()
                    .map_err(|_| anyhow!("'{token}' is not an array index"))?;
                items
                    .get_mut(index)
                    .ok_or_else(|| anyhow!("remove target's parent index {index} does not exist"))?
            }
            other => bail!(
                "cannot descend into '{token}': parent is a {}",
                kind_of(other)
            ),
        };
    }

    match cursor {
        Value::Object(map) => {
            map.remove(last.as_str())
                .ok_or_else(|| anyhow!("cannot remove '{last}': it does not exist"))?;
            Ok(())
        }
        Value::Array(items) => {
            let index: usize = last
                .parse()
                .map_err(|_| anyhow!("'{last}' is not an array index"))?;
            if index >= items.len() {
                bail!(
                    "cannot remove index {index}: array has {} items",
                    items.len()
                );
            }
            items.remove(index);
            Ok(())
        }
        other => Err(anyhow!("cannot remove '{last}' from a {}", kind_of(other))),
    }
}

fn is_index(token: &str) -> bool {
    token == "-" || token.chars().all(|c| c.is_ascii_digit()) && !token.is_empty()
}

fn kind_of(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assembles_the_stream_shape_the_catalog_prompt_documents() {
        // Copied from the generated prompt's own example, so a change in their
        // contract shows up here.
        let stream = r#"
{"op":"add","path":"/root","value":"main"}
{"op":"add","path":"/elements/main","value":{"type":"appShell","props":{"navWidth":"14rem"},"children":["list"]}}
{"op":"add","path":"/elements/list","value":{"type":"topbar","props":{},"repeat":{"statePath":"/items","key":"id"},"children":[]}}
{"op":"add","path":"/state/items","value":[]}
{"op":"add","path":"/state/items/0","value":{"id":"1","title":"First Item"}}
{"op":"add","path":"/state/items/1","value":{"id":"2","title":"Second Item"}}
"#;

        let (spec, stats) = assemble(stream).expect("stream should assemble");
        assert_eq!(stats.applied, 6);
        assert_eq!(stats.ignored, 0);
        assert_eq!(spec["root"], "main");
        assert_eq!(spec["elements"]["main"]["type"], "appShell");
        assert_eq!(spec["state"]["items"][1]["title"], "Second Item");
        assert_eq!(spec["state"]["items"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn prose_and_fences_are_ignored_not_fatal() {
        let stream = "Here is a dashboard for you:\n\
             ```jsonl\n\
             {\"op\":\"add\",\"path\":\"/root\",\"value\":\"r\"}\n\
             {\"op\":\"add\",\"path\":\"/elements/r\",\"value\":{\"type\":\"card\"}}\n\
             ```\n\
             Let me know if you want another section.";

        let (spec, stats) = assemble(stream).expect("prose should not defeat assembly");
        assert_eq!(stats.applied, 2);
        assert_eq!(stats.ignored, 2, "the two prose lines should be counted");
        assert_eq!(spec["root"], "r");
    }

    #[test]
    fn replace_and_remove_are_applied() {
        let stream = r#"
{"op":"add","path":"/root","value":"r"}
{"op":"add","path":"/elements/r","value":{"type":"card","props":{"title":"old","tone":null}}}
{"op":"replace","path":"/elements/r/props/title","value":"new"}
{"op":"add","path":"/state/rows","value":["a","b","c"]}
{"op":"remove","path":"/state/rows/1"}
"#;

        let (spec, _) = assemble(stream).unwrap();
        assert_eq!(spec["elements"]["r"]["props"]["title"], "new");
        assert_eq!(spec["state"]["rows"], serde_json::json!(["a", "c"]));
    }

    #[test]
    fn append_token_grows_an_array() {
        let stream = r#"
{"op":"add","path":"/root","value":"r"}
{"op":"add","path":"/elements/r","value":{"type":"card"}}
{"op":"add","path":"/state/rows","value":[]}
{"op":"add","path":"/state/rows/-","value":"first"}
{"op":"add","path":"/state/rows/-","value":"second"}
"#;

        let (spec, _) = assemble(stream).unwrap();
        assert_eq!(
            spec["state"]["rows"],
            serde_json::json!(["first", "second"])
        );
    }

    #[test]
    fn a_gap_in_an_array_is_refused() {
        // Padding with nulls would invent list items the model never described.
        let stream = r#"
{"op":"add","path":"/root","value":"r"}
{"op":"add","path":"/elements/r","value":{"type":"card"}}
{"op":"add","path":"/state/rows","value":[]}
{"op":"add","path":"/state/rows/3","value":"late"}
"#;

        let error = assemble(stream).expect_err("a gap must not be silently filled");
        assert!(
            error.to_string().contains("past the end"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn escaped_pointer_tokens_are_decoded() {
        let stream = r#"
{"op":"add","path":"/root","value":"r"}
{"op":"add","path":"/elements/r","value":{"type":"card"}}
{"op":"add","path":"/state/a~1b","value":1}
{"op":"add","path":"/state/c~0d","value":2}
"#;

        let (spec, _) = assemble(stream).unwrap();
        assert_eq!(spec["state"]["a/b"], 1);
        assert_eq!(spec["state"]["c~d"], 2);
    }

    #[test]
    fn an_unsupported_operation_is_an_error() {
        let stream = r#"
{"op":"add","path":"/root","value":"r"}
{"op":"add","path":"/elements/r","value":{"type":"card"}}
{"op":"move","from":"/elements/r","path":"/elements/q"}
"#;

        let error = assemble(stream).expect_err("unknown ops must not be dropped");
        assert!(error.to_string().contains("unsupported patch operation"));
    }

    #[test]
    fn a_stream_without_root_or_elements_is_incomplete() {
        let no_root = r#"{"op":"add","path":"/elements/r","value":{"type":"card"}}"#;
        assert!(assemble(no_root)
            .expect_err("missing root")
            .to_string()
            .contains("/root"));

        let no_elements = r#"{"op":"add","path":"/root","value":"r"}"#;
        assert!(assemble(no_elements)
            .expect_err("missing elements")
            .to_string()
            .contains("/elements"));
    }

    #[test]
    fn prose_only_output_is_rejected() {
        let error = assemble("I would be happy to help! What kind of dashboard?")
            .expect_err("no operations means no spec");
        assert!(error.to_string().contains("no patch operations"));
    }

    #[test]
    fn a_pointer_that_is_not_a_pointer_is_refused() {
        let stream = "{\"op\":\"add\",\"path\":\"root\",\"value\":\"r\"}";
        let error = assemble(stream).expect_err("relative paths must be refused");
        assert!(error.to_string().contains("not a JSON Pointer"));
    }
}
