use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};

pub const NC_ENDPOINT: &str = "__nc__";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ComponentKind {
    Source,
    Task,
    Sink,
    Bridge,
    Monitor,
    Import,
    Unresolved,
}

impl ComponentKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Source => "Source",
            Self::Task => "Task",
            Self::Sink => "Sink",
            Self::Bridge => "Bridge",
            Self::Monitor => "Monitor",
            Self::Import => "Import",
            Self::Unresolved => "Unresolved",
        }
    }

    pub fn ron_task_kind(self) -> Option<&'static str> {
        match self {
            Self::Source => Some("source"),
            Self::Task => Some("task"),
            Self::Sink => Some("sink"),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PortDirection {
    Input,
    Output,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortDescriptor {
    pub name: String,
    pub direction: PortDirection,
    pub ordinal: usize,
    pub declared_type: String,
    pub serialized_type: String,
    pub canonical_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeChannelDescriptor {
    pub id: String,
    pub direction: PortDirection,
    pub payload: PortDescriptor,
    pub default_route: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigHint {
    pub key: String,
    pub rust_type: Option<String>,
    pub default_ron: Option<String>,
    pub documentation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentDescriptor {
    pub type_path: String,
    pub display_name: String,
    pub package: String,
    pub kind: ComponentKind,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub channels: Vec<BridgeChannelDescriptor>,
    pub config_hints: Vec<ConfigHint>,
    pub workspace_only: bool,
    pub source_path: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditableConfig(pub BTreeMap<String, String>);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeChannelInstance {
    pub id: String,
    pub direction: PortDirection,
    pub route: Option<String>,
    pub config: EditableConfig,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub component_type: String,
    pub kind: ComponentKind,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub config: EditableConfig,
    pub channels: Vec<BridgeChannelInstance>,
    pub background: Option<bool>,
    pub run_in_sim: Option<bool>,
    pub logging_enabled: Option<bool>,
    pub missions: Vec<String>,
    pub pending_workspace_dependency: Option<String>,
    pub unresolved: bool,
    pub extra_fields: BTreeMap<String, String>,
}

impl GraphNode {
    pub fn from_component(component: &ComponentDescriptor, id: String) -> Self {
        Self {
            id,
            component_type: component.type_path.clone(),
            kind: component.kind,
            inputs: component.inputs.clone(),
            outputs: component.outputs.clone(),
            config: EditableConfig::default(),
            channels: component
                .channels
                .iter()
                .map(|channel| BridgeChannelInstance {
                    id: channel.id.clone(),
                    direction: channel.direction,
                    route: channel.default_route.clone(),
                    config: EditableConfig::default(),
                    enabled: true,
                })
                .collect(),
            background: None,
            run_in_sim: None,
            logging_enabled: None,
            missions: Vec::new(),
            pending_workspace_dependency: component
                .workspace_only
                .then(|| component.package.clone()),
            unresolved: false,
            extra_fields: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphEdge {
    pub source: usize,
    pub source_port: usize,
    pub target: usize,
    pub target_port: usize,
    pub message_type: String,
    pub missions: Vec<String>,
    pub extra_fields: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnconnectedOutput {
    pub source: usize,
    pub source_port: usize,
    pub message_type: String,
    pub missions: Vec<String>,
    pub extra_fields: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorInstance {
    pub component_type: String,
    pub config: EditableConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NumericStorage {
    I8,
    I16,
    I32,
    I64,
    Isize,
    U8,
    U16,
    U32,
    U64,
    Usize,
    F32,
    F64,
}

impl NumericStorage {
    pub const ALL: [Self; 12] = [
        Self::I8,
        Self::I16,
        Self::I32,
        Self::I64,
        Self::Isize,
        Self::U8,
        Self::U16,
        Self::U32,
        Self::U64,
        Self::Usize,
        Self::F32,
        Self::F64,
    ];

    pub fn ron_name(self) -> &'static str {
        match self {
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::Isize => "isize",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::Usize => "usize",
            Self::F32 => "f32",
            Self::F64 => "f64",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConstantDefinition {
    Numeric {
        id: String,
        module: String,
        storage: NumericStorage,
        quantity: Option<String>,
        unit: Option<String>,
        value_ron: String,
    },
    Expression {
        id: String,
        module: String,
        rust_type: String,
        expression: String,
    },
}

impl ConstantDefinition {
    pub fn id(&self) -> &str {
        match self {
            Self::Numeric { id, .. } | Self::Expression { id, .. } => id,
        }
    }

    pub fn module(&self) -> &str {
        match self {
            Self::Numeric { module, .. } | Self::Expression { module, .. } => module,
        }
    }

    pub fn qualified_id(&self) -> String {
        let module = self.module().trim();
        if module.is_empty() {
            format!("constants::{}", self.id())
        } else {
            format!("{module}::{}", self.id())
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GraphDocument {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub unconnected_outputs: Vec<UnconnectedOutput>,
    pub monitors: Vec<MonitorInstance>,
    pub constants: Vec<ConstantDefinition>,
    pub missions: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub message: String,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            message: message.into(),
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Warning,
            message: message.into(),
        }
    }
}

impl GraphDocument {
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let mut ids: HashMap<&str, Vec<&[String]>> = HashMap::new();
        for node in &self.nodes {
            if node.id.is_empty() {
                diagnostics.push(Diagnostic::error("Node IDs cannot be empty"));
            } else if node.kind != ComponentKind::Import
                && (node.id == NC_ENDPOINT || node.id.contains('/'))
            {
                diagnostics.push(Diagnostic::error(format!(
                    "Node '{}' uses a reserved endpoint character or name",
                    node.id
                )));
            } else {
                let duplicates = ids.entry(node.id.as_str()).or_default();
                if duplicates
                    .iter()
                    .any(|missions| missions_overlap(missions, &node.missions))
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "Node ID '{}' is duplicated in the same mission",
                        node.id
                    )));
                }
                duplicates.push(&node.missions);
            }
            if node.unresolved {
                diagnostics.push(Diagnostic::warning(format!(
                    "Component type '{}' could not be resolved and will be preserved",
                    node.component_type
                )));
            }
            validate_config_values(&node.id, &node.config, &mut diagnostics);
            for channel in &node.channels {
                validate_config_values(
                    &format!("{}/{}", node.id, channel.id),
                    &channel.config,
                    &mut diagnostics,
                );
            }
        }

        let mut mission_ids = HashSet::new();
        for mission in &self.missions {
            if mission.is_empty() {
                diagnostics.push(Diagnostic::error("Mission IDs cannot be empty"));
            } else if !mission_ids.insert(mission) {
                diagnostics.push(Diagnostic::error(format!(
                    "Mission '{}' is duplicated",
                    mission
                )));
            }
        }

        let mut connected_inputs = HashSet::new();
        for edge in &self.edges {
            let Some(source) = self.nodes.get(edge.source) else {
                diagnostics.push(Diagnostic::error("Connection has a missing source node"));
                continue;
            };
            let Some(target) = self.nodes.get(edge.target) else {
                diagnostics.push(Diagnostic::error("Connection has a missing target node"));
                continue;
            };
            let Some(output) = source.outputs.get(edge.source_port) else {
                diagnostics.push(Diagnostic::error(format!(
                    "Connection from '{}' uses a missing output port",
                    source.id
                )));
                continue;
            };
            let Some(input) = target.inputs.get(edge.target_port) else {
                diagnostics.push(Diagnostic::error(format!(
                    "Connection to '{}' uses a missing input port",
                    target.id
                )));
                continue;
            };
            if !bridge_port_enabled(source, output) {
                diagnostics.push(Diagnostic::error(format!(
                    "Connection uses disabled bridge channel '{}/{}'",
                    source.id, output.name
                )));
            }
            if !bridge_port_enabled(target, input) {
                diagnostics.push(Diagnostic::error(format!(
                    "Connection uses disabled bridge channel '{}/{}'",
                    target.id, input.name
                )));
            }
            let output_type = normalize_type(&output.canonical_type);
            let input_type = normalize_type(&input.canonical_type);
            if output_type != input_type
                && !matches!(output_type.as_str(), "" | "_unresolved")
                && !matches!(input_type.as_str(), "" | "_unresolved")
            {
                diagnostics.push(Diagnostic::warning(format!(
                    "Connection '{}.{}' -> '{}.{}' has mismatched types: '{}' -> '{}'",
                    source.id,
                    output.name,
                    target.id,
                    input.name,
                    output.declared_type,
                    input.declared_type,
                )));
            }
            if self.edges.iter().any(|other| {
                !std::ptr::eq(other, edge)
                    && other.target == edge.target
                    && other.target_port == edge.target_port
                    && missions_overlap(&other.missions, &edge.missions)
            }) {
                diagnostics.push(Diagnostic::error(format!(
                    "Input '{}' on '{}' has more than one producer in the same mission",
                    input.name, target.id
                )));
            }
            connected_inputs.insert((edge.target, edge.target_port));
        }

        for (node_index, node) in self.nodes.iter().enumerate() {
            for (port_index, input) in node.inputs.iter().enumerate() {
                if !connected_inputs.contains(&(node_index, port_index)) {
                    diagnostics.push(Diagnostic::warning(format!(
                        "Input '{}' on '{}' is not connected",
                        input.name, node.id
                    )));
                }
            }
        }

        let mut constants = HashSet::new();
        for constant in &self.constants {
            if !is_rust_identifier(constant.id()) {
                diagnostics.push(Diagnostic::error(format!(
                    "Constant ID '{}' is not a Rust identifier",
                    constant.id()
                )));
            }
            if !constant.module().is_empty()
                && !constant.module().split("::").all(is_rust_identifier)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "Constant module '{}' is not a Rust module path",
                    constant.module()
                )));
            }
            if !constants.insert(constant.qualified_id()) {
                diagnostics.push(Diagnostic::error(format!(
                    "Constant '{}' is duplicated",
                    constant.qualified_id()
                )));
            }
            if let ConstantDefinition::Numeric { value_ron, .. } = constant
                && ron::from_str::<ron::Value>(value_ron).is_err()
            {
                diagnostics.push(Diagnostic::error(format!(
                    "Constant '{}' has an invalid RON value",
                    constant.qualified_id()
                )));
            }
            if let ConstantDefinition::Expression {
                rust_type,
                expression,
                ..
            } = constant
            {
                if syn::parse_str::<syn::Type>(rust_type).is_err() {
                    diagnostics.push(Diagnostic::error(format!(
                        "Constant '{}' has an invalid Rust type",
                        constant.qualified_id()
                    )));
                }
                if syn::parse_str::<syn::Expr>(expression).is_err() {
                    diagnostics.push(Diagnostic::error(format!(
                        "Constant '{}' has an invalid Rust expression",
                        constant.qualified_id()
                    )));
                }
            }
        }
        for monitor in &self.monitors {
            validate_config_values(&monitor.component_type, &monitor.config, &mut diagnostics);
        }
        diagnostics
    }

    pub fn has_hard_errors(&self) -> bool {
        self.diagnostics()
            .iter()
            .any(|d| d.severity == DiagnosticSeverity::Error)
    }

    pub fn add_edge(
        &mut self,
        source: usize,
        source_port: usize,
        target: usize,
        target_port: usize,
    ) -> Result<(), String> {
        let output = self
            .nodes
            .get(source)
            .and_then(|node| node.outputs.get(source_port))
            .ok_or_else(|| "Missing output port".to_string())?;
        let _input = self
            .nodes
            .get(target)
            .and_then(|node| node.inputs.get(target_port))
            .ok_or_else(|| "Missing input port".to_string())?;
        if self
            .edges
            .iter()
            .any(|edge| edge.target == target && edge.target_port == target_port)
        {
            return Err("That input already has a producer".to_string());
        }
        let candidate = GraphEdge {
            source,
            source_port,
            target,
            target_port,
            message_type: output.serialized_type.clone(),
            missions: Vec::new(),
            extra_fields: BTreeMap::new(),
        };
        self.edges.push(candidate);
        Ok(())
    }

    pub fn unique_id_for(&self, display_name: &str) -> String {
        let base = to_snake_case(display_name);
        let used: HashMap<_, _> = self.nodes.iter().map(|node| (&node.id, ())).collect();
        if !used.contains_key(&base) {
            return base;
        }
        for suffix in 2.. {
            let candidate = format!("{base}_{suffix}");
            if !used.contains_key(&candidate) {
                return candidate;
            }
        }
        unreachable!()
    }
}

fn missions_overlap(left: &[String], right: &[String]) -> bool {
    left.is_empty()
        || right.is_empty()
        || left.iter().any(|mission| right.contains(mission))
}

fn validate_config_values(owner: &str, config: &EditableConfig, diagnostics: &mut Vec<Diagnostic>) {
    for (key, value) in &config.0 {
        if ron::from_str::<ron::Value>(value).is_err() {
            diagnostics.push(Diagnostic::error(format!(
                "Config value '{key}' on '{owner}' is not valid RON"
            )));
        }
    }
}

fn bridge_port_enabled(node: &GraphNode, port: &PortDescriptor) -> bool {
    node.kind != ComponentKind::Bridge
        || node
            .channels
            .iter()
            .find(|channel| channel.id == port.name && channel.direction == port.direction)
            .is_none_or(|channel| channel.enabled)
}

pub fn normalize_type(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_whitespace() && *character != '&')
        .collect::<String>()
        .replace("'m,", "")
        .replace("'_,", "")
        .replace("crate::", "")
}

pub fn is_rust_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(first) if first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn to_snake_case(value: &str) -> String {
    let mut out = String::new();
    for (index, character) in value.chars().enumerate() {
        if character.is_ascii_uppercase() {
            if index > 0 && !out.ends_with('_') {
                out.push('_');
            }
            out.push(character.to_ascii_lowercase());
        } else if character.is_ascii_alphanumeric() || character == '_' {
            out.push(character.to_ascii_lowercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_stable_and_unique() {
        let mut document = GraphDocument::default();
        assert_eq!(document.unique_id_for("My Task"), "my_task");
        document.nodes.push(GraphNode {
            id: "my_task".into(),
            component_type: "x::Task".into(),
            kind: ComponentKind::Task,
            inputs: vec![],
            outputs: vec![],
            config: EditableConfig::default(),
            channels: vec![],
            background: None,
            run_in_sim: None,
            logging_enabled: None,
            missions: Vec::new(),
            pending_workspace_dependency: None,
            unresolved: false,
            extra_fields: BTreeMap::new(),
        });
        assert_eq!(document.unique_id_for("My Task"), "my_task_2");
    }

    #[test]
    fn invalid_free_form_values_and_expressions_are_hard_errors() {
        let mut document = GraphDocument::default();
        document.monitors.push(MonitorInstance {
            component_type: "Monitor".into(),
            config: EditableConfig(BTreeMap::from([("bad".into(), "[".into())])),
        });
        document.constants.push(ConstantDefinition::Expression {
            id: "POSE".into(),
            module: "constants".into(),
            rust_type: "not a type".into(),
            expression: "not valid +".into(),
        });
        assert!(document.has_hard_errors());
    }

    #[test]
    fn mismatched_connection_types_are_advisory_warnings() {
        let mut document = GraphDocument {
            nodes: vec![
                GraphNode {
                    id: "source".into(),
                    component_type: "Source".into(),
                    kind: ComponentKind::Source,
                    inputs: vec![],
                    outputs: vec![PortDescriptor {
                        name: "out".into(),
                        direction: PortDirection::Output,
                        ordinal: 0,
                        declared_type: "Angle".into(),
                        serialized_type: "Angle".into(),
                        canonical_type: "Angle".into(),
                    }],
                    config: EditableConfig::default(),
                    channels: vec![],
                    background: None,
                    run_in_sim: None,
                    logging_enabled: None,
                    missions: Vec::new(),
                    pending_workspace_dependency: None,
                    unresolved: false,
                    extra_fields: BTreeMap::new(),
                },
                GraphNode {
                    id: "target".into(),
                    component_type: "Target".into(),
                    kind: ComponentKind::Sink,
                    inputs: vec![PortDescriptor {
                        name: "in".into(),
                        direction: PortDirection::Input,
                        ordinal: 0,
                        declared_type: "Velocity".into(),
                        serialized_type: "Velocity".into(),
                        canonical_type: "Velocity".into(),
                    }],
                    outputs: vec![],
                    config: EditableConfig::default(),
                    channels: vec![],
                    background: None,
                    run_in_sim: None,
                    logging_enabled: None,
                    missions: Vec::new(),
                    pending_workspace_dependency: None,
                    unresolved: false,
                    extra_fields: BTreeMap::new(),
                },
            ],
            edges: vec![GraphEdge {
                source: 0,
                source_port: 0,
                target: 1,
                target_port: 0,
                message_type: "Angle".into(),
                missions: Vec::new(),
                extra_fields: BTreeMap::new(),
            }],
            ..GraphDocument::default()
        };

        let diagnostics = document.diagnostics();
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.severity == DiagnosticSeverity::Warning
                && diagnostic.message.contains("mismatched types")
        }));
        assert!(!document.has_hard_errors());
    }
}
