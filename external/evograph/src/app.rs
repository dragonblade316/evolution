use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use eframe::egui;
use egui_snarl::{InPin, InPinId, NodeId, OutPin, OutPinId, Snarl};
use egui_snarl::ui::{PinInfo, SnarlStyle, SnarlViewer, SnarlWidget};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};

use crate::config::{ConfigError, ConfigFile};
use crate::indexer::ComponentCatalog;
use crate::model::{
    ComponentDescriptor, ComponentKind, ConstantDefinition, DiagnosticSeverity, EditableConfig,
    GraphDocument, GraphNode, MonitorInstance, NumericStorage, PortDescriptor, PortDirection,
};
use crate::project::{ProjectInfo, atomic_write};

type EditorState = Snarl<UiNode>;

#[derive(Debug, Clone)]
struct UiNode {
    component_type: String,
    model_id: String,
    kind: ComponentKind,
    side: BridgeSide,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
}

impl UiNode {
    fn from_component(
        component: &ComponentDescriptor,
        component_type: String,
        model_id: String,
        side: BridgeSide,
    ) -> Self {
        Self {
            component_type,
            model_id,
            kind: component.kind,
            side,
            inputs: if side == BridgeSide::Receive {
                Vec::new()
            } else {
                component.inputs.clone()
            },
            outputs: if side == BridgeSide::Transmit {
                Vec::new()
            } else {
                component.outputs.clone()
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BridgeSide {
    Regular,
    Receive,
    Transmit,
}

impl BridgeSide {
    fn label(self) -> Option<&'static str> {
        match self {
            Self::Regular => None,
            Self::Receive => Some("RX · source side"),
            Self::Transmit => Some("TX · sink side"),
        }
    }
}

#[derive(Clone, Copy, Default)]
struct VisualNodeIds {
    regular: Option<NodeId>,
    receive: Option<NodeId>,
    transmit: Option<NodeId>,
}

const LAYOUT_FILE_NAME: &str = ".evonode";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct EvoNodeLayout {
    #[serde(default = "layout_version")]
    version: u32,
    #[serde(default)]
    positions: BTreeMap<String, NodePosition>,
    #[serde(default)]
    camera: Option<CameraState>,
}

fn layout_version() -> u32 {
    2
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct NodePosition {
    x: f32,
    y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct CameraState {
    scale: f32,
    x: f32,
    y: f32,
}

impl Default for CameraState {
    fn default() -> Self {
        Self {
            scale: 1.0,
            x: 0.0,
            y: 0.0,
        }
    }
}

impl CameraState {
    fn transform(self) -> egui::emath::TSTransform {
        egui::emath::TSTransform::new(egui::vec2(self.x, self.y), self.scale)
    }

    fn from_transform(transform: egui::emath::TSTransform) -> Self {
        Self {
            scale: transform.scaling,
            x: transform.translation.x,
            y: transform.translation.y,
        }
    }

    fn approximately_eq(self, other: Self) -> bool {
        (self.scale - other.scale).abs() < 0.0001
            && (self.x - other.x).abs() < 0.05
            && (self.y - other.y).abs() < 0.05
    }
}

enum GraphAction {
    Add(ComponentDescriptor, egui::Pos2),
    Delete(NodeId),
}

struct GraphViewer<'a> {
    document: &'a mut GraphDocument,
    catalog: &'a ComponentCatalog,
    status: &'a mut String,
    dirty: &'a mut bool,
    actions: &'a mut Vec<GraphAction>,
    menu_query: &'a mut String,
    camera: &'a mut CameraState,
    restore_camera: &'a mut bool,
    camera_changed: &'a mut bool,
}

#[allow(refining_impl_trait)]
impl SnarlViewer<UiNode> for GraphViewer<'_> {
    fn title(&mut self, node: &UiNode) -> String {
        visual_label(&node.model_id, node.side)
    }

    fn inputs(&mut self, node: &UiNode) -> usize {
        node.inputs.len()
    }

    fn outputs(&mut self, node: &UiNode) -> usize {
        node.outputs.len()
    }

    fn show_input(
        &mut self,
        pin: &InPin,
        ui: &mut egui::Ui,
        snarl: &mut Snarl<UiNode>,
    ) -> PinInfo {
        let port = &snarl[pin.id.node].inputs[pin.id.input];
        ui.label(&port.name).on_hover_text(&port.declared_type);
        PinInfo::circle().with_fill(port_color(&port.canonical_type))
    }

    fn show_output(
        &mut self,
        pin: &OutPin,
        ui: &mut egui::Ui,
        snarl: &mut Snarl<UiNode>,
    ) -> PinInfo {
        let port = &snarl[pin.id.node].outputs[pin.id.output];
        ui.label(&port.name).on_hover_text(&port.declared_type);
        PinInfo::circle().with_fill(port_color(&port.canonical_type))
    }

    fn has_footer(&mut self, _node: &UiNode) -> bool {
        true
    }

    fn show_footer(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut egui::Ui,
        snarl: &mut Snarl<UiNode>,
    ) {
        let node = &snarl[node];
        let side = node.side.label().unwrap_or(node.kind.label());
        ui.label(egui::RichText::new(format!("{side} · {}", node.component_type)).small());
    }

    fn header_frame(
        &mut self,
        default: egui::Frame,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        snarl: &Snarl<UiNode>,
    ) -> egui::Frame {
        default.fill(node_color(&snarl[node]))
    }

    fn connect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<UiNode>) {
        let source_id = snarl[from.id.node].model_id.clone();
        let target_id = snarl[to.id.node].model_id.clone();
        let Some(source) = self.document.nodes.iter().position(|node| node.id == source_id) else {
            return;
        };
        let Some(target) = self.document.nodes.iter().position(|node| node.id == target_id) else {
            return;
        };
        let previous = self
            .document
            .edges
            .iter()
            .find(|edge| edge.target == target && edge.target_port == to.id.input)
            .cloned();
        self.document
            .edges
            .retain(|edge| !(edge.target == target && edge.target_port == to.id.input));
        match self
            .document
            .add_edge(source, from.id.output, target, to.id.input)
        {
            Ok(()) => {
                for remote in to.remotes.iter().copied() {
                    snarl.disconnect(remote, to.id);
                }
                snarl.connect(from.id, to.id);
                *self.dirty = true;
            }
            Err(error) => {
                if let Some(previous) = previous {
                    self.document.edges.push(previous);
                }
                *self.status = error;
            }
        }
    }

    fn disconnect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<UiNode>) {
        remove_document_edge(self.document, snarl, from.id, to.id);
        snarl.disconnect(from.id, to.id);
        *self.dirty = true;
    }

    fn drop_outputs(&mut self, pin: &OutPin, snarl: &mut Snarl<UiNode>) {
        for remote in pin.remotes.iter().copied() {
            remove_document_edge(self.document, snarl, pin.id, remote);
        }
        snarl.drop_outputs(pin.id);
        *self.dirty = true;
    }

    fn drop_inputs(&mut self, pin: &InPin, snarl: &mut Snarl<UiNode>) {
        for remote in pin.remotes.iter().copied() {
            remove_document_edge(self.document, snarl, remote, pin.id);
        }
        snarl.drop_inputs(pin.id);
        *self.dirty = true;
    }

    fn has_graph_menu(&mut self, _pos: egui::Pos2, _snarl: &mut Snarl<UiNode>) -> bool {
        true
    }

    fn show_graph_menu(
        &mut self,
        pos: egui::Pos2,
        ui: &mut egui::Ui,
        _snarl: &mut Snarl<UiNode>,
    ) {
        ui.label("Add component");
        ui.text_edit_singleline(self.menu_query).request_focus();
        let query = self.menu_query.trim().to_lowercase();
        egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
            for component in self
                .catalog
                .components
                .iter()
                .filter(|component| component.kind != ComponentKind::Monitor)
                .filter(|component| {
                    query.is_empty()
                        || component.display_name.to_lowercase().contains(&query)
                        || component.type_path.to_lowercase().contains(&query)
                        || component.package.to_lowercase().contains(&query)
                })
            {
                ui.label(
                    egui::RichText::new(format!(
                        "{} / {}",
                        component.kind.label(),
                        component.package
                    ))
                    .small(),
                );
                if ui
                    .button(&component.display_name)
                    .on_hover_text(&component.type_path)
                    .clicked()
                {
                    self.actions.push(GraphAction::Add(component.clone(), pos));
                    self.menu_query.clear();
                    ui.close();
                }
            }
        });
    }

    fn has_node_menu(&mut self, _node: &UiNode) -> bool {
        true
    }

    fn show_node_menu(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut egui::Ui,
        _snarl: &mut Snarl<UiNode>,
    ) {
        if ui.button("Delete").clicked() {
            self.actions.push(GraphAction::Delete(node));
            ui.close();
        }
    }

    fn current_transform(
        &mut self,
        transform: &mut egui::emath::TSTransform,
        _snarl: &mut Snarl<UiNode>,
    ) {
        if *self.restore_camera {
            *transform = self.camera.transform();
            *self.restore_camera = false;
            return;
        }
        let current = CameraState::from_transform(*transform);
        if !self.camera.approximately_eq(current) {
            *self.camera = current;
            *self.camera_changed = true;
        }
    }

}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Graph,
    Constants,
    Monitors,
}

struct TransformDraft {
    id: String,
    module: String,
    rust_type: String,
    tx: f64,
    ty: f64,
    tz: f64,
    roll: f64,
    pitch: f64,
    yaw: f64,
}

impl Default for TransformDraft {
    fn default() -> Self {
        Self {
            id: "sensor_to_robot".into(),
            module: "constants".into(),
            rust_type: "cu_spatial_payloads::Transform3D<f64>".into(),
            tx: 0.0,
            ty: 0.0,
            tz: 0.0,
            roll: 0.0,
            pitch: 0.0,
            yaw: 0.0,
        }
    }
}

pub struct EvographApp {
    project: Option<ProjectInfo>,
    catalog: ComponentCatalog,
    config: Option<ConfigFile>,
    editor: EditorState,
    selected_node: Option<NodeId>,
    graph_widget_id: egui::Id,
    node_menu_query: String,
    camera: CameraState,
    restore_camera: bool,
    view: View,
    status: String,
    dirty: bool,
    overwrite_dialog: bool,
    new_config_key: String,
    transform_draft: TransformDraft,
    show_transform_wizard: bool,
    watcher: Option<RecommendedWatcher>,
    watch_rx: Option<Receiver<notify::Result<Event>>>,
    reindex_after: Option<Instant>,
    layout_path: Option<PathBuf>,
    layout: EvoNodeLayout,
    layout_save_after: Option<Instant>,
}

impl EvographApp {
    pub fn new(initial_path: Option<PathBuf>) -> Self {
        let mut app = Self {
            project: None,
            catalog: ComponentCatalog::default(),
            config: None,
            editor: EditorState::new(),
            selected_node: None,
            graph_widget_id: egui::Id::new("evograph_graph"),
            node_menu_query: String::new(),
            camera: CameraState::default(),
            restore_camera: true,
            view: View::Graph,
            status: "Open a Copper Rust application folder to begin".into(),
            dirty: false,
            overwrite_dialog: false,
            new_config_key: String::new(),
            transform_draft: TransformDraft::default(),
            show_transform_wizard: false,
            watcher: None,
            watch_rx: None,
            reindex_after: None,
            layout_path: None,
            layout: EvoNodeLayout {
                version: layout_version(),
                ..Default::default()
            },
            layout_save_after: None,
        };
        if let Some(path) = initial_path {
            app.open_project(&path);
        }
        app
    }

    fn open_project(&mut self, path: &Path) {
        match ProjectInfo::discover(path) {
            Ok(project) => {
                let catalog = ComponentCatalog::index(&project);
                match ConfigFile::load(&project.config_path, &catalog) {
                    Ok(config) => {
                        let metadata_fallback = project.metadata_warning.is_some();
                        self.layout_path = project
                            .app_manifest
                            .parent()
                            .map(|directory| directory.join(LAYOUT_FILE_NAME));
                        self.layout = self
                            .layout_path
                            .as_deref()
                            .and_then(read_layout)
                            .unwrap_or_else(|| EvoNodeLayout {
                                version: layout_version(),
                                ..Default::default()
                            });
                        self.camera = self.layout.camera.unwrap_or_default();
                        self.restore_camera = true;
                        self.graph_widget_id = egui::Id::new((
                            "evograph_graph",
                            project.app_manifest.to_string_lossy().as_ref(),
                        ));
                        self.layout_save_after = None;
                        self.project = Some(project);
                        self.catalog = catalog;
                        self.config = Some(config);
                        self.rebuild_editor();
                        self.install_watcher();
                        self.dirty = false;
                        self.status = if metadata_fallback {
                            format!(
                                "Indexed {} components using manifest fallback",
                                self.catalog.components.len()
                            )
                        } else {
                            format!(
                                "Indexed {} graph components",
                                self.catalog.components.len()
                            )
                        };
                    }
                    Err(error) => self.status = format!("Could not load config: {error}"),
                }
            }
            Err(error) => self.status = format!("Could not open project: {error}"),
        }
    }

    fn install_watcher(&mut self) {
        self.watcher = None;
        self.watch_rx = None;
        let Some(project) = &self.project else { return };
        let (tx, rx) = mpsc::channel();
        let Ok(mut watcher) = notify::recommended_watcher(move |event| {
            let _ = tx.send(event);
        }) else {
            self.status = "Project opened, but file watching could not be started".into();
            return;
        };
        let mut roots = HashSet::new();
        if let Some(root) = project.app_manifest.parent() {
            roots.insert(root.to_path_buf());
        }
        for package in &project.source_packages {
            roots.insert(package.source_root.clone());
        }
        let watched_any = roots
            .iter()
            .filter(|root| root.exists())
            .fold(false, |watched, root| {
                watcher.watch(root, RecursiveMode::Recursive).is_ok() || watched
            });
        if watched_any {
            self.watcher = Some(watcher);
            self.watch_rx = Some(rx);
        }
    }

    fn process_file_events(&mut self) {
        let mut relevant = false;
        if let Some(rx) = &self.watch_rx {
            while let Ok(event) = rx.try_recv() {
                if let Ok(event) = event
                    && event.paths.iter().any(|path| {
                        path.extension().is_some_and(|ext| ext == "rs" || ext == "toml")
                    })
                {
                    relevant = true;
                }
            }
        }
        if relevant {
            self.reindex_after = Some(Instant::now() + Duration::from_millis(500));
        }
        if self.reindex_after.is_some_and(|deadline| Instant::now() >= deadline) {
            self.reindex_after = None;
            self.reindex();
        }
    }

    fn reindex(&mut self) {
        let Some(opened_path) = self.project.as_ref().map(|project| project.opened_path.clone())
        else {
            return;
        };
        let Ok(project) = ProjectInfo::discover(&opened_path) else {
            self.status = "Could not refresh Cargo project metadata".into();
            return;
        };
        self.catalog = ComponentCatalog::index(&project);
        self.project = Some(project);
        if !self.dirty
            && let Some(path) = self.config.as_ref().map(|config| config.path.clone())
        {
            match ConfigFile::load(path, &self.catalog) {
                Ok(config) => {
                    self.config = Some(config);
                    self.rebuild_editor();
                }
                Err(error) => {
                    self.status = format!("Sources reindexed, but config reload failed: {error}");
                    return;
                }
            }
        }
        self.install_watcher();
        self.status = if self.dirty {
            format!(
                "Reindexed {} components; existing dirty nodes keep their current ports",
                self.catalog.components.len()
            )
        } else {
            format!("Reindexed {} graph components", self.catalog.components.len())
        };
    }

    fn rebuild_editor(&mut self) {
        self.editor = EditorState::new();
        self.selected_node = None;
        let Some(document) = self.config.as_ref().map(|config| config.document.clone()) else {
            return;
        };
        let mut graph_ids = Vec::with_capacity(document.nodes.len());
        for (index, node) in document.nodes.iter().enumerate() {
            let component = component_for_node(node);
            let column = (index % 4) as f32;
            let row = (index / 4) as f32;
            let base_position = egui::pos2(40.0 + column * 520.0, 50.0 + row * 190.0);
            if node.kind == ComponentKind::Bridge {
                let receive = self.add_visual_bridge_node(
                    &component,
                    &node.component_type,
                    &node.id,
                    BridgeSide::Receive,
                    base_position,
                );
                let transmit = self.add_visual_bridge_node(
                    &component,
                    &node.component_type,
                    &node.id,
                    BridgeSide::Transmit,
                    base_position + egui::vec2(250.0, 0.0),
                );
                graph_ids.push(VisualNodeIds {
                    receive: Some(receive),
                    transmit: Some(transmit),
                    ..Default::default()
                });
            } else {
                let ui_data = UiNode::from_component(
                    &component,
                    node.component_type.clone(),
                    node.id.clone(),
                    BridgeSide::Regular,
                );
                let position = self.layout_position(&ui_data, base_position);
                let graph_id = self.editor.insert_node(position, ui_data);
                graph_ids.push(VisualNodeIds {
                    regular: Some(graph_id),
                    ..Default::default()
                });
            }
        }
        for edge in &document.edges {
            let Some(source_ids) = graph_ids.get(edge.source) else { continue };
            let Some(target_ids) = graph_ids.get(edge.target) else { continue };
            let source_node = if document.nodes[edge.source].kind == ComponentKind::Bridge {
                source_ids.receive
            } else {
                source_ids.regular
            };
            let target_node = if document.nodes[edge.target].kind == ComponentKind::Bridge {
                target_ids.transmit
            } else {
                target_ids.regular
            };
            let (Some(source_node), Some(target_node)) = (source_node, target_node) else {
                continue;
            };
            if self.editor[source_node].outputs.get(edge.source_port).is_none() {
                continue;
            }
            if self.editor[target_node].inputs.get(edge.target_port).is_none() {
                continue;
            }
            self.editor.connect(
                OutPinId {
                    node: source_node,
                    output: edge.source_port,
                },
                InPinId {
                    node: target_node,
                    input: edge.target_port,
                },
            );
        }
    }

    fn add_visual_bridge_node(
        &mut self,
        component: &ComponentDescriptor,
        component_type: &str,
        model_id: &str,
        side: BridgeSide,
        position: egui::Pos2,
    ) -> NodeId {
        let ui_data = UiNode::from_component(
            component,
            component_type.to_string(),
            model_id.to_string(),
            side,
        );
        let position = self.layout_position(&ui_data, position);
        self.editor.insert_node(position, ui_data)
    }

    fn layout_position(&self, node: &UiNode, fallback: egui::Pos2) -> egui::Pos2 {
        self.layout
            .positions
            .get(&layout_key(node))
            .map(|position| egui::pos2(position.x, position.y))
            .unwrap_or(fallback)
    }

    fn mark_layout_dirty(&mut self) {
        self.layout_save_after = Some(Instant::now() + Duration::from_millis(500));
    }

    fn process_layout_save(&mut self) {
        if self
            .layout_save_after
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            self.layout_save_after = None;
            if let Err(error) = self.save_layout() {
                self.status = format!("Could not save {LAYOUT_FILE_NAME}: {error}");
            }
        }
    }

    fn save_layout(&mut self) -> Result<(), String> {
        let Some(path) = self.layout_path.clone() else {
            return Ok(());
        };
        let mut positions = BTreeMap::new();
        for (_, position, node) in self.editor.nodes_pos_ids() {
            positions.insert(
                layout_key(node),
                NodePosition {
                    x: position.x,
                    y: position.y,
                },
            );
        }
        self.layout = EvoNodeLayout {
            version: layout_version(),
            positions,
            camera: Some(self.camera),
        };
        let source = ron::ser::to_string(&self.layout).map_err(|error| error.to_string())?;
        atomic_write(&path, source.as_bytes()).map_err(|error| error.to_string())
    }

    fn selected_model_index(&self) -> Option<usize> {
        let graph_id = self.selected_node?;
        let model_id = &self.editor.get_node(graph_id)?.model_id;
        self.config
            .as_ref()?
            .document
            .nodes
            .iter()
            .position(|node| &node.id == model_id)
    }

    fn graph_positions(&self) -> BTreeMap<String, NodePosition> {
        self.editor
            .nodes_pos_ids()
            .map(|(_, position, node)| {
                (
                    layout_key(node),
                    NodePosition {
                        x: position.x,
                        y: position.y,
                    },
                )
            })
            .collect()
    }

    fn handle_graph_actions(&mut self, actions: Vec<GraphAction>) {
        for action in actions {
            match action {
                GraphAction::Add(component, position) => {
                    let Some(config) = &mut self.config else { continue };
                    let id = config.document.unique_id_for(&component.display_name);
                    config
                        .document
                        .nodes
                        .push(GraphNode::from_component(&component, id.clone()));
                    if component.kind == ComponentKind::Bridge {
                        self.editor.insert_node(
                            position,
                            UiNode::from_component(
                                &component,
                                component.type_path.clone(),
                                id.clone(),
                                BridgeSide::Receive,
                            ),
                        );
                        self.editor.insert_node(
                            position + egui::vec2(250.0, 0.0),
                            UiNode::from_component(
                                &component,
                                component.type_path.clone(),
                                id,
                                BridgeSide::Transmit,
                            ),
                        );
                    } else {
                        self.editor.insert_node(
                            position,
                            UiNode::from_component(
                                &component,
                                component.type_path.clone(),
                                id,
                                BridgeSide::Regular,
                            ),
                        );
                    }
                    self.dirty = true;
                    self.mark_layout_dirty();
                }
                GraphAction::Delete(node_id) => {
                    let Some(node) = self.editor.get_node(node_id).cloned() else { continue };
                    let matching = self
                        .editor
                        .node_ids()
                        .filter_map(|(id, candidate)| {
                            (candidate.model_id == node.model_id).then_some(id)
                        })
                        .collect::<Vec<_>>();
                    for id in matching {
                        self.editor.remove_node(id);
                    }
                    self.selected_node = None;
                    let Some(config) = &mut self.config else { continue };
                    if let Some(index) = config
                        .document
                        .nodes
                        .iter()
                        .position(|candidate| candidate.id == node.model_id)
                    {
                        config.document.nodes.remove(index);
                        config
                            .document
                            .edges
                            .retain(|edge| edge.source != index && edge.target != index);
                        for edge in &mut config.document.edges {
                            if edge.source > index {
                                edge.source -= 1;
                            }
                            if edge.target > index {
                                edge.target -= 1;
                            }
                        }
                        self.dirty = true;
                        self.mark_layout_dirty();
                    }
                }
            }
        }
    }

    fn save(&mut self, overwrite_external: bool) {
        let Some(config) = &mut self.config else { return };
        if config.document.has_hard_errors() {
            self.status = "Save blocked by validation errors".into();
            return;
        }
        if !overwrite_external && config.disk_changed() {
            self.overwrite_dialog = true;
            return;
        }
        if let Err(error) = config.rendered() {
            self.status = format!("Save failed: {error}");
            return;
        }
        let mut dependencies: HashSet<_> = config
            .document
            .nodes
            .iter()
            .filter_map(|node| node.pending_workspace_dependency.clone())
            .collect();
        dependencies.extend(config.document.monitors.iter().filter_map(|monitor| {
            self.catalog.find(&monitor.component_type).and_then(|component| {
                component
                    .workspace_only
                    .then(|| component.package.clone())
            })
        }));
        if let Some(project) = &self.project
            && let Err(error) = project.add_workspace_dependencies(&dependencies)
        {
            self.status = format!("Could not update Cargo.toml: {error}");
            return;
        }
        let saved = match config.save(overwrite_external) {
            Ok(()) => {
                for node in &mut config.document.nodes {
                    node.pending_workspace_dependency = None;
                }
                self.dirty = false;
                self.overwrite_dialog = false;
                self.status = format!("Saved {}", config.path.display());
                true
            }
            Err(ConfigError::ExternalChange) => {
                self.overwrite_dialog = true;
                false
            }
            Err(error) => {
                self.status = format!("Save failed: {error}");
                false
            }
        };
        if saved {
            self.layout_save_after = None;
            if let Err(error) = self.save_layout() {
                self.status = format!("Saved config, but could not save {LAYOUT_FILE_NAME}: {error}");
            }
        }
    }

    fn toolbar(&mut self, root: &mut egui::Ui) {
        egui::Panel::top("toolbar").show(root, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Open folder…").clicked()
                    && let Some(path) = rfd::FileDialog::new().pick_folder()
                {
                    self.open_project(&path);
                }
                let save = ui.add_enabled(self.config.is_some(), egui::Button::new("Save"));
                if save.clicked() {
                    self.save(false);
                }
                if ui.button("Reindex").clicked() {
                    self.reindex();
                }
                ui.separator();
                ui.selectable_value(&mut self.view, View::Graph, "Graph");
                ui.selectable_value(&mut self.view, View::Constants, "Constants");
                ui.selectable_value(&mut self.view, View::Monitors, "Monitors");
                ui.separator();
                if self.view == View::Graph && self.config.is_some() {
                    ui.label("Right-click the canvas to add a node");
                    ui.separator();
                }
                if self.dirty {
                    ui.colored_label(egui::Color32::YELLOW, "Unsaved changes");
                }
                ui.label(&self.status);
            });
        });
    }

    fn graph_view(&mut self, root: &mut egui::Ui) {
        self.inspector(root);
        egui::CentralPanel::default().show(root, |ui| {
            if self.config.is_none() {
                ui.centered_and_justified(|ui| ui.heading("Open a Copper application folder"));
                return;
            }
            let positions_before = self.graph_positions();
            let mut actions = Vec::new();
            let mut camera_changed = false;
            let widget = SnarlWidget::new()
                .id(self.graph_widget_id)
                .style(SnarlStyle {
                    min_scale: Some(0.2),
                    max_scale: Some(2.0),
                    crisp_magnified_text: Some(true),
                    ..SnarlStyle::new()
                });
            let Some(config) = &mut self.config else { return };
            let mut viewer = GraphViewer {
                document: &mut config.document,
                catalog: &self.catalog,
                status: &mut self.status,
                dirty: &mut self.dirty,
                actions: &mut actions,
                menu_query: &mut self.node_menu_query,
                camera: &mut self.camera,
                restore_camera: &mut self.restore_camera,
                camera_changed: &mut camera_changed,
            };
            widget.show(&mut self.editor, &mut viewer, ui);
            let selected = widget.get_selected_nodes(ui);
            self.selected_node = self
                .selected_node
                .filter(|node| selected.contains(node))
                .or_else(|| selected.last().copied());
            let delete_requested = !ui.ctx().egui_wants_keyboard_input()
                && ui.ctx().input_mut(|input| {
                    input.consume_key(egui::Modifiers::NONE, egui::Key::Delete)
                        || input.consume_key(egui::Modifiers::NONE, egui::Key::Backspace)
                        || input.consume_key(egui::Modifiers::NONE, egui::Key::X)
                });
            if delete_requested && let Some(node) = self.selected_node {
                actions.push(GraphAction::Delete(node));
            }
            if self.graph_positions() != positions_before || camera_changed {
                self.mark_layout_dirty();
            }
            self.handle_graph_actions(actions);
        });
    }

    fn inspector(&mut self, root: &mut egui::Ui) {
        let mut layout_changed = false;
        egui::Panel::right("inspector")
            .default_size(330.0)
            .show(root, |ui| {
                ui.heading("Inspector");
                let Some(index) = self.selected_model_index() else {
                    ui.label("Select a node to edit its ID and configuration.");
                    return;
                };
                let hints = self
                    .config
                    .as_ref()
                    .and_then(|config| config.document.nodes.get(index))
                    .and_then(|node| self.catalog.find(&node.component_type))
                    .map(|component| component.config_hints.clone())
                    .unwrap_or_default();
                let Some(config) = &mut self.config else { return };
                let other_ids = config
                    .document
                    .nodes
                    .iter()
                    .enumerate()
                    .filter(|(other_index, _)| *other_index != index)
                    .map(|(_, node)| node.id.clone())
                    .collect::<HashSet<_>>();
                let Some(node) = config.document.nodes.get_mut(index) else { return };
                ui.label(&node.component_type);
                let old_id = node.id.clone();
                if ui.text_edit_singleline(&mut node.id).changed() {
                    if other_ids.contains(&node.id) {
                        self.status = format!("Node ID '{}' is already in use", node.id);
                        node.id = old_id;
                        return;
                    }
                    self.editor.nodes_mut().for_each(|graph_node| {
                        if graph_node.model_id == old_id {
                            graph_node.model_id = node.id.clone();
                        }
                    });
                    self.dirty = true;
                    layout_changed = true;
                }
                ui.separator();
                ui.heading("Optional configuration");
                for hint in hints {
                    if node.config.0.contains_key(&hint.key) {
                        continue;
                    }
                    let detail = hint.rust_type.as_deref().unwrap_or("unknown type");
                    if ui.button(format!("+ {} ({detail})", hint.key)).clicked() {
                        node.config.0.insert(
                            hint.key,
                            hint.default_ron.unwrap_or_else(|| "()".into()),
                        );
                        self.dirty = true;
                    }
                }
                edit_config(ui, &mut node.config, &mut self.new_config_key, &mut self.dirty);
                ui.separator();
                if node.kind != ComponentKind::Bridge {
                    self.dirty |= edit_optional_bool(ui, "Run in background", &mut node.background);
                    self.dirty |= edit_optional_bool(ui, "Run in simulation", &mut node.run_in_sim);
                    self.dirty |=
                        edit_optional_bool(ui, "Enable logging", &mut node.logging_enabled);
                } else {
                    ui.heading("Bridge channels");
                    for channel in &mut node.channels {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                self.dirty |=
                                    ui.checkbox(&mut channel.enabled, &channel.id).changed();
                                ui.label(match channel.direction {
                                    PortDirection::Input => "TX · from graph",
                                    PortDirection::Output => "RX · into graph",
                                });
                            });
                            let mut route = channel.route.clone().unwrap_or_default();
                            ui.horizontal(|ui| {
                                ui.label("route");
                                if ui.text_edit_singleline(&mut route).changed() {
                                    channel.route = (!route.trim().is_empty()).then_some(route);
                                    self.dirty = true;
                                }
                            });
                            edit_config(
                                ui,
                                &mut channel.config,
                                &mut self.new_config_key,
                                &mut self.dirty,
                            );
                        });
                    }
                }
            });
        if layout_changed {
            self.mark_layout_dirty();
        }
    }

    fn constants_view(&mut self, root: &mut egui::Ui) {
        let ctx = root.ctx().clone();
        egui::CentralPanel::default().show(root, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Constants");
                if ui.button("+ Numeric").clicked()
                    && let Some(config) = &mut self.config
                {
                    config.document.constants.push(ConstantDefinition::Numeric {
                        id: "new_constant".into(),
                        module: "constants".into(),
                        storage: NumericStorage::F64,
                        quantity: None,
                        unit: None,
                        value_ron: "0.0".into(),
                    });
                    self.dirty = true;
                }
                if ui.button("+ Expression").clicked()
                    && let Some(config) = &mut self.config
                {
                    config.document.constants.push(ConstantDefinition::Expression {
                        id: "new_constant".into(),
                        module: "constants".into(),
                        rust_type: "f64".into(),
                        expression: "0.0".into(),
                    });
                    self.dirty = true;
                }
                if ui.button("Transform wizard…").clicked() {
                    self.show_transform_wizard = true;
                }
            });
            ui.separator();
            let Some(config) = &mut self.config else {
                ui.label("Open a project first.");
                return;
            };
            let mut remove = None;
            egui::ScrollArea::vertical().show(ui, |ui| {
                for (index, constant) in config.document.constants.iter_mut().enumerate() {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.strong(constant.qualified_id());
                            if ui.button("Remove").clicked() {
                                remove = Some(index);
                            }
                        });
                        edit_constant(ui, constant, &mut self.dirty);
                    });
                }
            });
            if let Some(index) = remove {
                config.document.constants.remove(index);
                self.dirty = true;
            }
        });
        self.transform_window(&ctx);
    }

    fn transform_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_transform_wizard;
        egui::Window::new("Transform constant")
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label("Creates a const Transform3D expression. Angles are radians.");
                ui.horizontal(|ui| {
                    ui.label("ID");
                    ui.text_edit_singleline(&mut self.transform_draft.id);
                });
                ui.horizontal(|ui| {
                    ui.label("Module");
                    ui.text_edit_singleline(&mut self.transform_draft.module);
                });
                ui.horizontal(|ui| {
                    ui.label("Rust type");
                    ui.text_edit_singleline(&mut self.transform_draft.rust_type);
                });
                egui::Grid::new("transform_values").show(ui, |ui| {
                    ui.label("Translation");
                    ui.add(egui::DragValue::new(&mut self.transform_draft.tx).prefix("x "));
                    ui.add(egui::DragValue::new(&mut self.transform_draft.ty).prefix("y "));
                    ui.add(egui::DragValue::new(&mut self.transform_draft.tz).prefix("z "));
                    ui.end_row();
                    ui.label("Euler XYZ");
                    ui.add(egui::DragValue::new(&mut self.transform_draft.roll).prefix("r "));
                    ui.add(egui::DragValue::new(&mut self.transform_draft.pitch).prefix("p "));
                    ui.add(egui::DragValue::new(&mut self.transform_draft.yaw).prefix("y "));
                    ui.end_row();
                });
                if ui.button("Add transform").clicked()
                    && let Some(config) = &mut self.config
                {
                    let draft = &self.transform_draft;
                    config.document.constants.push(ConstantDefinition::Expression {
                        id: draft.id.clone(),
                        module: draft.module.clone(),
                        rust_type: draft.rust_type.clone(),
                        expression: format!(
                            "{}::from_translation_euler_xyz([cu29::units::si::f64::Length {{ value: {} }}, cu29::units::si::f64::Length {{ value: {} }}, cu29::units::si::f64::Length {{ value: {} }}], [cu29::units::si::f64::Angle {{ value: {} }}, cu29::units::si::f64::Angle {{ value: {} }}, cu29::units::si::f64::Angle {{ value: {} }}])",
                            draft.rust_type,
                            draft.tx,
                            draft.ty,
                            draft.tz,
                            draft.roll,
                            draft.pitch,
                            draft.yaw
                        ),
                    });
                    self.dirty = true;
                    self.show_transform_wizard = false;
                }
            });
        self.show_transform_wizard &= open;
    }

    fn monitors_view(&mut self, root: &mut egui::Ui) {
        egui::Panel::left("monitor_catalog")
            .default_size(280.0)
            .show(root, |ui| {
                ui.heading("Available monitors");
                for component in self
                    .catalog
                    .components
                    .iter()
                    .filter(|component| component.kind == ComponentKind::Monitor)
                {
                    ui.horizontal(|ui| {
                        ui.label(&component.display_name);
                        if ui.button("Add").clicked()
                            && let Some(config) = &mut self.config
                        {
                            config.document.monitors.push(MonitorInstance {
                                component_type: component.type_path.clone(),
                                config: EditableConfig::default(),
                            });
                            self.dirty = true;
                        }
                    });
                }
            });
        egui::CentralPanel::default().show(root, |ui| {
            ui.heading("Configured monitors");
            let Some(config) = &mut self.config else { return };
            let mut remove = None;
            for (index, monitor) in config.document.monitors.iter_mut().enumerate() {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        self.dirty |= ui.text_edit_singleline(&mut monitor.component_type).changed();
                        if ui.button("Remove").clicked() {
                            remove = Some(index);
                        }
                    });
                    edit_config(
                        ui,
                        &mut monitor.config,
                        &mut self.new_config_key,
                        &mut self.dirty,
                    );
                });
            }
            if let Some(index) = remove {
                config.document.monitors.remove(index);
                self.dirty = true;
            }
        });
    }

    fn diagnostics(&mut self, root: &mut egui::Ui) {
        let Some(document) = self.config.as_ref().map(|config| &config.document) else {
            return;
        };
        let diagnostics = document.diagnostics();
        let index_diagnostics = self.catalog.diagnostics.clone();
        let metadata_warning = self
            .project
            .as_ref()
            .and_then(|project| project.metadata_warning.clone());
        egui::Panel::bottom("diagnostics")
            .resizable(true)
            .default_size(
                if diagnostics.is_empty()
                    && index_diagnostics.is_empty()
                    && metadata_warning.is_none()
                {
                    28.0
                } else {
                    100.0
                },
            )
            .show(root, |ui| {
                ui.horizontal(|ui| {
                    ui.strong("Validation");
                    ui.label(if diagnostics.is_empty() {
                        "Ready to save"
                    } else {
                        "Errors block saving; incomplete inputs are warnings"
                    });
                });
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if let Some(warning) = &metadata_warning {
                        ui.colored_label(
                            egui::Color32::YELLOW,
                            format!("Cargo metadata fallback: {warning}"),
                        );
                    }
                    for diagnostic in &index_diagnostics {
                        ui.colored_label(
                            egui::Color32::YELLOW,
                            format!("Indexer: {diagnostic}"),
                        );
                    }
                    for diagnostic in diagnostics {
                        let color = match diagnostic.severity {
                            DiagnosticSeverity::Error => egui::Color32::RED,
                            DiagnosticSeverity::Warning => egui::Color32::YELLOW,
                            DiagnosticSeverity::Info => egui::Color32::LIGHT_BLUE,
                        };
                        ui.colored_label(color, diagnostic.message);
                    }
                });
            });
    }

    fn overwrite_window(&mut self, ctx: &egui::Context) {
        if !self.overwrite_dialog {
            return;
        }
        egui::Window::new("File changed on disk")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("copperconfig.ron changed since it was opened.");
                ui.label("Overwrite it with the graph currently in Evograph?");
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.overwrite_dialog = false;
                    }
                    if ui.button("Overwrite").clicked() {
                        self.save(true);
                    }
                });
            });
    }
}

impl eframe::App for EvographApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.process_file_events();
        self.process_layout_save();
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
            self.save(false);
        }
        self.toolbar(ui);
        self.diagnostics(ui);
        match self.view {
            View::Graph => self.graph_view(ui),
            View::Constants => self.constants_view(ui),
            View::Monitors => self.monitors_view(ui),
        }
        self.overwrite_window(&ctx);
        if self.reindex_after.is_some() || self.layout_save_after.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        let _ = self.save_layout();
    }
}

fn port_color(canonical_type: &str) -> egui::Color32 {
    let hash = canonical_type.bytes().fold(0x811c9dc5_u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(16777619)
    });
    egui::Color32::from_rgb(
        72 + (hash & 0x7f) as u8,
        72 + ((hash >> 8) & 0x7f) as u8,
        72 + ((hash >> 16) & 0x7f) as u8,
    )
}

fn node_color(node: &UiNode) -> egui::Color32 {
    match (node.kind, node.side) {
        (ComponentKind::Bridge, BridgeSide::Receive) => egui::Color32::from_rgb(35, 100, 70),
        (ComponentKind::Bridge, BridgeSide::Transmit) => egui::Color32::from_rgb(115, 65, 40),
        (ComponentKind::Source, _) => egui::Color32::from_rgb(35, 100, 70),
        (ComponentKind::Task, _) => egui::Color32::from_rgb(45, 75, 125),
        (ComponentKind::Sink, _) => egui::Color32::from_rgb(115, 65, 40),
        (ComponentKind::Bridge, _) => egui::Color32::from_rgb(100, 55, 115),
        (ComponentKind::Monitor, _) => egui::Color32::from_rgb(80, 80, 80),
        (ComponentKind::Unresolved, _) => egui::Color32::from_rgb(130, 45, 45),
    }
}

fn remove_document_edge(
    document: &mut GraphDocument,
    snarl: &Snarl<UiNode>,
    from: OutPinId,
    to: InPinId,
) {
    let source_id = &snarl[from.node].model_id;
    let target_id = &snarl[to.node].model_id;
    let source = document.nodes.iter().position(|node| &node.id == source_id);
    let target = document.nodes.iter().position(|node| &node.id == target_id);
    if let (Some(source), Some(target)) = (source, target) {
        document.edges.retain(|edge| {
            !(edge.source == source
                && edge.source_port == from.output
                && edge.target == target
                && edge.target_port == to.input)
        });
    }
}

fn visual_label(id: &str, side: BridgeSide) -> String {
    let label = match side {
        BridgeSide::Regular => id.to_string(),
        BridgeSide::Receive => format!("{id} · RX"),
        BridgeSide::Transmit => format!("{id} · TX"),
    };
    label
}

fn layout_key(node: &UiNode) -> String {
    let prefix = match node.side {
        BridgeSide::Regular => "node",
        BridgeSide::Receive => "bridge-rx",
        BridgeSide::Transmit => "bridge-tx",
    };
    format!("{prefix}:{}", node.model_id)
}

fn read_layout(path: &Path) -> Option<EvoNodeLayout> {
    let source = fs::read_to_string(path).ok()?;
    let mut layout = ron::from_str::<EvoNodeLayout>(&source).ok()?;
    if layout.version == 1 {
        layout.version = layout_version();
        layout.camera = None;
    }
    (layout.version == layout_version()).then_some(layout)
}

fn component_for_node(node: &GraphNode) -> ComponentDescriptor {
    ComponentDescriptor {
        type_path: node.component_type.clone(),
        display_name: node.id.clone(),
        package: String::new(),
        kind: node.kind,
        inputs: node.inputs.clone(),
        outputs: node.outputs.clone(),
        channels: Vec::new(),
        config_hints: Vec::new(),
        workspace_only: false,
        source_path: None,
    }
}

fn edit_config(
    ui: &mut egui::Ui,
    config: &mut EditableConfig,
    new_key: &mut String,
    dirty: &mut bool,
) {
    let mut remove = None;
    for key in config.0.keys().cloned().collect::<Vec<_>>() {
        ui.horizontal(|ui| {
            ui.label(&key);
            let value = config.0.get_mut(&key).expect("key was just collected");
            let response = ui.text_edit_singleline(value);
            if response.changed() {
                *dirty = true;
            }
            if ron::from_str::<ron::Value>(value).is_err() {
                response.on_hover_text("This is not valid RON");
                ui.colored_label(egui::Color32::RED, "invalid");
            }
            if ui.small_button("×").clicked() {
                remove = Some(key.clone());
            }
        });
    }
    if let Some(key) = remove {
        config.0.remove(&key);
        *dirty = true;
    }
    ui.horizontal(|ui| {
        ui.text_edit_singleline(new_key);
        if ui.button("Add key").clicked() && !new_key.trim().is_empty() {
            config.0.insert(new_key.trim().to_string(), "()".into());
            new_key.clear();
            *dirty = true;
        }
    });
}

fn edit_constant(ui: &mut egui::Ui, constant: &mut ConstantDefinition, dirty: &mut bool) {
    let changed = match constant {
        ConstantDefinition::Numeric {
            id,
            module,
            storage,
            quantity,
            unit,
            value_ron,
        } => {
            let mut changed = false;
            egui::Grid::new(ui.next_auto_id()).show(ui, |ui| {
                ui.label("ID");
                changed |= ui.text_edit_singleline(id).changed();
                ui.end_row();
                ui.label("Module");
                changed |= ui.text_edit_singleline(module).changed();
                ui.end_row();
                ui.label("Storage");
                egui::ComboBox::from_id_salt(ui.next_auto_id())
                    .selected_text(storage.ron_name())
                    .show_ui(ui, |ui| {
                        for candidate in NumericStorage::ALL {
                            changed |= ui
                                .selectable_value(storage, candidate, candidate.ron_name())
                                .changed();
                        }
                    });
                ui.end_row();
                ui.label("Quantity");
                let mut quantity_text = quantity.clone().unwrap_or_default();
                if ui.text_edit_singleline(&mut quantity_text).changed() {
                    *quantity = (!quantity_text.trim().is_empty()).then_some(quantity_text);
                    changed = true;
                }
                ui.end_row();
                ui.label("Unit");
                let mut unit_text = unit.clone().unwrap_or_default();
                if ui.text_edit_singleline(&mut unit_text).changed() {
                    *unit = (!unit_text.trim().is_empty()).then_some(unit_text);
                    changed = true;
                }
                ui.end_row();
                ui.label("Value (RON)");
                changed |= ui.text_edit_singleline(value_ron).changed();
                ui.end_row();
            });
            changed
        }
        ConstantDefinition::Expression {
            id,
            module,
            rust_type,
            expression,
        } => {
            let mut changed = false;
            egui::Grid::new(ui.next_auto_id()).show(ui, |ui| {
                ui.label("ID");
                changed |= ui.text_edit_singleline(id).changed();
                ui.end_row();
                ui.label("Module");
                changed |= ui.text_edit_singleline(module).changed();
                ui.end_row();
                ui.label("Rust type");
                changed |= ui.text_edit_singleline(rust_type).changed();
                ui.end_row();
                ui.label("Const expression");
                changed |= ui.text_edit_multiline(expression).changed();
                ui.end_row();
            });
            changed
        }
    };
    *dirty |= changed;
}

fn edit_optional_bool(ui: &mut egui::Ui, label: &str, value: &mut Option<bool>) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(label);
        egui::ComboBox::from_id_salt(ui.next_auto_id())
            .selected_text(match value {
                None => "Copper default",
                Some(true) => "true",
                Some(false) => "false",
            })
            .show_ui(ui, |ui| {
                changed |= ui.selectable_value(value, None, "Copper default").changed();
                changed |= ui.selectable_value(value, Some(true), "true").changed();
                changed |= ui.selectable_value(value, Some(false), "false").changed();
            });
    });
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PortDescriptor;

    fn port(name: &str, direction: PortDirection) -> PortDescriptor {
        PortDescriptor {
            name: name.into(),
            direction,
            ordinal: 0,
            declared_type: "Payload".into(),
            serialized_type: "Payload".into(),
            canonical_type: "Payload".into(),
        }
    }

    fn component(
        name: &str,
        kind: ComponentKind,
        inputs: Vec<PortDescriptor>,
        outputs: Vec<PortDescriptor>,
    ) -> ComponentDescriptor {
        ComponentDescriptor {
            type_path: name.into(),
            display_name: name.into(),
            package: "fixture".into(),
            kind,
            inputs,
            outputs,
            channels: Vec::new(),
            config_hints: Vec::new(),
            workspace_only: false,
            source_path: None,
        }
    }

    fn attempt_viewer_connection(target_canonical_type: &str) -> (usize, usize, String) {
        let mut output = port("out", PortDirection::Output);
        output.declared_type = "AliasedPayload".into();
        output.canonical_type = "common::Payload".into();
        let mut input = port("in", PortDirection::Input);
        input.canonical_type = target_canonical_type.into();
        let source = component("Source", ComponentKind::Source, Vec::new(), vec![output]);
        let target = component("Target", ComponentKind::Sink, vec![input], Vec::new());
        let mut document = GraphDocument::default();
        document
            .nodes
            .push(GraphNode::from_component(&source, "source".into()));
        document
            .nodes
            .push(GraphNode::from_component(&target, "target".into()));

        let mut snarl = Snarl::new();
        let source_id = snarl.insert_node(
            egui::Pos2::ZERO,
            UiNode::from_component(
                &source,
                source.type_path.clone(),
                "source".into(),
                BridgeSide::Regular,
            ),
        );
        let target_id = snarl.insert_node(
            egui::pos2(100.0, 0.0),
            UiNode::from_component(
                &target,
                target.type_path.clone(),
                "target".into(),
                BridgeSide::Regular,
            ),
        );
        let from_id = OutPinId {
            node: source_id,
            output: 0,
        };
        let to_id = InPinId {
            node: target_id,
            input: 0,
        };
        let from = snarl.out_pin(from_id);
        let to = snarl.in_pin(to_id);
        let catalog = ComponentCatalog::default();
        let mut status = String::new();
        let mut dirty = false;
        let mut actions = Vec::new();
        let mut menu_query = String::new();
        let mut camera = CameraState::default();
        let mut restore_camera = false;
        let mut camera_changed = false;
        let mut viewer = GraphViewer {
            document: &mut document,
            catalog: &catalog,
            status: &mut status,
            dirty: &mut dirty,
            actions: &mut actions,
            menu_query: &mut menu_query,
            camera: &mut camera,
            restore_camera: &mut restore_camera,
            camera_changed: &mut camera_changed,
        };
        viewer.connect(&from, &to, &mut snarl);
        (
            document.edges.len(),
            snarl.out_pin(from_id).remotes.len(),
            status,
        )
    }

    #[test]
    fn bridge_sides_expose_only_the_ports_for_their_graph_direction() {
        let component = component(
            "Bridge",
            ComponentKind::Bridge,
            vec![port("command", PortDirection::Input)],
            vec![port("state", PortDirection::Output)],
        );
        let receive = UiNode::from_component(
            &component,
            "Bridge".into(),
            "bridge".into(),
            BridgeSide::Receive,
        );
        let transmit = UiNode::from_component(
            &component,
            "Bridge".into(),
            "bridge".into(),
            BridgeSide::Transmit,
        );
        assert!(receive.inputs.is_empty());
        assert_eq!(receive.outputs.len(), 1);
        assert_eq!(transmit.inputs.len(), 1);
        assert!(transmit.outputs.is_empty());
    }

    #[test]
    fn viewer_accepts_aliases_and_rejects_incompatible_connections() {
        let accepted = attempt_viewer_connection("common::Payload");
        assert_eq!((accepted.0, accepted.1), (1, 1));
        assert!(accepted.2.is_empty());

        let rejected = attempt_viewer_connection("OtherPayload");
        assert_eq!((rejected.0, rejected.1), (0, 0));
        assert!(rejected.2.contains("not compatible"));
    }

    #[test]
    fn layout_round_trips_and_distinguishes_bridge_sides() {
        let receive = UiNode {
            component_type: "Bridge".into(),
            model_id: "can".into(),
            kind: ComponentKind::Bridge,
            side: BridgeSide::Receive,
            inputs: Vec::new(),
            outputs: Vec::new(),
        };
        let transmit = UiNode {
            side: BridgeSide::Transmit,
            ..receive.clone()
        };
        assert_ne!(layout_key(&receive), layout_key(&transmit));

        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join(LAYOUT_FILE_NAME);
        let layout = EvoNodeLayout {
            version: layout_version(),
            positions: BTreeMap::from([(
                layout_key(&receive),
                NodePosition { x: 12.5, y: -4.0 },
            )]),
            camera: Some(CameraState {
                scale: 1.25,
                x: 30.0,
                y: -20.0,
            }),
        };
        fs::write(&path, ron::ser::to_string(&layout).unwrap()).unwrap();
        let loaded = read_layout(&path).unwrap();
        assert_eq!(loaded.positions[&layout_key(&receive)].x, 12.5);
        assert_eq!(loaded.camera.unwrap().scale, 1.25);
    }

    #[test]
    fn version_one_layouts_load_with_default_camera() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join(LAYOUT_FILE_NAME);
        fs::write(&path, "(version:1,positions:{\"node:a\":(x:1.0,y:2.0)})").unwrap();
        let loaded = read_layout(&path).unwrap();
        assert_eq!(loaded.version, layout_version());
        assert_eq!(loaded.camera, None);
    }

    #[test]
    fn camera_round_trips_through_scene_transform() {
        let camera = CameraState {
            scale: 0.75,
            x: 42.0,
            y: -17.0,
        };
        assert!(camera.approximately_eq(CameraState::from_transform(camera.transform())));
    }

    #[test]
    fn graph_labels_are_not_truncated() {
        let id = "a_component_id_that_is_longer_than_twenty_eight_characters";
        assert_eq!(visual_label(id, BridgeSide::Regular), id);
    }
}
