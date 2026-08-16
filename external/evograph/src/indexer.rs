use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use quote::ToTokens;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::visit::Visit;
use syn::{
    braced, Expr, ExprLit, ExprMethodCall, GenericArgument, ImplItem, Item, ItemImpl, ItemMacro,
    ItemType, Lit, LitStr, PathArguments, Token, Type, TypeMacro, TypePath, Visibility,
};

use crate::model::{
    BridgeChannelDescriptor, ComponentDescriptor, ComponentKind, ConfigHint, PortDescriptor,
    PortDirection, normalize_type,
};
use crate::project::{ProjectInfo, SourcePackage};

#[derive(Debug, Clone, Default)]
pub struct ComponentCatalog {
    pub components: Vec<ComponentDescriptor>,
    pub diagnostics: Vec<String>,
}

impl ComponentCatalog {
    pub fn find(&self, type_path: &str) -> Option<&ComponentDescriptor> {
        self.components.iter().find(|component| {
            normalize_type(&component.type_path) == normalize_type(type_path)
                || component.type_path.ends_with(type_path)
                || type_path.ends_with(&component.type_path)
        })
    }

    pub fn index(project: &ProjectInfo) -> Self {
        let mut collector = Collector::default();
        for package in &project.source_packages {
            collector.index_package(package, package.package_name == project.package_name);
        }
        collector.finish()
    }
}

#[derive(Default)]
struct Collector {
    impls: Vec<ImplRecord>,
    aliases: HashMap<String, AliasRecord>,
    simple_aliases: HashMap<String, Vec<String>>,
    channel_sets: HashMap<String, Vec<BridgeChannelDescriptor>>,
    public_uses: Vec<PublicUseRecord>,
    diagnostics: Vec<String>,
}

struct PublicUseRecord {
    exported: String,
    target: String,
    name: String,
    package: String,
}

#[derive(Clone)]
struct AliasRecord {
    qualified: String,
    target: Type,
    imports: HashMap<String, String>,
    source_path: PathBuf,
    package: String,
    crate_name: String,
    module: String,
    workspace_only: bool,
    is_app: bool,
}

struct ImplRecord {
    trait_name: String,
    self_type: Type,
    qualified_self: String,
    inputs: Vec<String>,
    outputs: Vec<String>,
    tx_set: Option<String>,
    rx_set: Option<String>,
    hints: Vec<ConfigHint>,
    imports: HashMap<String, String>,
    source_path: PathBuf,
    package: String,
    crate_name: String,
    module: String,
    workspace_only: bool,
    is_app: bool,
}

impl Collector {
    fn index_package(&mut self, package: &SourcePackage, is_app: bool) {
        if !package.source_root.exists() {
            self.diagnostics.push(format!(
                "Source for package '{}' is not available at {}",
                package.package_name,
                package.source_root.display()
            ));
            return;
        }
        for entry in walkdir::WalkDir::new(&package.source_root)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "rs"))
        {
            let path = entry.path();
            let Ok(source) = fs::read_to_string(path) else {
                continue;
            };
            let syntax = match syn::parse_file(&source) {
                Ok(syntax) => syntax,
                Err(error) => {
                    self.diagnostics
                        .push(format!("Could not parse {}: {error}", path.display()));
                    continue;
                }
            };
            let module = module_for_path(&package.source_root, path);
            self.collect_items(package, is_app, path, &module, &syntax.items);
        }
    }

    fn collect_items(
        &mut self,
        package: &SourcePackage,
        is_app: bool,
        path: &Path,
        module: &str,
        items: &[Item],
    ) {
        let imports = collect_imports(items);
        for item in items {
            let Item::Use(item_use) = item else { continue };
            if !matches!(item_use.vis, Visibility::Public(_)) {
                continue;
            }
            let mut exported = HashMap::new();
            flatten_use_tree(String::new(), &item_use.tree, &mut exported);
            for (name, target) in exported {
                self.public_uses.push(PublicUseRecord {
                    exported: qualify_item(is_app, &package.crate_name, module, &name),
                    target,
                    name,
                    package: package.package_name.clone(),
                });
            }
        }
        for item in items {
            match item {
                Item::Type(alias) => {
                    self.collect_alias(package, is_app, path, module, &imports, alias)
                }
                Item::Impl(item_impl) => {
                    self.collect_impl(package, is_app, path, module, &imports, item_impl)
                }
                Item::Macro(item_macro) => {
                    self.collect_channel_macro(package, module, &imports, item_macro)
                }
                Item::Mod(item_mod) => {
                    if let Some((_, nested)) = &item_mod.content {
                        let nested_module = if module.is_empty() {
                            item_mod.ident.to_string()
                        } else {
                            format!("{module}::{}", item_mod.ident)
                        };
                        self.collect_items(package, is_app, path, &nested_module, nested);
                    }
                }
                _ => {}
            }
        }
    }

    fn collect_alias(
        &mut self,
        package: &SourcePackage,
        is_app: bool,
        path: &Path,
        module: &str,
        imports: &HashMap<String, String>,
        alias: &ItemType,
    ) {
        let qualified = qualify_item(is_app, &package.crate_name, module, &alias.ident.to_string());
        let record = AliasRecord {
            qualified: qualified.clone(),
            target: (*alias.ty).clone(),
            imports: imports.clone(),
            source_path: path.to_path_buf(),
            package: package.package_name.clone(),
            crate_name: package.crate_name.clone(),
            module: module.to_string(),
            workspace_only: package.workspace_only,
            is_app,
        };
        self.simple_aliases
            .entry(alias.ident.to_string())
            .or_default()
            .push(qualified.clone());
        self.aliases.insert(qualified, record);
    }

    fn collect_impl(
        &mut self,
        package: &SourcePackage,
        is_app: bool,
        path: &Path,
        module: &str,
        imports: &HashMap<String, String>,
        item_impl: &ItemImpl,
    ) {
        let Some((_, trait_path, _)) = &item_impl.trait_ else {
            return;
        };
        let Some(trait_name) = trait_path.segments.last().map(|segment| segment.ident.to_string())
        else {
            return;
        };
        if !matches!(
            trait_name.as_str(),
            "CuSrcTask" | "CuTask" | "CuSinkTask" | "CuBridge" | "CuMonitor"
        ) {
            return;
        }

        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        let mut tx_set = None;
        let mut rx_set = None;
        let mut hints = Vec::new();
        for item in &item_impl.items {
            match item {
                ImplItem::Type(item_type) if item_type.ident == "Input" => {
                    inputs = extract_message_types(&item_type.ty)
                }
                ImplItem::Type(item_type) if item_type.ident == "Output" => {
                    outputs = extract_message_types(&item_type.ty)
                }
                ImplItem::Type(item_type) if item_type.ident == "Tx" => {
                    tx_set = Some(type_to_string(&item_type.ty))
                }
                ImplItem::Type(item_type) if item_type.ident == "Rx" => {
                    rx_set = Some(type_to_string(&item_type.ty))
                }
                ImplItem::Fn(function) if function.sig.ident == "new" => {
                    let mut visitor = ConfigHintVisitor::default();
                    visitor.visit_block(&function.block);
                    hints.extend(visitor.hints);
                }
                _ => {}
            }
        }
        deduplicate_hints(&mut hints);
        let self_name = type_to_string(&item_impl.self_ty);
        let qualified_self = qualify_type(
            &self_name,
            is_app,
            &package.crate_name,
            module,
            imports,
        );
        self.impls.push(ImplRecord {
            trait_name,
            self_type: (*item_impl.self_ty).clone(),
            qualified_self,
            inputs,
            outputs,
            tx_set,
            rx_set,
            hints,
            imports: imports.clone(),
            source_path: path.to_path_buf(),
            package: package.package_name.clone(),
            crate_name: package.crate_name.clone(),
            module: module.to_string(),
            workspace_only: package.workspace_only,
            is_app,
        });
    }

    fn collect_channel_macro(
        &mut self,
        package: &SourcePackage,
        module: &str,
        imports: &HashMap<String, String>,
        item_macro: &ItemMacro,
    ) {
        let Some(name) = item_macro.mac.path.segments.last().map(|segment| segment.ident.to_string())
        else {
            return;
        };
        let direction = match name.as_str() {
            "tx_channels" => PortDirection::Input,
            "rx_channels" => PortDirection::Output,
            _ => return,
        };
        let Ok(definition) = syn::parse2::<ChannelSetSyntax>(item_macro.mac.tokens.clone()) else {
            return;
        };
        let set_name = definition.name.to_string();
        let channels: Vec<BridgeChannelDescriptor> = definition
            .channels
            .into_iter()
            .enumerate()
            .map(|(ordinal, channel)| {
                let declared = type_to_string(&channel.payload);
                let serialized = qualify_type(
                    &declared,
                    false,
                    &package.crate_name,
                    module,
                    imports,
                );
                BridgeChannelDescriptor {
                    id: channel.id.to_string(),
                    direction,
                    payload: PortDescriptor {
                        name: channel.id.to_string(),
                        direction,
                        ordinal,
                        declared_type: declared,
                        serialized_type: serialized.clone(),
                        canonical_type: serialized,
                    },
                    default_route: channel.route.map(|route| route.value()),
                }
            })
            .collect();
        self.channel_sets.insert(set_name.clone(), channels.clone());
        self.channel_sets.insert(
            format!("{}::{module}::{set_name}", package.crate_name).replace("::::", "::"),
            channels,
        );
    }

    fn finish(self) -> ComponentCatalog {
        let mut components = Vec::new();
        for record in &self.impls {
            if record.trait_name == "CuBridge" && type_has_generics(&record.self_type) {
                continue;
            }
            components.push(self.component_from_impl(record, None));
        }

        // Concrete aliases are how generic bridges become nameable in copperconfig.ron.
        for alias in self.aliases.values() {
            let Type::Path(alias_path) = &alias.target else {
                continue;
            };
            let Some(base_segment) = alias_path.path.segments.last() else {
                continue;
            };
            let base_name = base_segment.ident.to_string();
            let Some(generic_impl) = self.impls.iter().find(|record| {
                record.trait_name == "CuBridge"
                    && type_base_name(&record.self_type).as_deref() == Some(base_name.as_str())
                    && type_has_generics(&record.self_type)
            }) else {
                continue;
            };
            components.push(self.component_from_impl(generic_impl, Some(alias)));
        }

        for component in &mut components {
            for port in component.inputs.iter_mut().chain(&mut component.outputs) {
                port.canonical_type = self.resolve_alias(&port.serialized_type, 0);
            }
            for channel in &mut component.channels {
                channel.payload.canonical_type = self.resolve_alias(&channel.payload.serialized_type, 0);
            }
        }
        let mut reexports = Vec::new();
        for public_use in &self.public_uses {
            let mut candidates = components.iter().filter(|component| {
                component.package == public_use.package
                    && component.display_name == public_use.name
                    && normalize_type(&component.type_path)
                        .ends_with(&normalize_type(&public_use.target))
            });
            let Some(component) = candidates.next() else { continue };
            if candidates.next().is_some() {
                continue;
            }
            let mut component = component.clone();
            component.type_path = public_use.exported.clone();
            reexports.push(component);
        }
        components.extend(reexports);
        components.sort_by(|left, right| {
            left.kind
                .label()
                .cmp(right.kind.label())
                .then_with(|| left.type_path.cmp(&right.type_path))
        });
        components.dedup_by(|left, right| left.type_path == right.type_path);
        ComponentCatalog {
            components,
            diagnostics: self.diagnostics,
        }
    }

    fn component_from_impl(
        &self,
        record: &ImplRecord,
        concrete_alias: Option<&AliasRecord>,
    ) -> ComponentDescriptor {
        let (type_path, package, workspace_only, source_path, is_app, crate_name, imports) =
            if let Some(alias) = concrete_alias {
                (
                    alias.qualified.clone(),
                    alias.package.clone(),
                    alias.workspace_only,
                    alias.source_path.clone(),
                    alias.is_app,
                    alias.crate_name.clone(),
                    alias.imports.clone(),
                )
            } else {
                (
                    record.qualified_self.clone(),
                    record.package.clone(),
                    record.workspace_only,
                    record.source_path.clone(),
                    record.is_app,
                    record.crate_name.clone(),
                    record.imports.clone(),
                )
            };
        let kind = match record.trait_name.as_str() {
            "CuSrcTask" => ComponentKind::Source,
            "CuTask" => ComponentKind::Task,
            "CuSinkTask" => ComponentKind::Sink,
            "CuBridge" => ComponentKind::Bridge,
            "CuMonitor" => ComponentKind::Monitor,
            _ => ComponentKind::Unresolved,
        };
        let mut channels = Vec::new();
        if kind == ComponentKind::Bridge {
            if let Some(alias) = concrete_alias {
                let generic_types = generic_arguments(&alias.target);
                if let Some(tx) = generic_types.first().and_then(|name| self.lookup_channels(name)) {
                    channels.extend(tx.clone());
                }
                if let Some(rx) = generic_types.get(1).and_then(|name| self.lookup_channels(name)) {
                    channels.extend(rx.clone());
                }
            } else {
                if let Some(tx) = record.tx_set.as_deref().and_then(|name| self.lookup_channels(name)) {
                    channels.extend(tx.clone());
                }
                if let Some(rx) = record.rx_set.as_deref().and_then(|name| self.lookup_channels(name)) {
                    channels.extend(rx.clone());
                }
            }
        }
        let inputs = if kind == ComponentKind::Bridge {
            channels
                .iter()
                .filter(|channel| channel.direction == PortDirection::Input)
                .enumerate()
                .map(|(ordinal, channel)| {
                    let mut port = channel.payload.clone();
                    port.ordinal = ordinal;
                    port
                })
                .collect()
        } else {
            make_ports(
                &record.inputs,
                PortDirection::Input,
                is_app,
                &crate_name,
                &record.module,
                &imports,
            )
        };
        let outputs = if kind == ComponentKind::Bridge {
            channels
                .iter()
                .filter(|channel| channel.direction == PortDirection::Output)
                .enumerate()
                .map(|(ordinal, channel)| {
                    let mut port = channel.payload.clone();
                    port.ordinal = ordinal;
                    port
                })
                .collect()
        } else {
            make_ports(
                &record.outputs,
                PortDirection::Output,
                is_app,
                &crate_name,
                &record.module,
                &imports,
            )
        };
        let display_name = type_path
            .rsplit("::")
            .next()
            .unwrap_or(&type_path)
            .to_string();
        ComponentDescriptor {
            type_path,
            display_name,
            package,
            kind,
            inputs,
            outputs,
            channels,
            config_hints: record.hints.clone(),
            workspace_only,
            source_path: Some(source_path.display().to_string()),
        }
    }

    fn lookup_channels(&self, name: &str) -> Option<&Vec<BridgeChannelDescriptor>> {
        self.channel_sets.get(name).or_else(|| {
            self.channel_sets
                .iter()
                .find(|(key, _)| key.ends_with(&format!("::{name}")))
                .map(|(_, value)| value)
        })
    }

    fn resolve_alias(&self, ty: &str, depth: usize) -> String {
        if depth > 16 {
            return normalize_type(ty);
        }
        let normalized = normalize_type(ty);
        let exact = self.aliases.get(&normalized);
        let simple = normalized.rsplit("::").next().and_then(|name| {
            self.simple_aliases
                .get(name)
                .filter(|matches| matches.len() == 1)
                .and_then(|matches| self.aliases.get(&matches[0]))
        });
        if let Some(alias) = exact.or(simple) {
            let target = qualify_type(
                &type_to_string(&alias.target),
                alias.is_app,
                &alias.crate_name,
                &alias.module,
                &alias.imports,
            );
            return self.resolve_alias(&target, depth + 1);
        }
        normalized
    }
}

fn make_ports(
    types: &[String],
    direction: PortDirection,
    is_app: bool,
    crate_name: &str,
    module: &str,
    imports: &HashMap<String, String>,
) -> Vec<PortDescriptor> {
    types
        .iter()
        .enumerate()
        .map(|(ordinal, declared)| {
            let serialized = qualify_type(declared, is_app, crate_name, module, imports);
            PortDescriptor {
                name: declared.clone(),
                direction,
                ordinal,
                declared_type: declared.clone(),
                serialized_type: serialized.clone(),
                canonical_type: serialized,
            }
        })
        .collect()
}

fn extract_message_types(ty: &Type) -> Vec<String> {
    match ty {
        Type::Tuple(tuple) => tuple.elems.iter().flat_map(extract_message_types).collect(),
        Type::Reference(reference) => extract_message_types(&reference.elem),
        Type::Macro(type_macro) => extract_macro_types(type_macro),
        Type::Path(path) => {
            let Some(segment) = path.path.segments.last() else {
                return vec![];
            };
            if segment.ident == "CuMsg" {
                if let PathArguments::AngleBracketed(arguments) = &segment.arguments {
                    return arguments
                        .args
                        .iter()
                        .filter_map(|argument| match argument {
                            GenericArgument::Type(ty) => Some(type_to_string(ty)),
                            _ => None,
                        })
                        .collect();
                }
            }
            vec![type_to_string(ty)]
        }
        _ => vec![type_to_string(ty)],
    }
}

fn extract_macro_types(type_macro: &TypeMacro) -> Vec<String> {
    let name = type_macro
        .mac
        .path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
        .unwrap_or_default();
    if !matches!(name.as_str(), "input_msg" | "output_msg") {
        return vec![type_macro.mac.tokens.to_string()];
    }
    split_top_level_commas(&type_macro.mac.tokens.to_string())
        .into_iter()
        .filter(|item| !item.trim_start().starts_with('\''))
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

fn split_top_level_commas(source: &str) -> Vec<String> {
    let mut depth = 0usize;
    let mut start = 0usize;
    let mut values = Vec::new();
    for (index, character) in source.char_indices() {
        match character {
            '(' | '[' | '<' | '{' => depth += 1,
            ')' | ']' | '>' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                values.push(source[start..index].to_string());
                start = index + 1;
            }
            _ => {}
        }
    }
    values.push(source[start..].to_string());
    values
}

fn qualify_type(
    source: &str,
    is_app: bool,
    crate_name: &str,
    module: &str,
    imports: &HashMap<String, String>,
) -> String {
    let compact = source.trim();
    if compact.is_empty() || is_primitive_or_composite(compact) {
        return compact.to_string();
    }
    let first = compact
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .find(|part| !part.is_empty())
        .unwrap_or(compact);
    if let Some(imported) = imports.get(first) {
        return compact.replacen(first, imported, 1);
    }
    if compact.starts_with("crate::") {
        return if is_app {
            compact.trim_start_matches("crate::").to_string()
        } else {
            compact.replacen("crate", crate_name, 1)
        };
    }
    if compact.contains("::") {
        return compact.to_string();
    }
    qualify_item(is_app, crate_name, module, compact)
}

fn qualify_item(is_app: bool, crate_name: &str, module: &str, item: &str) -> String {
    let prefix = if is_app { "" } else { crate_name };
    [prefix, module, item]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("::")
}

fn is_primitive_or_composite(value: &str) -> bool {
    matches!(
        value,
        "()" | "bool"
            | "u8" | "u16" | "u32" | "u64" | "u128" | "usize"
            | "i8" | "i16" | "i32" | "i64" | "i128" | "isize"
            | "f32" | "f64" | "String"
    ) || value.starts_with('(')
        || value.starts_with('[')
}

fn type_to_string(ty: &Type) -> String {
    ty.to_token_stream().to_string()
}

fn type_has_generics(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Path(TypePath { path, .. })
            if path.segments.last().is_some_and(|segment| !matches!(segment.arguments, PathArguments::None))
    )
}

fn type_base_name(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(path) => path.path.segments.last().map(|segment| segment.ident.to_string()),
        _ => None,
    }
}

fn generic_arguments(ty: &Type) -> Vec<String> {
    let Type::Path(path) = ty else { return vec![] };
    let Some(segment) = path.path.segments.last() else {
        return vec![];
    };
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return vec![];
    };
    arguments
        .args
        .iter()
        .filter_map(|argument| match argument {
            GenericArgument::Type(ty) => Some(type_to_string(ty)),
            _ => None,
        })
        .collect()
}

fn module_for_path(source_root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(source_root).unwrap_or(path);
    let mut parts: Vec<String> = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect();
    let Some(file) = parts.pop() else { return String::new() };
    let stem = file.trim_end_matches(".rs");
    if !matches!(stem, "lib" | "main" | "mod") {
        parts.push(stem.to_string());
    }
    parts.join("::")
}

fn collect_imports(items: &[Item]) -> HashMap<String, String> {
    let mut imports = HashMap::new();
    for item in items {
        if let Item::Use(item_use) = item {
            flatten_use_tree(String::new(), &item_use.tree, &mut imports);
        }
    }
    imports
}

fn flatten_use_tree(prefix: String, tree: &syn::UseTree, imports: &mut HashMap<String, String>) {
    match tree {
        syn::UseTree::Path(path) => {
            let next = if prefix.is_empty() {
                path.ident.to_string()
            } else {
                format!("{prefix}::{}", path.ident)
            };
            flatten_use_tree(next, &path.tree, imports);
        }
        syn::UseTree::Name(name) => {
            let full = if prefix.is_empty() {
                name.ident.to_string()
            } else {
                format!("{prefix}::{}", name.ident)
            };
            imports.insert(name.ident.to_string(), full);
        }
        syn::UseTree::Rename(rename) => {
            let full = if prefix.is_empty() {
                rename.ident.to_string()
            } else {
                format!("{prefix}::{}", rename.ident)
            };
            imports.insert(rename.rename.to_string(), full);
        }
        syn::UseTree::Group(group) => {
            for item in &group.items {
                flatten_use_tree(prefix.clone(), item, imports);
            }
        }
        syn::UseTree::Glob(_) => {}
    }
}

#[derive(Default)]
struct ConfigHintVisitor {
    hints: Vec<ConfigHint>,
}

impl<'ast> Visit<'ast> for ConfigHintVisitor {
    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        if matches!(node.method.to_string().as_str(), "get" | "get_value")
            && let Some(Expr::Lit(ExprLit { lit: Lit::Str(key), .. })) = node.args.first()
        {
            let rust_type = node.turbofish.as_ref().and_then(|arguments| {
                arguments.args.iter().find_map(|argument| match argument {
                    GenericArgument::Type(ty) => Some(type_to_string(ty)),
                    _ => None,
                })
            });
            self.hints.push(ConfigHint {
                key: key.value(),
                rust_type,
                default_ron: None,
                documentation: None,
            });
        }
        syn::visit::visit_expr_method_call(self, node);
    }
}

fn deduplicate_hints(hints: &mut Vec<ConfigHint>) {
    let mut unique = BTreeMap::new();
    for hint in hints.drain(..) {
        unique.entry(hint.key.clone()).or_insert(hint);
    }
    hints.extend(unique.into_values());
}

struct ChannelSetSyntax {
    #[allow(dead_code)]
    visibility: Visibility,
    name: syn::Ident,
    #[allow(dead_code)]
    id_type: syn::Ident,
    channels: Vec<ChannelSyntax>,
}

struct ChannelSyntax {
    id: syn::Ident,
    payload: Type,
    route: Option<LitStr>,
}

impl Parse for ChannelSetSyntax {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let visibility = input.parse::<Visibility>()?;
        input.parse::<Token![struct]>()?;
        let name = input.parse()?;
        input.parse::<Token![:]>()?;
        let id_type = input.parse()?;
        let content;
        braced!(content in input);
        let channels = Punctuated::<ChannelSyntax, Token![,]>::parse_terminated(&content)?
            .into_iter()
            .collect();
        Ok(Self {
            visibility,
            name,
            id_type,
            channels,
        })
    }
}

impl Parse for ChannelSyntax {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.peek(syn::token::Bracket) {
            let ignored;
            syn::bracketed!(ignored in input);
            let _: proc_macro2::TokenStream = ignored.parse()?;
        }
        let id = input.parse()?;
        input.parse::<Token![=>]>()?;
        let payload = input.parse()?;
        let route = if input.peek(Token![=]) {
            input.parse::<Token![=]>()?;
            Some(input.parse()?)
        } else {
            None
        };
        Ok(Self { id, payload, route })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, HashSet};

    #[test]
    fn splits_lifetime_multi_port_macro() {
        let ty: Type = syn::parse_str("output_msg!('m, Left, Right)").unwrap();
        assert_eq!(extract_message_types(&ty), vec!["Left", "Right"]);
    }

    #[test]
    fn parses_bridge_channels() {
        let syntax: ChannelSetSyntax = syn::parse_str(
            r#"pub struct Tx : TxId { left => common::Cmd = "motor/left", right => common::Cmd, }"#,
        )
        .unwrap();
        assert_eq!(syntax.name, "Tx");
        assert_eq!(syntax.channels.len(), 2);
        assert_eq!(syntax.channels[0].route.as_ref().unwrap().value(), "motor/left");
    }

    #[test]
    fn indexes_tasks_aliases_bridges_monitors_and_config_hints() {
        let temp = tempfile::tempdir().unwrap();
        let source_root = temp.path().join("src");
        fs::create_dir_all(&source_root).unwrap();
        fs::write(
            source_root.join("lib.rs"),
            r#"
                use cu29::prelude::*;
                pub struct Payload;
                pub type TaggedPayload = Payload;
                pub struct Source;
                impl CuSrcTask for Source {
                    type Output<'m> = output_msg!('m, TaggedPayload);
                    fn new(config: Option<&ComponentConfig>) -> CuResult<Self> {
                        let _ = config.unwrap().get::<u32>("rate");
                        Ok(Self)
                    }
                }
                pub struct Sink;
                impl CuSinkTask for Sink {
                    type Input<'m> = input_msg!('m, Payload);
                }
                pub struct Monitor;
                impl CuMonitor for Monitor {}
                mod inner {
                    use super::*;
                    pub struct Reexported;
                    impl CuSinkTask for Reexported { type Input<'m> = input_msg!('m, Payload); }
                }
                pub use inner::Reexported;
                tx_channels! { pub struct Tx : TxId { command => Payload = "cmd", } }
                rx_channels! { pub struct Rx : RxId { state => Payload = "state", } }
                pub struct GenericBridge<T, R>(T, R);
                impl<T, R> CuBridge for GenericBridge<T, R> {
                    type Tx = T;
                    type Rx = R;
                }
                pub type ConcreteBridge = GenericBridge<Tx, Rx>;
            "#,
        )
        .unwrap();
        let manifest = temp.path().join("Cargo.toml");
        fs::write(&manifest, "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\n")
            .unwrap();
        let project = ProjectInfo {
            opened_path: temp.path().to_path_buf(),
            app_manifest: manifest.clone(),
            workspace_manifest: manifest.clone(),
            package_name: "fixture".into(),
            config_path: temp.path().join("copperconfig.ron"),
            source_packages: vec![SourcePackage {
                package_name: "fixture".into(),
                crate_name: "fixture".into(),
                manifest_path: manifest,
                source_root,
                workspace_only: false,
            }],
            direct_dependencies: HashSet::new(),
            workspace_dependencies: BTreeMap::new(),
            metadata_warning: None,
        };

        let catalog = ComponentCatalog::index(&project);
        let source = catalog.find("Source").unwrap();
        let sink = catalog.find("Sink").unwrap();
        assert_eq!(source.kind, ComponentKind::Source);
        assert_eq!(source.outputs[0].canonical_type, sink.inputs[0].canonical_type);
        assert_eq!(source.config_hints[0].key, "rate");
        let bridge = catalog.find("ConcreteBridge").unwrap();
        assert_eq!(bridge.channels.len(), 2);
        assert_eq!(bridge.inputs.len(), 1);
        assert_eq!(bridge.outputs.len(), 1);
        assert_eq!(catalog.find("Monitor").unwrap().kind, ComponentKind::Monitor);
        assert_eq!(catalog.find("Reexported").unwrap().type_path, "Reexported");
    }
}
