use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use ron::extensions::Extensions;
use ron::{Options, Value};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::indexer::ComponentCatalog;
use crate::model::{
    ComponentKind, ConstantDefinition, EditableConfig, GraphDocument, GraphEdge, GraphNode,
    MonitorInstance, NumericStorage, PortDescriptor, PortDirection, NC_ENDPOINT, normalize_type,
};
use crate::project::atomic_write;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid Copper RON: {0}")]
    Parse(String),
    #[error("cannot save because the file changed outside Evograph")]
    ExternalChange,
    #[error("cannot save while hard validation errors remain")]
    Validation,
    #[error("failed to write {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
}

#[derive(Debug, Clone)]
pub struct ConfigFile {
    pub path: PathBuf,
    pub original_source: String,
    pub original_hash: [u8; 32],
    pub document: GraphDocument,
    original_document: GraphDocument,
}

impl ConfigFile {
    pub fn load(path: impl AsRef<Path>, catalog: &ComponentCatalog) -> Result<Self, ConfigError> {
        let path = path.as_ref().to_path_buf();
        let source = if path.exists() {
            fs::read_to_string(&path).map_err(|source| ConfigError::Read {
                path: path.clone(),
                source,
            })?
        } else {
            "(\n    tasks: [],\n    cnx: [],\n)\n".to_string()
        };
        let parsed: RawRoot = copper_ron_options()
            .from_str(&source)
            .map_err(|error| ConfigError::Parse(error.to_string()))?;
        let document = build_document(parsed, &source, catalog)?;
        Ok(Self {
            path,
            original_hash: hash(&source),
            original_source: source,
            original_document: document.clone(),
            document,
        })
    }

    pub fn disk_changed(&self) -> bool {
        fs::read_to_string(&self.path)
            .map(|source| hash(&source) != self.original_hash)
            .unwrap_or(self.path.exists())
    }

    pub fn rendered(&self) -> Result<String, ConfigError> {
        if self.document.has_hard_errors() {
            return Err(ConfigError::Validation);
        }
        let mut source = self.original_source.clone();
        let monitor_field = if find_field_value_span(&source, "monitors").is_some() {
            "monitors"
        } else if find_field_value_span(&source, "monitor").is_some()
            && self.document.monitors.len() <= 1
        {
            "monitor"
        } else {
            "monitors"
        };
        let nodes_changed = self.document.nodes != self.original_document.nodes;
        let mut replacements = Vec::new();
        if nodes_changed {
            replacements.push(("tasks", render_tasks(&self.document)));
            replacements.push(("bridges", render_bridges(&self.document)));
        }
        if nodes_changed || self.document.edges != self.original_document.edges {
            replacements.push(("cnx", render_connections(&self.document)));
        }
        if self.document.constants != self.original_document.constants {
            replacements.push(("constants", render_constants(&self.document)));
        }
        if self.document.monitors != self.original_document.monitors {
            if monitor_field == "monitor" {
                let value = self
                    .document
                    .monitors
                    .first()
                    .map(render_monitor)
                    .unwrap_or_else(|| "(type: \"cu29::monitoring::NoMonitor\")".to_string());
                replacements.push(("monitor", value));
            } else {
                replacements.push(("monitors", render_monitors(&self.document)));
            }
        }

        // Replacing from right to left keeps all previously calculated spans valid.
        let mut existing = Vec::new();
        let mut missing = Vec::new();
        for (name, value) in replacements {
            if let Some((start, end)) = find_field_value_span(&source, name) {
                existing.push((start, end, value));
            } else if should_insert(name, &self.document) {
                missing.push((name, value));
            }
        }
        existing.sort_by_key(|(start, _, _)| std::cmp::Reverse(*start));
        for (start, end, value) in existing {
            source.replace_range(start..end, &value);
        }
        for (name, value) in missing {
            source = insert_root_field(&source, name, &value)?;
        }
        copper_ron_options()
            .from_str::<RawRoot>(&source)
            .map_err(|error| ConfigError::Parse(error.to_string()))?;
        let cst = ronin_core::parse(&source);
        if !cst.diagnostics().is_empty() {
            return Err(ConfigError::Parse(format!(
                "lossless RON parser reported {} diagnostic(s)",
                cst.diagnostics().len()
            )));
        }
        Ok(ronin_core::print(&cst))
    }

    pub fn save(&mut self, overwrite_external: bool) -> Result<(), ConfigError> {
        if !overwrite_external && self.disk_changed() {
            return Err(ConfigError::ExternalChange);
        }
        let rendered = self.rendered()?;
        atomic_write(&self.path, rendered.as_bytes()).map_err(|source| ConfigError::Write {
            path: self.path.clone(),
            source,
        })?;
        self.original_hash = hash(&rendered);
        self.original_source = rendered;
        self.original_document = self.document.clone();
        Ok(())
    }
}

fn copper_ron_options() -> Options {
    Options::default().with_default_extension(Extensions::IMPLICIT_SOME)
}

#[derive(Debug, Default, Deserialize)]
struct RawRoot {
    #[serde(default)]
    tasks: Vec<RawTask>,
    #[serde(default)]
    bridges: Vec<RawBridge>,
    #[serde(default)]
    cnx: Vec<RawConnection>,
    #[serde(default)]
    monitor: Option<RawMonitor>,
    #[serde(default)]
    monitors: Vec<RawMonitor>,
}

#[derive(Debug, Deserialize)]
struct RawTask {
    id: String,
    #[serde(rename = "type")]
    type_: Option<String>,
    #[serde(default)]
    kind: Option<RawTaskKind>,
    #[serde(default)]
    config: BTreeMap<String, Value>,
    background: Option<bool>,
    run_in_sim: Option<bool>,
    logging: Option<RawLogging>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum RawTaskKind {
    Source,
    #[serde(alias = "regular")]
    Task,
    Sink,
}

#[derive(Debug, Deserialize)]
struct RawLogging {
    enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct RawBridge {
    id: String,
    #[serde(rename = "type")]
    type_: String,
    #[serde(default)]
    config: BTreeMap<String, Value>,
    #[serde(default)]
    channels: Vec<RawBridgeChannel>,
    run_in_sim: Option<bool>,
}

#[derive(Debug, Deserialize)]
enum RawBridgeChannel {
    Rx {
        id: String,
        route: Option<String>,
        #[serde(default)]
        config: BTreeMap<String, Value>,
    },
    Tx {
        id: String,
        route: Option<String>,
        #[serde(default)]
        config: BTreeMap<String, Value>,
    },
}

#[derive(Debug, Deserialize)]
struct RawConnection {
    src: String,
    dst: String,
    msg: String,
}

#[derive(Debug, Deserialize)]
struct RawMonitor {
    #[serde(rename = "type")]
    type_: String,
    #[serde(default)]
    config: BTreeMap<String, Value>,
}

fn build_document(
    raw: RawRoot,
    source: &str,
    catalog: &ComponentCatalog,
) -> Result<GraphDocument, ConfigError> {
    let mut document = GraphDocument::default();
    let task_extras = section_extras(
        source,
        "tasks",
        &["id", "type", "kind", "config", "background", "run_in_sim", "logging"],
    );
    let bridge_extras = section_extras(
        source,
        "bridges",
        &["id", "type", "config", "channels", "run_in_sim"],
    );
    let connection_extras = section_extras(source, "cnx", &["src", "dst", "msg"]);
    for (task_index, task) in raw.tasks.into_iter().enumerate() {
        let type_path = task.type_.unwrap_or_default();
        let descriptor = catalog.find(&type_path);
        let kind = descriptor.map(|component| component.kind).unwrap_or_else(|| {
            task.kind
                .map(|kind| match kind {
                    RawTaskKind::Source => ComponentKind::Source,
                    RawTaskKind::Task => ComponentKind::Task,
                    RawTaskKind::Sink => ComponentKind::Sink,
                })
                .unwrap_or(ComponentKind::Unresolved)
        });
        document.nodes.push(GraphNode {
            id: task.id,
            component_type: type_path,
            kind,
            inputs: descriptor.map(|item| item.inputs.clone()).unwrap_or_default(),
            outputs: descriptor.map(|item| item.outputs.clone()).unwrap_or_default(),
            config: values_to_config(task.config),
            channels: vec![],
            background: task.background,
            run_in_sim: task.run_in_sim,
            logging_enabled: task.logging.and_then(|logging| logging.enabled),
            pending_workspace_dependency: None,
            unresolved: descriptor.is_none(),
            extra_fields: task_extras.get(task_index).cloned().unwrap_or_default(),
        });
    }
    for (bridge_index, bridge) in raw.bridges.into_iter().enumerate() {
        let descriptor = catalog.find(&bridge.type_);
        let mut node = descriptor.map_or_else(
            || GraphNode {
                id: bridge.id.clone(),
                component_type: bridge.type_.clone(),
                kind: ComponentKind::Bridge,
                inputs: vec![],
                outputs: vec![],
                config: EditableConfig::default(),
                channels: vec![],
                background: None,
                run_in_sim: bridge.run_in_sim,
                logging_enabled: None,
                pending_workspace_dependency: None,
                unresolved: true,
                extra_fields: BTreeMap::new(),
            },
            |component| GraphNode::from_component(component, bridge.id.clone()),
        );
        node.config = values_to_config(bridge.config);
        node.run_in_sim = bridge.run_in_sim;
        node.extra_fields = bridge_extras
            .get(bridge_index)
            .cloned()
            .unwrap_or_default();
        node.channels.clear();
        node.inputs.clear();
        node.outputs.clear();
        for channel in bridge.channels {
            let (id, direction, route, config) = match channel {
                RawBridgeChannel::Rx { id, route, config } => {
                    (id, PortDirection::Output, route, config)
                }
                RawBridgeChannel::Tx { id, route, config } => {
                    (id, PortDirection::Input, route, config)
                }
            };
            let discovered = descriptor.and_then(|component| {
                component
                    .channels
                    .iter()
                    .find(|candidate| candidate.id == id && candidate.direction == direction)
            });
            let ordinal = if direction == PortDirection::Input {
                node.inputs.len()
            } else {
                node.outputs.len()
            };
            let port = discovered.map(|channel| channel.payload.clone()).unwrap_or_else(|| {
                inferred_port(direction, ordinal, &id, "_unresolved")
            });
            if direction == PortDirection::Input {
                node.inputs.push(port);
            } else {
                node.outputs.push(port);
            }
            node.channels.push(crate::model::BridgeChannelInstance {
                id,
                direction,
                route,
                config: values_to_config(config),
                enabled: true,
            });
        }
        document.nodes.push(node);
    }

    let node_by_id: HashMap<String, usize> = document
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.clone(), index))
        .collect();
    for (connection_index, connection) in raw.cnx.into_iter().enumerate() {
        let (source_id, source_channel) = split_endpoint(&connection.src);
        let Some(&source) = node_by_id.get(source_id) else {
            continue;
        };
        let source_port = ensure_port(
            &mut document.nodes[source],
            PortDirection::Output,
            source_channel,
            &connection.msg,
            &[],
        );
        if connection.dst == NC_ENDPOINT {
            continue;
        }
        let (target_id, target_channel) = split_endpoint(&connection.dst);
        let Some(&target) = node_by_id.get(target_id) else {
            continue;
        };
        let occupied_inputs = document
            .edges
            .iter()
            .filter(|edge| edge.target == target)
            .map(|edge| edge.target_port)
            .collect::<Vec<_>>();
        let target_port = ensure_port(
            &mut document.nodes[target],
            PortDirection::Input,
            target_channel,
            &connection.msg,
            &occupied_inputs,
        );
        document.edges.push(GraphEdge {
            source,
            source_port,
            target,
            target_port,
            message_type: connection.msg,
            extra_fields: connection_extras
                .get(connection_index)
                .cloned()
                .unwrap_or_default(),
        });
    }
    for node in &mut document.nodes {
        if node.kind == ComponentKind::Unresolved {
            node.kind = match (node.inputs.is_empty(), node.outputs.is_empty()) {
                (true, false) => ComponentKind::Source,
                (false, true) => ComponentKind::Sink,
                (false, false) => ComponentKind::Task,
                (true, true) => ComponentKind::Unresolved,
            };
        }
    }
    document.monitors = raw
        .monitors
        .into_iter()
        .chain(raw.monitor)
        .map(|monitor| MonitorInstance {
            component_type: monitor.type_,
            config: values_to_config(monitor.config),
        })
        .collect();
    document.constants = parse_constants(source);
    Ok(document)
}

fn ensure_port(
    node: &mut GraphNode,
    direction: PortDirection,
    channel: Option<&str>,
    message: &str,
    unavailable: &[usize],
) -> usize {
    let ports = if direction == PortDirection::Input {
        &mut node.inputs
    } else {
        &mut node.outputs
    };
    if let Some(channel) = channel
        && let Some(index) = ports.iter().position(|port| port.name == channel)
    {
        let port = &mut ports[index];
        if port.canonical_type == "_unresolved" {
            port.declared_type = message.to_string();
            port.serialized_type = message.to_string();
            port.canonical_type = message.to_string();
        }
        return index;
    }
    let normalized_message = normalize_type(message);
    if let Some(index) = ports.iter().enumerate().find_map(|(index, port)| {
        (!unavailable.contains(&index)
            && normalize_type(&port.serialized_type) == normalized_message)
            .then_some(index)
    }) {
        return index;
    }
    if let Some(index) = ports.iter().enumerate().find_map(|(index, port)| {
        (!unavailable.contains(&index)
            && normalize_type(&port.canonical_type) == normalized_message)
            .then_some(index)
    }) {
        return index;
    }
    let ordinal = ports.len();
    ports.push(inferred_port(
        direction,
        ordinal,
        channel.unwrap_or(message),
        message,
    ));
    ordinal
}

fn inferred_port(
    direction: PortDirection,
    ordinal: usize,
    name: &str,
    message: &str,
) -> PortDescriptor {
    PortDescriptor {
        name: name.to_string(),
        direction,
        ordinal,
        declared_type: message.to_string(),
        serialized_type: message.to_string(),
        canonical_type: message.to_string(),
    }
}

fn split_endpoint(endpoint: &str) -> (&str, Option<&str>) {
    endpoint
        .split_once('/')
        .map_or((endpoint, None), |(node, channel)| (node, Some(channel)))
}

fn values_to_config(values: BTreeMap<String, Value>) -> EditableConfig {
    EditableConfig(values_to_raw(values))
}

fn values_to_raw(values: BTreeMap<String, Value>) -> BTreeMap<String, String> {
    values
        .into_iter()
        .map(|(key, value)| {
            let value = ron::ser::to_string(&value).unwrap_or_else(|_| "()".to_string());
            (key, value)
        })
        .collect()
}

fn section_extras(
    source: &str,
    section: &str,
    known_fields: &[&str],
) -> Vec<BTreeMap<String, String>> {
    let Some((start, end)) = find_field_value_span(source, section) else {
        return Vec::new();
    };
    split_collection_items(&source[start..end], '[', ']')
        .into_iter()
        .map(|item| {
            parse_struct_fields(item)
                .into_iter()
                .filter(|(key, _)| !known_fields.contains(&key.as_str()))
                .collect()
        })
        .collect()
}

fn parse_constants(source: &str) -> Vec<ConstantDefinition> {
    let Some((start, end)) = find_field_value_span(source, "constants") else {
        return vec![];
    };
    split_collection_items(&source[start..end], '[', ']')
        .into_iter()
        .filter_map(|item| {
            let fields = parse_struct_fields(item);
            let id = fields.get("id").and_then(|value| ron::from_str(value).ok())?;
            let module = fields
                .get("module")
                .and_then(|value| ron::from_str(value).ok())
                .unwrap_or_default();
            if let (Some(rust_type), Some(expression)) = (fields.get("type"), fields.get("expression")) {
                return Some(ConstantDefinition::Expression {
                    id,
                    module,
                    rust_type: ron::from_str(rust_type).ok()?,
                    expression: ron::from_str(expression).ok()?,
                });
            }
            let storage = fields
                .get("storage")
                .and_then(|value| parse_storage(value.trim()))
                .unwrap_or(NumericStorage::F32);
            Some(ConstantDefinition::Numeric {
                id,
                module,
                storage,
                quantity: fields.get("quantity").map(|value| value.trim().to_string()),
                unit: fields.get("unit").map(|value| value.trim().to_string()),
                value_ron: fields.get("value").cloned().unwrap_or_else(|| "0.0".into()),
            })
        })
        .collect()
}

fn parse_storage(value: &str) -> Option<NumericStorage> {
    NumericStorage::ALL
        .into_iter()
        .find(|storage| storage.ron_name() == value)
}

fn render_tasks(document: &GraphDocument) -> String {
    let tasks: Vec<_> = document
        .nodes
        .iter()
        .filter(|node| node.kind != ComponentKind::Bridge)
        .map(|node| {
            let mut fields = vec![
                format!("id: {},", ron_string(&node.id)),
                format!("type: {},", ron_string(&node.component_type)),
            ];
            if let Some(kind) = node.kind.ron_task_kind() {
                fields.push(format!("kind: {kind},"));
            }
            if !node.config.0.is_empty() {
                fields.push(format!("config: {},", render_config(&node.config)));
            }
            if let Some(background) = node.background {
                fields.push(format!("background: {background},"));
            }
            if let Some(run_in_sim) = node.run_in_sim {
                fields.push(format!("run_in_sim: {run_in_sim},"));
            }
            if let Some(enabled) = node.logging_enabled {
                fields.push(format!("logging: (enabled: {enabled}),"));
            }
            append_extra(&mut fields, &node.extra_fields);
            render_struct(&fields, 2)
        })
        .collect();
    render_list(&tasks, 1)
}

fn render_bridges(document: &GraphDocument) -> String {
    let bridges: Vec<_> = document
        .nodes
        .iter()
        .filter(|node| node.kind == ComponentKind::Bridge)
        .map(|node| {
            let mut fields = vec![
                format!("id: {},", ron_string(&node.id)),
                format!("type: {},", ron_string(&node.component_type)),
            ];
            if !node.config.0.is_empty() {
                fields.push(format!("config: {},", render_config(&node.config)));
            }
            let channels: Vec<_> = node
                .channels
                .iter()
                .filter(|channel| channel.enabled)
                .map(|channel| {
                    let variant = if channel.direction == PortDirection::Output {
                        "Rx"
                    } else {
                        "Tx"
                    };
                    let mut channel_fields = vec![format!("id: {},", ron_string(&channel.id))];
                    if let Some(route) = &channel.route {
                        channel_fields.push(format!("route: {},", ron_string(route)));
                    }
                    if !channel.config.0.is_empty() {
                        channel_fields.push(format!("config: {},", render_config(&channel.config)));
                    }
                    format!("{variant}{}", render_struct(&channel_fields, 3))
                })
                .collect();
            fields.push(format!("channels: {},", render_list(&channels, 2)));
            if let Some(run_in_sim) = node.run_in_sim {
                fields.push(format!("run_in_sim: {run_in_sim},"));
            }
            append_extra(&mut fields, &node.extra_fields);
            render_struct(&fields, 2)
        })
        .collect();
    render_list(&bridges, 1)
}

fn render_connections(document: &GraphDocument) -> String {
    let mut connections = Vec::new();
    for edge in &document.edges {
        let Some(source) = document.nodes.get(edge.source) else { continue };
        let Some(target) = document.nodes.get(edge.target) else { continue };
        let src = endpoint_for(source, PortDirection::Output, edge.source_port);
        let dst = endpoint_for(target, PortDirection::Input, edge.target_port);
        let mut fields = vec![
            format!("src: {},", ron_string(&src)),
            format!("dst: {},", ron_string(&dst)),
            format!("msg: {},", ron_string(&edge.message_type)),
        ];
        append_extra(&mut fields, &edge.extra_fields);
        connections.push(render_struct(&fields, 2));
    }
    for (node_index, node) in document.nodes.iter().enumerate() {
        if node.kind == ComponentKind::Bridge {
            continue;
        }
        for output_index in 0..node.outputs.len() {
            if document
                .edges
                .iter()
                .any(|edge| edge.source == node_index && edge.source_port == output_index)
            {
                continue;
            }
            let output = &node.outputs[output_index];
            connections.push(render_struct(
                &[
                    format!("src: {},", ron_string(&node.id)),
                    format!("dst: {},", ron_string(NC_ENDPOINT)),
                    format!("msg: {},", ron_string(&output.serialized_type)),
                ],
                2,
            ));
        }
    }
    render_list(&connections, 1)
}

fn endpoint_for(node: &GraphNode, direction: PortDirection, port: usize) -> String {
    if node.kind != ComponentKind::Bridge {
        return node.id.clone();
    }
    let ports = if direction == PortDirection::Input {
        &node.inputs
    } else {
        &node.outputs
    };
    let channel = ports.get(port).map(|port| port.name.as_str()).unwrap_or("unknown");
    format!("{}/{channel}", node.id)
}

fn render_constants(document: &GraphDocument) -> String {
    let values: Vec<_> = document
        .constants
        .iter()
        .map(|constant| match constant {
            ConstantDefinition::Numeric {
                id,
                module,
                storage,
                quantity,
                unit,
                value_ron,
            } => {
                let mut fields = vec![format!("id: {},", ron_string(id))];
                if !module.is_empty() {
                    fields.push(format!("module: {},", ron_string(module)));
                }
                fields.push(format!("storage: {},", storage.ron_name()));
                if let Some(quantity) = quantity.as_ref().filter(|value| !value.trim().is_empty()) {
                    fields.push(format!("quantity: {quantity},"));
                }
                if let Some(unit) = unit.as_ref().filter(|value| !value.trim().is_empty()) {
                    fields.push(format!("unit: {unit},"));
                }
                fields.push(format!("value: {value_ron},"));
                render_struct(&fields, 2)
            }
            ConstantDefinition::Expression {
                id,
                module,
                rust_type,
                expression,
            } => {
                let mut fields = vec![format!("id: {},", ron_string(id))];
                if !module.is_empty() {
                    fields.push(format!("module: {},", ron_string(module)));
                }
                fields.push(format!("type: {},", ron_string(rust_type)));
                fields.push(format!("expression: {},", ron_string(expression)));
                render_struct(&fields, 2)
            }
        })
        .collect();
    render_list(&values, 1)
}

fn render_monitors(document: &GraphDocument) -> String {
    render_list(
        &document.monitors.iter().map(render_monitor).collect::<Vec<_>>(),
        1,
    )
}

fn render_monitor(monitor: &MonitorInstance) -> String {
    let mut fields = vec![format!("type: {},", ron_string(&monitor.component_type))];
    if !monitor.config.0.is_empty() {
        fields.push(format!("config: {},", render_config(&monitor.config)));
    }
    render_struct(&fields, 2)
}

fn render_config(config: &EditableConfig) -> String {
    if config.0.is_empty() {
        return "{}".into();
    }
    let fields = config
        .0
        .iter()
        .map(|(key, value)| format!("{}: {value},", ron_string(key)))
        .collect::<Vec<_>>()
        .join(" ");
    format!("{{ {fields} }}")
}

fn append_extra(fields: &mut Vec<String>, extra: &BTreeMap<String, String>) {
    for (key, value) in extra {
        if !matches!(
            key.as_str(),
            "id" | "type" | "kind" | "config" | "background" | "run_in_sim" | "logging" | "channels"
        ) {
            fields.push(format!("{key}: {value},"));
        }
    }
}

fn render_struct(fields: &[String], indent: usize) -> String {
    let padding = "    ".repeat(indent);
    let closing = "    ".repeat(indent.saturating_sub(1));
    format!("(\n{padding}{}\n{closing})", fields.join(&format!("\n{padding}")))
}

fn render_list(values: &[String], indent: usize) -> String {
    if values.is_empty() {
        return "[]".into();
    }
    let padding = "    ".repeat(indent + 1);
    let closing = "    ".repeat(indent);
    format!("[\n{padding}{},\n{closing}]", values.join(&format!(",\n{padding}")))
}

fn ron_string(value: &str) -> String {
    ron::ser::to_string(value).unwrap_or_else(|_| format!("\"{value}\""))
}

fn should_insert(name: &str, document: &GraphDocument) -> bool {
    match name {
        "tasks" | "cnx" => true,
        "bridges" => document.nodes.iter().any(|node| node.kind == ComponentKind::Bridge),
        "constants" => !document.constants.is_empty(),
        "monitor" | "monitors" => !document.monitors.is_empty(),
        _ => false,
    }
}

fn hash(source: &str) -> [u8; 32] {
    Sha256::digest(source.as_bytes()).into()
}

fn insert_root_field(source: &str, name: &str, value: &str) -> Result<String, ConfigError> {
    let Some(close) = find_root_close(source) else {
        return Err(ConfigError::Parse("configuration root is not a tuple".into()));
    };
    let mut result = source.to_string();
    let insertion = format!("    {name}: {value},\n");
    result.insert_str(close, &insertion);
    Ok(result)
}

fn find_root_close(source: &str) -> Option<usize> {
    source.rfind(')')
}

fn find_field_value_span(source: &str, field: &str) -> Option<(usize, usize)> {
    let bytes = source.as_bytes();
    let mut index = 0usize;
    let mut depth = 0usize;
    let mut state = ScanState::Normal;
    while index < bytes.len() {
        if advance_state(bytes, &mut index, &mut state) {
            continue;
        }
        match bytes[index] {
            b'(' | b'[' | b'{' => {
                depth += 1;
                index += 1;
            }
            b')' | b']' | b'}' => {
                depth = depth.saturating_sub(1);
                index += 1;
            }
            byte if depth == 1 && (byte == b'_' || byte.is_ascii_alphabetic()) => {
                let start = index;
                index += 1;
                while index < bytes.len()
                    && (bytes[index] == b'_' || bytes[index].is_ascii_alphanumeric())
                {
                    index += 1;
                }
                let name = &source[start..index];
                let mut cursor = skip_trivia(source, index);
                if name == field && bytes.get(cursor) == Some(&b':') {
                    cursor += 1;
                    let value_start = skip_trivia(source, cursor);
                    let value_end = scan_value_end(source, value_start);
                    return Some((value_start, value_end));
                }
            }
            _ => index += 1,
        }
    }
    None
}

fn scan_value_end(source: &str, start: usize) -> usize {
    let bytes = source.as_bytes();
    let mut index = start;
    let mut nested = 0usize;
    let mut state = ScanState::Normal;
    while index < bytes.len() {
        if advance_state(bytes, &mut index, &mut state) {
            continue;
        }
        match bytes[index] {
            b'(' | b'[' | b'{' => {
                nested += 1;
                index += 1;
            }
            b')' | b']' | b'}' if nested > 0 => {
                nested -= 1;
                index += 1;
            }
            b',' | b')' if nested == 0 => return index,
            _ => index += 1,
        }
    }
    index
}

fn split_collection_items(source: &str, open: char, close: char) -> Vec<&str> {
    let Some(open_index) = source.find(open) else { return vec![] };
    let Some(close_index) = source.rfind(close) else { return vec![] };
    let body = &source[open_index + open.len_utf8()..close_index];
    split_top_level(body)
}

fn split_top_level(source: &str) -> Vec<&str> {
    let bytes = source.as_bytes();
    let mut items = Vec::new();
    let mut index = 0usize;
    let mut start = 0usize;
    let mut depth = 0usize;
    let mut state = ScanState::Normal;
    while index < bytes.len() {
        if advance_state(bytes, &mut index, &mut state) {
            continue;
        }
        match bytes[index] {
            b'(' | b'[' | b'{' => {
                depth += 1;
                index += 1;
            }
            b')' | b']' | b'}' => {
                depth = depth.saturating_sub(1);
                index += 1;
            }
            b',' if depth == 0 => {
                let value = source[start..index].trim();
                if !value.is_empty() {
                    items.push(value);
                }
                start = index + 1;
                index += 1;
            }
            _ => index += 1,
        }
    }
    let value = source[start..].trim();
    if !value.is_empty() {
        items.push(value);
    }
    items
}

fn parse_struct_fields(source: &str) -> BTreeMap<String, String> {
    let body = source
        .trim()
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
        .unwrap_or(source);
    split_top_level(body)
        .into_iter()
        .filter_map(|field| {
            let colon = field.find(':')?;
            Some((
                field[..colon].trim().to_string(),
                field[colon + 1..].trim().to_string(),
            ))
        })
        .collect()
}

fn skip_trivia(source: &str, mut index: usize) -> usize {
    let bytes = source.as_bytes();
    loop {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if bytes.get(index..index + 2) == Some(b"//") {
            index += 2;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
        } else if bytes.get(index..index + 2) == Some(b"/*") {
            index += 2;
            while index + 1 < bytes.len() && &bytes[index..index + 2] != b"*/" {
                index += 1;
            }
            index = (index + 2).min(bytes.len());
        } else {
            return index;
        }
    }
}

#[derive(Clone, Copy)]
enum ScanState {
    Normal,
    String(bool),
    Character(bool),
    LineComment,
    BlockComment(usize),
}

fn advance_state(bytes: &[u8], index: &mut usize, state: &mut ScanState) -> bool {
    match *state {
        ScanState::Normal => {
            if bytes.get(*index..*index + 2) == Some(b"//") {
                *state = ScanState::LineComment;
                *index += 2;
                true
            } else if bytes.get(*index..*index + 2) == Some(b"/*") {
                *state = ScanState::BlockComment(1);
                *index += 2;
                true
            } else if bytes[*index] == b'"' {
                *state = ScanState::String(false);
                *index += 1;
                true
            } else if bytes[*index] == b'\'' {
                *state = ScanState::Character(false);
                *index += 1;
                true
            } else {
                false
            }
        }
        ScanState::String(escaped) => {
            let byte = bytes[*index];
            *index += 1;
            *state = if escaped {
                ScanState::String(false)
            } else if byte == b'\\' {
                ScanState::String(true)
            } else if byte == b'"' {
                ScanState::Normal
            } else {
                ScanState::String(false)
            };
            true
        }
        ScanState::Character(escaped) => {
            let byte = bytes[*index];
            *index += 1;
            *state = if escaped {
                ScanState::Character(false)
            } else if byte == b'\\' {
                ScanState::Character(true)
            } else if byte == b'\'' {
                ScanState::Normal
            } else {
                ScanState::Character(false)
            };
            true
        }
        ScanState::LineComment => {
            if bytes[*index] == b'\n' {
                *state = ScanState::Normal;
            }
            *index += 1;
            true
        }
        ScanState::BlockComment(depth) => {
            if bytes.get(*index..*index + 2) == Some(b"/*") {
                *state = ScanState::BlockComment(depth + 1);
                *index += 2;
            } else if bytes.get(*index..*index + 2) == Some(b"*/") {
                *state = if depth == 1 {
                    ScanState::Normal
                } else {
                    ScanState::BlockComment(depth - 1)
                };
                *index += 2;
            } else {
                *index += 1;
            }
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_root_fields_without_matching_nested_config() {
        let source = r#"(
            tasks: [(id: "a", config: {"tasks": [1, 2]})],
            cnx: [],
        )"#;
        let (start, end) = find_field_value_span(source, "tasks").unwrap();
        assert!(source[start..end].starts_with("[(id:"));
        assert!(find_field_value_span(source, "cnx").is_some());
    }

    #[test]
    fn parses_numeric_and_expression_constants() {
        let source = r#"(
            constants: [
                (id: "COUNT", storage: usize, value: 2),
                (id: "POSE", module: "geo", type: "Pose", expression: "Pose::new()"),
            ], tasks: [], cnx: [],
        )"#;
        let constants = parse_constants(source);
        assert_eq!(constants.len(), 2);
        assert_eq!(constants[1].qualified_id(), "geo::POSE");
    }

    #[test]
    fn unchanged_render_is_lossless_and_constant_edit_preserves_other_sections() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("copperconfig.ron");
        let source = r#"(
    // task documentation must survive unrelated edits
    tasks: [],
    cnx: [],
    missions: [(id: "drive")], // unsupported but preserved
)
"#;
        fs::write(&path, source).unwrap();
        let mut config = ConfigFile::load(&path, &ComponentCatalog::default()).unwrap();
        assert_eq!(config.rendered().unwrap(), source);
        config.document.constants.push(ConstantDefinition::Expression {
            id: "COUNT".into(),
            module: "constants".into(),
            rust_type: "usize".into(),
            expression: "2 + 2".into(),
        });
        let rendered = config.rendered().unwrap();
        assert!(rendered.contains("task documentation must survive unrelated edits"));
        assert!(rendered.contains("missions: [(id: \"drive\")]"));
        assert!(rendered.contains("expression: \"2 + 2\""));
    }

    #[test]
    fn node_edit_preserves_unknown_node_fields() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("copperconfig.ron");
        fs::write(
            &path,
            r#"(
                tasks: [(id: "source", type: "Source", priority: 7)],
                cnx: [],
            )"#,
        )
        .unwrap();
        let mut config = ConfigFile::load(&path, &ComponentCatalog::default()).unwrap();
        config.document.nodes[0].run_in_sim = Some(false);
        let rendered = config.rendered().unwrap();
        assert!(rendered.contains("priority: 7"));
    }

    #[test]
    fn detects_an_external_edit_before_save() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("copperconfig.ron");
        fs::write(&path, "(tasks: [], cnx: [])").unwrap();
        let mut config = ConfigFile::load(&path, &ComponentCatalog::default()).unwrap();
        fs::write(&path, "(tasks: [], cnx: [], /* external */)").unwrap();
        assert!(matches!(config.save(false), Err(ConfigError::ExternalChange)));
    }
}
