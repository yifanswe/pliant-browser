use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Number, Value};

pub const SCHEMA_VERSION: u64 = 1;
pub const MAX_DEFINITION_BYTES: usize = 64 * 1024;
pub const MAX_NODE_DEPTH: usize = 32;
pub const MAX_NODE_COUNT: usize = 256;
pub const MAX_STRING_BYTES: usize = 16 * 1024;
pub const MAX_STRING_VALUE_BYTES: usize = 2048;
pub const MAX_RUNTIME_ADDRESS_BYTES: usize = MAX_STRING_VALUE_BYTES;
pub const MAX_CONTAINER_GAP: f64 = 256.0;
pub const MAX_SPACER_SIZE: f64 = 4096.0;
pub const BUILT_IN_SAFE_DEFINITION: &str = r#"{
  "schema_version": 1,
  "address_open": "current_page",
  "root": {
    "type": "column",
    "id": "safe-root",
    "children": [
      {
        "type": "row",
        "id": "safe-toolbar",
        "children": [
          {
            "type": "button",
            "id": "safe-back",
            "label": "Back",
            "command": {"type": "back"}
          },
          {
            "type": "button",
            "id": "safe-forward",
            "label": "Forward",
            "command": {"type": "forward"}
          },
          {
            "type": "address_field",
            "id": "safe-address",
            "placeholder": "Enter address"
          },
          {
            "type": "button",
            "id": "safe-new-page",
            "label": "New page",
            "command": {"type": "new_page"}
          }
        ]
      },
      {"type": "page_list", "id": "safe-pages"},
      {"type": "content_surface", "id": "safe-content"}
    ]
  }
}"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressOpenTarget {
    CurrentPage,
    NewPage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddressOpenResolution {
    NavigateCurrent { page_id: String, url: String },
    OpenNewPage { url: String },
}

pub fn resolve_address_open(
    target: AddressOpenTarget,
    current_page_id: Option<&str>,
    address: &str,
) -> Result<AddressOpenResolution, DefinitionError> {
    if address.len() > MAX_RUNTIME_ADDRESS_BYTES {
        return Err(DefinitionError {
            kind: ErrorKind::InputTooLarge,
            message: format!(
                "runtime address is {} bytes; maximum is {MAX_RUNTIME_ADDRESS_BYTES} bytes",
                address.len()
            ),
        });
    }
    validate_navigation_url(address, "runtime address")?;
    let resolution = match (target, current_page_id) {
        (AddressOpenTarget::CurrentPage, Some(page_id)) => AddressOpenResolution::NavigateCurrent {
            page_id: page_id.to_owned(),
            url: address.to_owned(),
        },
        (AddressOpenTarget::CurrentPage, None) | (AddressOpenTarget::NewPage, _) => {
            AddressOpenResolution::OpenNewPage {
                url: address.to_owned(),
            }
        }
    };
    Ok(resolution)
}

#[derive(Debug, Clone, PartialEq)]
pub struct BrowserDefinition {
    schema_version: u64,
    address_open_target: AddressOpenTarget,
    root: Node,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DefinitionSnapshot {
    source: String,
    definition: BrowserDefinition,
}

impl DefinitionSnapshot {
    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn definition(&self) -> &BrowserDefinition {
        &self.definition
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CandidateId {
    owner_id: u64,
    sequence: u64,
}

static NEXT_STATE_OWNER_ID: AtomicU64 = AtomicU64::new(1);

fn next_state_owner_id() -> u64 {
    NEXT_STATE_OWNER_ID
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        .expect("definition state owner ID space exhausted")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateErrorKind {
    StaleCandidate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateError {
    kind: StateErrorKind,
    message: String,
}

impl StateError {
    pub fn kind(&self) -> StateErrorKind {
        self.kind
    }
}

impl fmt::Display for StateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for StateError {}

#[derive(Debug, Clone)]
struct PreviewCandidate {
    id: CandidateId,
    snapshot: DefinitionSnapshot,
}

#[derive(Debug)]
pub struct DefinitionState {
    owner_id: u64,
    safe: DefinitionSnapshot,
    active: DefinitionSnapshot,
    pending: Option<PreviewCandidate>,
    next_candidate_sequence: u64,
}

impl Clone for DefinitionState {
    fn clone(&self) -> Self {
        let owner_id = next_state_owner_id();
        let pending = self.pending.as_ref().map(|candidate| PreviewCandidate {
            id: CandidateId {
                owner_id,
                sequence: candidate.id.sequence,
            },
            snapshot: candidate.snapshot.clone(),
        });
        Self {
            owner_id,
            safe: self.safe.clone(),
            active: self.active.clone(),
            pending,
            next_candidate_sequence: self.next_candidate_sequence,
        }
    }
}

impl DefinitionState {
    pub fn new(initial_source: &str) -> Result<Self, DefinitionError> {
        let definition = parse_definition(initial_source)?;
        let safe = DefinitionSnapshot {
            source: BUILT_IN_SAFE_DEFINITION.to_owned(),
            definition: parse_definition(BUILT_IN_SAFE_DEFINITION)?,
        };
        Ok(Self {
            safe,
            active: DefinitionSnapshot {
                source: initial_source.to_owned(),
                definition,
            },
            pending: None,
            owner_id: next_state_owner_id(),
            next_candidate_sequence: 1,
        })
    }

    pub fn active(&self) -> &DefinitionSnapshot {
        &self.active
    }

    pub fn pending_candidate_id(&self) -> Option<CandidateId> {
        self.pending.as_ref().map(|candidate| candidate.id)
    }

    pub fn pending(&self) -> Option<(CandidateId, &DefinitionSnapshot)> {
        self.pending
            .as_ref()
            .map(|candidate| (candidate.id, &candidate.snapshot))
    }

    pub fn preview(&mut self, source: &str) -> Result<CandidateId, DefinitionError> {
        let definition = parse_definition(source)?;
        let next_candidate_sequence = self
            .next_candidate_sequence
            .checked_add(1)
            .ok_or_else(|| invalid("candidate ID space exhausted".to_owned()))?;
        let id = CandidateId {
            owner_id: self.owner_id,
            sequence: self.next_candidate_sequence,
        };
        self.next_candidate_sequence = next_candidate_sequence;
        self.pending = Some(PreviewCandidate {
            id,
            snapshot: DefinitionSnapshot {
                source: source.to_owned(),
                definition,
            },
        });
        Ok(id)
    }

    pub fn apply(&mut self, candidate_id: CandidateId) -> Result<(), StateError> {
        match self.pending.take() {
            Some(candidate) if candidate.id == candidate_id => {
                self.active = candidate.snapshot;
                Ok(())
            }
            pending => {
                self.pending = pending;
                Err(stale_candidate_error())
            }
        }
    }

    pub fn reject(&mut self, candidate_id: CandidateId) -> Result<(), StateError> {
        if self.pending_candidate_id() != Some(candidate_id) {
            return Err(stale_candidate_error());
        }
        self.pending = None;
        Ok(())
    }

    pub fn reset(&mut self) {
        self.active = self.safe.clone();
        self.pending = None;
    }
}

fn stale_candidate_error() -> StateError {
    StateError {
        kind: StateErrorKind::StaleCandidate,
        message: "preview candidate is stale or no longer pending".to_owned(),
    }
}

impl BrowserDefinition {
    pub fn schema_version(&self) -> u64 {
        self.schema_version
    }

    pub fn address_open_target(&self) -> AddressOpenTarget {
        self.address_open_target
    }

    pub fn root(&self) -> &Node {
        &self.root
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Row(ContainerNode),
    Column(ContainerNode),
    Spacer(SpacerNode),
    Label(LabelNode),
    AddressField(AddressFieldNode),
    PageList(PageListNode),
    ContentSurface(ContentSurfaceNode),
    Button(ButtonNode),
}

impl Node {
    pub fn id(&self) -> &str {
        match self {
            Self::Row(node) | Self::Column(node) => node.id(),
            Self::Spacer(node) => node.id(),
            Self::Label(node) => node.id(),
            Self::AddressField(node) => node.id(),
            Self::PageList(node) => node.id(),
            Self::ContentSurface(node) => node.id(),
            Self::Button(node) => node.id(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContainerNode {
    id: String,
    children: Vec<Node>,
    gap: Option<f64>,
}

impl ContainerNode {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn children(&self) -> &[Node] {
        &self.children
    }

    pub fn gap(&self) -> Option<f64> {
        self.gap
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpacerNode {
    id: String,
    size: f64,
}

impl SpacerNode {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn size(&self) -> f64 {
        self.size
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelNode {
    id: String,
    text: String,
}

impl LabelNode {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressFieldNode {
    id: String,
    placeholder: Option<String>,
}

impl AddressFieldNode {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn placeholder(&self) -> Option<&str> {
        self.placeholder.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageListNode {
    id: String,
}

impl PageListNode {
    pub fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentSurfaceNode {
    id: String,
}

impl ContentSurfaceNode {
    pub fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ButtonNode {
    id: String,
    label: String,
    command: Command,
}

impl ButtonNode {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn command(&self) -> &Command {
        &self.command
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Navigate { url: String },
    Back,
    Forward,
    Close,
    NewPage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    InputTooLarge,
    DepthLimitExceeded,
    NodeLimitExceeded,
    StringLimitExceeded,
    MalformedJson,
    UnsupportedSchemaVersion,
    UnknownField,
    UnknownCommand,
    InvalidCommand,
    DuplicateNodeId,
    InvalidStructure,
    InvalidValue,
    UnauthorizedUrl,
    InvalidDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefinitionError {
    kind: ErrorKind,
    message: String,
}

impl DefinitionError {
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }
}

impl fmt::Display for DefinitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for DefinitionError {}

struct UniqueJsonValue(Value);

impl<'de> Deserialize<'de> for UniqueJsonValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(UniqueJsonValueVisitor)
    }
}

struct UniqueJsonValueVisitor;

impl<'de> Visitor<'de> for UniqueJsonValueVisitor {
    type Value = UniqueJsonValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value without duplicate object keys")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(UniqueJsonValue(Value::Bool(value)))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(UniqueJsonValue(Value::Number(Number::from(value))))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(UniqueJsonValue(Value::Number(Number::from(value))))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Number::from_f64(value)
            .map(Value::Number)
            .map(UniqueJsonValue)
            .ok_or_else(|| E::custom("JSON number must be finite"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(UniqueJsonValue(Value::String(value.to_owned())))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(UniqueJsonValue(Value::String(value)))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(UniqueJsonValue(Value::Null))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(UniqueJsonValue(Value::Null))
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        UniqueJsonValue::deserialize(deserializer)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element::<UniqueJsonValue>()? {
            values.push(value.0);
        }
        Ok(UniqueJsonValue(Value::Array(values)))
    }

    fn visit_map<A>(self, mut entries: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut object = Map::new();
        while let Some(key) = entries.next_key::<String>()? {
            if object.contains_key(&key) {
                return Err(de::Error::custom(format!("duplicate object key `{key}`")));
            }
            let value = entries.next_value::<UniqueJsonValue>()?;
            object.insert(key, value.0);
        }
        Ok(UniqueJsonValue(Value::Object(object)))
    }
}

fn deserialize_unique_json(source: &str) -> Result<Value, serde_json::Error> {
    let mut deserializer = serde_json::Deserializer::from_str(source);
    let value = UniqueJsonValue::deserialize(&mut deserializer)?;
    deserializer.end()?;
    Ok(value.0)
}

pub fn parse_definition(source: &str) -> Result<BrowserDefinition, DefinitionError> {
    if source.len() > MAX_DEFINITION_BYTES {
        return Err(DefinitionError {
            kind: ErrorKind::InputTooLarge,
            message: format!(
                "definition is {} bytes; maximum is {MAX_DEFINITION_BYTES} bytes",
                source.len()
            ),
        });
    }
    let value = deserialize_unique_json(source).map_err(|error| DefinitionError {
        kind: ErrorKind::MalformedJson,
        message: format!("invalid definition JSON: {error}"),
    })?;
    validate_string_budgets(&value)?;
    let object = object_at(&value, "definition")?;
    ensure_known_fields(
        object,
        &["schema_version", "address_open", "root"],
        "definition",
    )?;
    let schema_version = u64_at(
        required(object, "schema_version", "definition")?,
        "schema_version",
    )?;
    if schema_version != SCHEMA_VERSION {
        return Err(DefinitionError {
            kind: ErrorKind::UnsupportedSchemaVersion,
            message: format!("schema_version must be {SCHEMA_VERSION}, got {schema_version}"),
        });
    }
    let address_open_target = match string_at(
        required(object, "address_open", "definition")?,
        "address_open",
    )? {
        "current_page" => AddressOpenTarget::CurrentPage,
        "new_page" => AddressOpenTarget::NewPage,
        value => {
            return Err(invalid_value(format!(
                "address_open has unsupported value `{value}`"
            )));
        }
    };
    let mut node_count = 0;
    let root = parse_node(
        required(object, "root", "definition")?,
        "root",
        1,
        &mut node_count,
    )?;
    validate_unique_node_ids(&root)?;
    validate_structure(&root)?;

    Ok(BrowserDefinition {
        schema_version,
        address_open_target,
        root,
    })
}

fn validate_string_budgets(value: &Value) -> Result<(), DefinitionError> {
    fn visit(value: &Value, path: &str, total: &mut usize) -> Result<(), DefinitionError> {
        match value {
            Value::String(string) => {
                if string.len() > MAX_STRING_VALUE_BYTES {
                    return Err(DefinitionError {
                        kind: ErrorKind::StringLimitExceeded,
                        message: format!(
                            "{path} is {} bytes; maximum string value is {MAX_STRING_VALUE_BYTES} bytes",
                            string.len()
                        ),
                    });
                }
                *total = total.saturating_add(string.len());
                if *total > MAX_STRING_BYTES {
                    return Err(DefinitionError {
                        kind: ErrorKind::StringLimitExceeded,
                        message: format!(
                            "total string content exceeds the {MAX_STRING_BYTES} byte limit at {path}"
                        ),
                    });
                }
            }
            Value::Array(values) => {
                for (index, value) in values.iter().enumerate() {
                    visit(value, &format!("{path}[{index}]"), total)?;
                }
            }
            Value::Object(object) => {
                for (field, value) in object {
                    visit(value, &format!("{path}.{field}"), total)?;
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) => {}
        }
        Ok(())
    }

    visit(value, "definition", &mut 0)
}

fn validate_structure(root: &Node) -> Result<(), DefinitionError> {
    if !matches!(root, Node::Row(_) | Node::Column(_)) {
        return Err(structure_error("root must be a row or column"));
    }

    #[derive(Default)]
    struct Counts {
        address_fields: usize,
        page_lists: usize,
        content_surfaces: usize,
    }

    fn visit(node: &Node, counts: &mut Counts) {
        match node {
            Node::Row(container) | Node::Column(container) => {
                for child in container.children() {
                    visit(child, counts);
                }
            }
            Node::AddressField(_) => counts.address_fields += 1,
            Node::PageList(_) => counts.page_lists += 1,
            Node::ContentSurface(_) => counts.content_surfaces += 1,
            Node::Spacer(_) | Node::Label(_) | Node::Button(_) => {}
        }
    }

    let mut counts = Counts::default();
    visit(root, &mut counts);
    if counts.content_surfaces != 1 {
        return Err(structure_error(format!(
            "definition must contain exactly one content_surface, found {}",
            counts.content_surfaces
        )));
    }
    if counts.address_fields == 0 {
        return Err(structure_error(
            "definition must contain at least one address_field",
        ));
    }
    if counts.page_lists == 0 {
        return Err(structure_error(
            "definition must contain at least one page_list",
        ));
    }
    Ok(())
}

fn structure_error(message: impl Into<String>) -> DefinitionError {
    DefinitionError {
        kind: ErrorKind::InvalidStructure,
        message: message.into(),
    }
}

fn validate_unique_node_ids(root: &Node) -> Result<(), DefinitionError> {
    fn visit(
        node: &Node,
        path: &str,
        seen: &mut HashMap<String, String>,
    ) -> Result<(), DefinitionError> {
        if let Some(first_path) = seen.insert(node.id().to_owned(), path.to_owned()) {
            return Err(DefinitionError {
                kind: ErrorKind::DuplicateNodeId,
                message: format!(
                    "duplicate node id `{}` at {path}; first used at {first_path}",
                    node.id()
                ),
            });
        }

        let children = match node {
            Node::Row(container) | Node::Column(container) => container.children(),
            _ => return Ok(()),
        };
        for (index, child) in children.iter().enumerate() {
            visit(child, &format!("{path}.children[{index}]"), seen)?;
        }
        Ok(())
    }

    visit(root, "root", &mut HashMap::new())
}

fn parse_node(
    value: &Value,
    path: &str,
    depth: usize,
    node_count: &mut usize,
) -> Result<Node, DefinitionError> {
    if depth > MAX_NODE_DEPTH {
        return Err(DefinitionError {
            kind: ErrorKind::DepthLimitExceeded,
            message: format!("{path} exceeds the maximum node depth of {MAX_NODE_DEPTH}"),
        });
    }
    *node_count += 1;
    if *node_count > MAX_NODE_COUNT {
        return Err(DefinitionError {
            kind: ErrorKind::NodeLimitExceeded,
            message: format!(
                "{path} exceeds the maximum definition node count of {MAX_NODE_COUNT}"
            ),
        });
    }
    let object = object_at(value, path)?;
    let node_type = string_at(required(object, "type", path)?, &format!("{path}.type"))?;
    let id_path = format!("{path}.id");
    let id = validated_id(
        string_at(required(object, "id", path)?, &id_path)?,
        &id_path,
    )?;

    match node_type {
        "row" | "column" => {
            ensure_known_fields(object, &["type", "id", "children", "gap"], path)?;
            let children_value = required(object, "children", path)?;
            let children_array = children_value
                .as_array()
                .ok_or_else(|| invalid(format!("{path}.children must be an array")))?;
            let mut children = Vec::with_capacity(children_array.len());
            for (index, child) in children_array.iter().enumerate() {
                children.push(parse_node(
                    child,
                    &format!("{path}.children[{index}]"),
                    depth + 1,
                    node_count,
                )?);
            }
            let gap = object
                .get("gap")
                .map(|value| number_at(value, &format!("{path}.gap")))
                .transpose()?;
            if let Some(gap) = gap {
                if !(0.0..=MAX_CONTAINER_GAP).contains(&gap) {
                    return Err(invalid_value(format!(
                        "{path}.gap must be between 0 and {MAX_CONTAINER_GAP}"
                    )));
                }
            }
            let container = ContainerNode { id, children, gap };
            if node_type == "row" {
                Ok(Node::Row(container))
            } else {
                Ok(Node::Column(container))
            }
        }
        "spacer" => {
            ensure_known_fields(object, &["type", "id", "size"], path)?;
            let size = number_at(required(object, "size", path)?, &format!("{path}.size"))?;
            if !(0.0..=MAX_SPACER_SIZE).contains(&size) {
                return Err(invalid_value(format!(
                    "{path}.size must be between 0 and {MAX_SPACER_SIZE}"
                )));
            }
            Ok(Node::Spacer(SpacerNode { id, size }))
        }
        "label" => {
            ensure_known_fields(object, &["type", "id", "text"], path)?;
            let text_path = format!("{path}.text");
            Ok(Node::Label(LabelNode {
                id,
                text: non_empty_text(
                    string_at(required(object, "text", path)?, &text_path)?,
                    &text_path,
                )?
                .to_owned(),
            }))
        }
        "address_field" => {
            ensure_known_fields(object, &["type", "id", "placeholder"], path)?;
            Ok(Node::AddressField(AddressFieldNode {
                id,
                placeholder: object
                    .get("placeholder")
                    .map(|value| {
                        let placeholder_path = format!("{path}.placeholder");
                        non_empty_text(string_at(value, &placeholder_path)?, &placeholder_path)
                            .map(str::to_owned)
                    })
                    .transpose()?,
            }))
        }
        "page_list" => {
            ensure_known_fields(object, &["type", "id"], path)?;
            Ok(Node::PageList(PageListNode { id }))
        }
        "content_surface" => {
            ensure_known_fields(object, &["type", "id"], path)?;
            Ok(Node::ContentSurface(ContentSurfaceNode { id }))
        }
        "button" => {
            ensure_known_fields(object, &["type", "id", "label", "command"], path)?;
            let label_path = format!("{path}.label");
            Ok(Node::Button(ButtonNode {
                id,
                label: non_empty_text(
                    string_at(required(object, "label", path)?, &label_path)?,
                    &label_path,
                )?
                .to_owned(),
                command: parse_command(
                    required(object, "command", path)?,
                    &format!("{path}.command"),
                )?,
            }))
        }
        value => Err(invalid(format!(
            "{path}.type has unsupported node type `{value}`"
        ))),
    }
}

fn parse_command(value: &Value, path: &str) -> Result<Command, DefinitionError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid_command(format!("{path} must be an object")))?;
    let command_type = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_command(format!("{path}.type must be a string")))?;
    match command_type {
        "navigate" => {
            ensure_known_fields(object, &["type", "url"], path)?;
            let url_path = format!("{path}.url");
            let url = object
                .get("url")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid_command(format!("{url_path} must be a string")))?;
            validate_navigation_url(url, &url_path)?;
            Ok(Command::Navigate {
                url: url.to_owned(),
            })
        }
        "back" => {
            ensure_known_fields(object, &["type"], path)?;
            Ok(Command::Back)
        }
        "forward" => {
            ensure_known_fields(object, &["type"], path)?;
            Ok(Command::Forward)
        }
        "close" => {
            ensure_known_fields(object, &["type"], path)?;
            Ok(Command::Close)
        }
        "new_page" => {
            ensure_known_fields(object, &["type"], path)?;
            Ok(Command::NewPage)
        }
        value => Err(DefinitionError {
            kind: ErrorKind::UnknownCommand,
            message: format!("{path}.type has unsupported command `{value}`"),
        }),
    }
}

fn ensure_known_fields(
    object: &Map<String, Value>,
    allowed: &[&str],
    path: &str,
) -> Result<(), DefinitionError> {
    if let Some(field) = object
        .keys()
        .find(|field| !allowed.contains(&field.as_str()))
    {
        return Err(DefinitionError {
            kind: ErrorKind::UnknownField,
            message: format!("{path} contains unknown field `{field}`"),
        });
    }
    Ok(())
}

fn required<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    path: &str,
) -> Result<&'a Value, DefinitionError> {
    object
        .get(field)
        .ok_or_else(|| invalid(format!("{path} is missing required field `{field}`")))
}

fn object_at<'a>(value: &'a Value, path: &str) -> Result<&'a Map<String, Value>, DefinitionError> {
    value
        .as_object()
        .ok_or_else(|| invalid(format!("{path} must be an object")))
}

fn string_at<'a>(value: &'a Value, path: &str) -> Result<&'a str, DefinitionError> {
    value
        .as_str()
        .ok_or_else(|| invalid(format!("{path} must be a string")))
}

fn u64_at(value: &Value, path: &str) -> Result<u64, DefinitionError> {
    value
        .as_u64()
        .ok_or_else(|| invalid(format!("{path} must be an unsigned integer")))
}

fn number_at(value: &Value, path: &str) -> Result<f64, DefinitionError> {
    let number = value
        .as_f64()
        .ok_or_else(|| invalid(format!("{path} must be a finite number")))?;
    if !number.is_finite() {
        return Err(invalid_value(format!("{path} must be finite")));
    }
    Ok(number)
}

fn validated_id(id: &str, path: &str) -> Result<String, DefinitionError> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(invalid_value(format!(
            "{path} must contain only ASCII letters, digits, `.`, `_`, or `-`"
        )));
    }
    Ok(id.to_owned())
}

fn non_empty_text<'a>(text: &'a str, path: &str) -> Result<&'a str, DefinitionError> {
    if text.trim().is_empty() {
        return Err(invalid_value(format!("{path} must not be empty")));
    }
    Ok(text)
}

fn validate_navigation_url(url: &str, path: &str) -> Result<(), DefinitionError> {
    let lower = url.to_ascii_lowercase();
    let valid = if lower == "about:blank" {
        true
    } else if let Some(remainder) = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
    {
        let authority = remainder.split(['/', '?', '#']).next().unwrap_or_default();
        !authority.is_empty()
    } else {
        false
    };
    if !valid || url.chars().any(char::is_whitespace) || url.chars().any(char::is_control) {
        return Err(DefinitionError {
            kind: ErrorKind::UnauthorizedUrl,
            message: format!(
                "{path} navigate URL `{url}` is not an allowed http, https, or about:blank target"
            ),
        });
    }
    Ok(())
}

fn invalid(message: String) -> DefinitionError {
    DefinitionError {
        kind: ErrorKind::InvalidDefinition,
        message,
    }
}

fn invalid_value(message: String) -> DefinitionError {
    DefinitionError {
        kind: ErrorKind::InvalidValue,
        message,
    }
}

fn invalid_command(message: String) -> DefinitionError {
    DefinitionError {
        kind: ErrorKind::InvalidCommand,
        message,
    }
}
