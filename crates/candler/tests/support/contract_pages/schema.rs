//! The walk of a committed schema file: the rows of the Fields table, their classes and types.
//!
//! The walk reads the `*.schema.json` text only, never the Rust types (the committed file is what a
//! plug-in author is given). A `$ref` is followed into `$defs` with its sibling keywords merged, a
//! cycle is cut by a stack of the names being walked, and a depth cap stops anything deeper.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde_json::Value;

/// How deep the walk goes below a root before it stops.
const MAX_DEPTH: usize = 24;

/// The class of a field: who sets it and whether the schema requires it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// An author sets it and the object's `required` lists it.
    Required,
    /// An author may set it and `required` does not list it.
    Optional,
    /// Candler writes it and an author never sets it (`x-candler-class`).
    Candler,
    /// Stored and never read (`x-candler-class`).
    Free,
}

impl Class {
    /// The word the table and the hand classes use.
    pub fn as_str(self) -> &'static str {
        match self {
            Class::Required => "required",
            Class::Optional => "optional",
            Class::Candler => "candler",
            Class::Free => "free",
        }
    }
}

/// One row of a Fields table.
#[derive(Clone, Debug)]
pub struct Row {
    /// The dotted path: `[]` for an array item, `*` for a map entry.
    pub path: String,
    /// The alternatives of the type, joined by `|` in the table.
    pub ty: Vec<String>,
    /// The class of the field.
    pub class: Class,
    /// The `default` keyword as JSON, or `-`.
    pub default: String,
    /// The variants of a union that require the field, when only they do; empty for a field the
    /// object requires whatever the variant.
    pub variants: Vec<String>,
}

impl Row {
    /// The class column: the class word, and for a `required` field of some variants only, those
    /// variants.
    pub fn class_text(&self) -> String {
        if self.class == Class::Required && !self.variants.is_empty() {
            format!("required ({})", self.variants.join("; "))
        } else {
            self.class.as_str().to_owned()
        }
    }
}

/// The rows of one alternative of a root, or of the root itself when it has no alternatives.
#[derive(Clone, Debug)]
pub struct Block {
    /// The title of the alternative, `None` for a root with no union.
    pub title: Option<String>,
    /// The rows, in the file's own key order.
    pub rows: Vec<Row>,
}

/// Reads `<dir>/<name>.schema.json`.
pub fn load(dir: &Path, name: &str) -> Result<Value, String> {
    let path = dir.join(format!("{name}.schema.json"));
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// The tag a document carries in `schema` (`experiment/1`), when the root has one.
pub fn wire_tag(schema: &Value) -> Option<String> {
    schema
        .get("properties")?
        .get("schema")?
        .get("const")?
        .as_str()
        .map(str::to_owned)
}

/// The blocks of a root: one per alternative of a top-level union, else one.
pub fn fields(schema: &Value) -> Result<Vec<Block>, String> {
    let walker = Walker { root: schema };
    let mut blocks = Vec::new();
    let alternatives = walker.expand(schema, 0)?;
    let several = alternatives.len() > 1;
    for (number, (title, nodes)) in alternatives.into_iter().enumerate() {
        let mut table = Table::default();
        for node in &nodes {
            walker.walk(node, "", &mut Vec::new(), 0, &mut table, None)?;
        }
        let title = if !several {
            None
        } else if title.is_empty() {
            Some(format!("alternative {}", number + 1))
        } else {
            Some(title.join(" / "))
        };
        blocks.push(Block {
            title,
            rows: table.rows,
        });
    }
    Ok(blocks)
}

/// The rows being collected, merged by path.
#[derive(Default)]
struct Table {
    rows: Vec<Row>,
    index: BTreeMap<String, usize>,
}

impl Table {
    /// Adds a row; a second row at a path (the branches of a nested union) merges its types.
    fn push(&mut self, row: Row) -> Result<(), String> {
        if let Some(at) = self.index.get(&row.path).copied() {
            let Some(existing) = self.rows.get_mut(at) else {
                return Err(format!("`{}`: row index out of range", row.path));
            };
            if existing.class != row.class {
                return Err(format!(
                    "`{}` is `{}` in one branch and `{}` in another",
                    row.path,
                    existing.class.as_str(),
                    row.class.as_str()
                ));
            }
            for alternative in row.ty {
                if !existing.ty.contains(&alternative) {
                    existing.ty.push(alternative);
                }
            }
            if existing.default == "-" {
                existing.default = row.default;
            }
            if row.variants.is_empty() || existing.variants.is_empty() {
                existing.variants.clear();
            } else {
                for variant in row.variants {
                    if !existing.variants.contains(&variant) {
                        existing.variants.push(variant);
                    }
                }
            }
        } else {
            self.index.insert(row.path.clone(), self.rows.len());
            self.rows.push(row);
        }
        Ok(())
    }
}

/// The last segment of a `$ref`.
fn ref_name(node: &Value) -> Option<&str> {
    node.get("$ref")?.as_str()?.rsplit('/').next()
}

/// The `method` or `kind` property of an object when it is a required constant string:
/// `method = "outcome"`.
fn discriminator(node: &Value) -> Option<String> {
    let required: BTreeSet<&str> = required_of(node);
    node.get("properties")?
        .as_object()?
        .iter()
        .filter(|(key, _)| matches!(key.as_str(), "method" | "kind"))
        .filter(|(key, _)| required.contains(key.as_str()))
        .find_map(|(key, sub)| {
            let value = sub.get("const")?.as_str()?;
            Some(format!("{key} = \"{value}\""))
        })
}

/// The name of the required constant `method` or `kind` property of an object.
fn discriminator_key(node: &Value) -> Option<String> {
    let required: BTreeSet<&str> = required_of(node);
    node.get("properties")?
        .as_object()?
        .iter()
        .filter(|(key, _)| matches!(key.as_str(), "method" | "kind"))
        .filter(|(key, _)| required.contains(key.as_str()))
        .find(|(_, sub)| sub.get("const").is_some_and(Value::is_string))
        .map(|(key, _)| key.clone())
}

fn required_of(node: &Value) -> BTreeSet<&str> {
    node.get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect()
}

/// The values of a string enum: an `enum`, or a `oneOf` whose branches are all constant strings.
fn string_variants(node: &Value) -> Option<Vec<String>> {
    if let Some(values) = node.get("enum").and_then(Value::as_array) {
        return Some(values.iter().map(json_text).collect());
    }
    let branches = node.get("oneOf")?.as_array()?;
    let values: Option<Vec<String>> = branches
        .iter()
        .map(|branch| branch.get("const").map(json_text))
        .collect();
    values.filter(|values| !values.is_empty())
}

fn json_text(value: &Value) -> String {
    value.to_string()
}

/// The title of an alternative of a union and the objects whose properties make its block.
type Alternative = (Vec<String>, Vec<Value>);

struct Walker<'a> {
    root: &'a Value,
}

impl Walker<'_> {
    /// Follows a `$ref` into `$defs`, the sibling keywords of the reference winning.
    fn resolve(&self, node: &Value) -> Result<(Option<String>, Value), String> {
        let Some(name) = ref_name(node) else {
            return Ok((None, node.clone()));
        };
        let mut merged = self
            .root
            .get("$defs")
            .and_then(|defs| defs.get(name))
            .cloned()
            .ok_or_else(|| format!("`$ref` to `{name}`, which `$defs` does not hold"))?;
        if let (Some(into), Some(from)) = (merged.as_object_mut(), node.as_object()) {
            for (key, value) in from {
                if key != "$ref" {
                    into.insert(key.clone(), value.clone());
                }
            }
        }
        Ok((Some(name.to_owned()), merged))
    }

    /// The alternatives of a node's top-level union, each a title and the objects whose
    /// properties make the block; a node with no union is one alternative.
    fn expand(&self, node: &Value, depth: usize) -> Result<Vec<Alternative>, String> {
        let (_, merged) = self.resolve(node)?;
        let branches = ["oneOf", "anyOf"]
            .iter()
            .find_map(|key| merged.get(*key).and_then(Value::as_array))
            .filter(|branches| {
                depth < MAX_DEPTH
                    && branches
                        .iter()
                        .any(|b| b.get("$ref").is_some() || b.get("properties").is_some())
            })
            .cloned();
        let mut own = merged;
        if let Some(object) = own.as_object_mut() {
            for key in ["oneOf", "anyOf", "allOf"] {
                object.remove(key);
            }
        }
        let Some(branches) = branches else {
            let title = discriminator(&own).into_iter().collect();
            return Ok(vec![(title, vec![own])]);
        };
        let mut out = Vec::new();
        for branch in &branches {
            let segment = ref_name(branch).map(str::to_owned);
            for (inner, nodes) in self.expand(branch, depth + 1)? {
                let mut title: Vec<String> = segment.iter().cloned().collect();
                for part in inner {
                    if title.last() != Some(&part) {
                        title.push(part);
                    }
                }
                let mut all = Vec::new();
                if own.get("properties").is_some() {
                    all.push(own.clone());
                }
                all.extend(nodes);
                out.push((title, all));
            }
        }
        Ok(out)
    }

    fn walk(
        &self,
        node: &Value,
        prefix: &str,
        stack: &mut Vec<String>,
        depth: usize,
        table: &mut Table,
        variant: Option<&str>,
    ) -> Result<(), String> {
        if depth > MAX_DEPTH {
            return Ok(());
        }
        let (name, node) = self.resolve(node)?;
        if let Some(name) = &name {
            if stack.contains(name) {
                return Ok(());
            }
            stack.push(name.clone());
        }
        let required = required_of(&node);
        if let Some(properties) = node.get("properties").and_then(Value::as_object) {
            let selector = discriminator_key(&node);
            for (key, sub) in properties {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                let own = if selector.as_deref() == Some(key.as_str()) {
                    None
                } else {
                    variant
                };
                table.push(Row {
                    path: path.clone(),
                    ty: self.type_of(sub)?,
                    class: self.class_of(sub, required.contains(key.as_str()))?,
                    default: self.default_of(sub)?,
                    variants: own.map(str::to_owned).into_iter().collect(),
                })?;
                self.walk(sub, &path, stack, depth + 1, table, None)?;
            }
        }
        if let Some(items) = node.get("items").filter(|items| items.is_object()) {
            self.walk(items, &format!("{prefix}[]"), stack, depth + 1, table, None)?;
        }
        for key in ["oneOf", "anyOf", "allOf"] {
            for branch in node
                .get(key)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let named = if key == "allOf" {
                    variant.map(str::to_owned)
                } else {
                    self.variant_of(branch, prefix, self.all_single(&node, key)?)?
                };
                self.walk(branch, prefix, stack, depth + 1, table, named.as_deref())?;
            }
        }
        if name.is_some() {
            stack.pop();
        }
        Ok(())
    }

    /// What names a branch of a nested union: the constant `kind` it fixes, or for a branch that
    /// holds one key, that it is the one key of the object.
    fn variant_of(
        &self,
        branch: &Value,
        prefix: &str,
        all_single: bool,
    ) -> Result<Option<String>, String> {
        let (_, node) = self.resolve(branch)?;
        if let Some(key) = discriminator_key(&node)
            && let Some(value) = node
                .get("properties")
                .and_then(|properties| properties.get(&key))
                .and_then(|sub| sub.get("const"))
                .and_then(Value::as_str)
        {
            return Ok(Some(if prefix.is_empty() {
                format!("{key} = {value}")
            } else {
                format!("{prefix}.{key} = {value}")
            }));
        }
        Ok((all_single && !prefix.is_empty()).then(|| format!("exactly one key of {prefix}")))
    }

    /// Whether every branch of the union `key` of `node` is an object of one property: a union
    /// of keys, of which an object holds exactly one.
    fn all_single(&self, node: &Value, key: &str) -> Result<bool, String> {
        let mut single = true;
        for branch in node
            .get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let (_, branch) = self.resolve(branch)?;
            single &= branch
                .get("properties")
                .and_then(Value::as_object)
                .is_some_and(|properties| properties.len() == 1);
        }
        Ok(single)
    }

    /// `x-candler-class` on the property or on its `$ref` target, else `required` or `optional`.
    fn class_of(&self, property: &Value, required: bool) -> Result<Class, String> {
        let own = property.get("x-candler-class").and_then(Value::as_str);
        let referred = ref_name(property)
            .and_then(|name| self.root.get("$defs")?.get(name))
            .and_then(|def| def.get("x-candler-class"))
            .and_then(Value::as_str);
        match own.or(referred) {
            Some("candler") => Ok(Class::Candler),
            Some("free") => Ok(Class::Free),
            Some(other) => Err(format!("a property declares the class `{other}`")),
            None if required => Ok(Class::Required),
            None => Ok(Class::Optional),
        }
    }

    fn default_of(&self, property: &Value) -> Result<String, String> {
        let (_, node) = self.resolve(property)?;
        Ok(node
            .get("default")
            .map_or_else(|| "-".to_owned(), Value::to_string))
    }

    /// The type of a property: a constant as its value, an enum as its values, a reference as its
    /// name, an array or a map as what it holds, else the `type` keyword.
    fn type_of(&self, property: &Value) -> Result<Vec<String>, String> {
        let (name, node) = self.resolve(property)?;
        if let Some(constant) = node.get("const") {
            return Ok(vec![json_text(constant)]);
        }
        if let Some(values) = string_variants(&node) {
            return Ok(values);
        }
        if let Some(name) = name {
            return Ok(vec![name]);
        }
        match node.get("type") {
            Some(Value::String(kind)) if kind == "array" => {
                let inner = match node.get("items") {
                    Some(items) => self.type_of(items)?.join(" | "),
                    None => "any".to_owned(),
                };
                Ok(vec![format!("array of {inner}")])
            }
            Some(Value::String(kind)) if kind == "object" => {
                if node.get("properties").is_some() {
                    return Ok(vec!["object".to_owned()]);
                }
                match node.get("additionalProperties") {
                    Some(extra)
                        if extra.is_object()
                            && !extra.as_object().is_some_and(serde_json::Map::is_empty) =>
                    {
                        Ok(vec![format!("map of {}", self.type_of(extra)?.join(" | "))])
                    }
                    _ => Ok(vec!["map".to_owned()]),
                }
            }
            Some(Value::String(kind)) => Ok(vec![kind.clone()]),
            Some(Value::Array(kinds)) => {
                let kinds: Vec<String> = kinds
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect();
                // Every JSON type is no constraint at all.
                if ["string", "number", "boolean", "array", "object"]
                    .iter()
                    .all(|kind| kinds.iter().any(|k| k == kind))
                {
                    return Ok(vec!["any".to_owned()]);
                }
                Ok(kinds)
            }
            _ => {
                let mut out: Vec<String> = Vec::new();
                for key in ["oneOf", "anyOf"] {
                    for branch in node
                        .get(key)
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                    {
                        for alternative in self.type_of(branch)? {
                            if !out.contains(&alternative) {
                                out.push(alternative);
                            }
                        }
                    }
                }
                if out.is_empty() {
                    out.push("any".to_owned());
                }
                Ok(out)
            }
        }
    }
}
