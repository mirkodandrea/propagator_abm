//! Connect egui's widget tree and actions to the existing Bevy desktop adapter.
//! bevy_egui 0.28 does not forward accessibility output. Its AccessKit 0.12
//! nodes need checked/toggled and retired-role translations for Bevy’s 0.14.
//! Keep that version boundary here, with a widget-tree regression test.

use bevy::a11y::{AccessibilityRequested, ActionRequest, ManageAccessibilityUpdates};
use bevy::prelude::*;
use bevy::winit::accessibility::AccessKitAdapters;
use bevy_egui::{egui, EguiContext, EguiInput, EguiOutput, EguiSet};

pub struct NativeAccessibilityPlugin;

impl Plugin for NativeAccessibilityPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PreUpdate,
            input
                .after(EguiSet::ProcessInput)
                .before(EguiSet::BeginFrame),
        )
        .add_systems(PostUpdate, output.after(EguiSet::ProcessOutput));
    }
}

fn input(
    requested: Res<AccessibilityRequested>,
    mut managed: ResMut<ManageAccessibilityUpdates>,
    mut contexts: Query<(&mut EguiContext, &mut EguiInput)>,
    mut actions: EventReader<ActionRequest>,
) {
    // egui owns the entire window's widget tree; do not let the ECS adapter
    // overwrite it with an empty Bevy UI tree on the following frame.
    managed.set(false);
    for (mut ctx, mut input) in &mut contexts {
        if requested.get() {
            ctx.get_mut().enable_accesskit();
        }
        for action in actions.read() {
            match serde_json::to_value(&action.0).and_then(serde_json::from_value) {
                Ok(action) => input
                    .events
                    .push(egui::Event::AccessKitActionRequest(action)),
                Err(error) => warn!("Unsupported accessibility action: {error}"),
            }
        }
    }
}

fn convert_node(node: &accesskit_egui::Node) -> Result<accesskit_native::Node, serde_json::Error> {
    let mut value = serde_json::to_value(node)?;
    if let Some(object) = value.as_object_mut() {
        let role = match object.get("role").and_then(|role| role.as_str()) {
            Some("toggleButton") => Some("button"),
            Some("window" | "column" | "tableHeaderContainer") => Some("genericContainer"),
            _ => None,
        };
        if let Some(role) = role {
            object.insert("role".into(), serde_json::Value::String(role.into()));
        }
        if let Some(checked) = object.remove("checked") {
            object.insert("toggled".into(), checked);
        }
    }
    serde_json::from_value(value)
}

fn output(
    mut adapters: NonSendMut<AccessKitAdapters>,
    mut windows: Query<(Entity, &Window, &mut EguiOutput)>,
) {
    use accesskit_native::{NodeBuilder, NodeId, Role, Tree, TreeUpdate};
    for (entity, window, mut output) in &mut windows {
        let Some(update) = output.platform_output.accesskit_update.take() else {
            continue;
        };
        let Some(adapter) = adapters.get_mut(&entity) else {
            continue;
        };
        let Some(tree) = update.tree else { continue };
        let nodes: Result<Vec<_>, _> = update
            .nodes
            .iter()
            .map(|(id, node)| convert_node(node).map(|node| (NodeId(id.0), node)))
            .collect();
        let mut nodes = match nodes {
            Ok(nodes) => nodes,
            Err(error) => {
                warn!("Accessibility tree conversion failed: {error}");
                continue;
            }
        };
        // Preserve the native window root installed by Bevy at activation.
        let root_id = NodeId(entity.to_bits());
        let mut root = NodeBuilder::new(Role::Window);
        root.set_name(window.title.clone());
        root.set_children(vec![NodeId(tree.root.0)]);
        nodes.push((root_id, root.build()));
        adapter.update_if_active(|| TreeUpdate {
            nodes,
            tree: Some(Tree::new(root_id)),
            focus: NodeId(update.focus.0),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widget_tree_converts_labels_actions_and_checked_state() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut checked = true;
        let mut text = String::from("Squadra A");
        let mut speed = 1.0;
        let result = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("Incident response");
                let _ = ui.button("Evacuate everyone");
                ui.checkbox(&mut checked, "Units");
                let _ = ui.selectable_label(true, "Squadra A");
                ui.collapsing("Roster", |ui| { ui.label("Ready"); });
                ui.menu_button("Operations", |ui| { let _ = ui.button("Evacuate"); });
                ui.text_edit_singleline(&mut text);
                ui.add(egui::Slider::new(&mut speed, 1.0..=8.0));
            });
        });
        let update = result.platform_output.accesskit_update.unwrap();
        assert!(update.nodes.len() > 5);
        let nodes: Vec<_> = update
            .nodes
            .iter()
            .map(|(_, node)| convert_node(node).unwrap())
            .collect();
        assert!(nodes
            .iter()
            .any(|node| node.name() == Some("Evacuate everyone")));
        assert!(nodes
            .iter()
            .any(|node| node.toggled() == Some(accesskit_native::Toggled::True)));
    }
}

#[cfg(test)]
mod action_tests {
    use super::*;

    #[test]
    fn native_press_reaches_the_egui_button() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let first = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = ui.button("Play");
            });
        });
        let update = first.platform_output.accesskit_update.unwrap();
        let id = update
            .nodes
            .iter()
            .find(|(_, node)| node.name() == Some("Play"))
            .unwrap()
            .0;
        let request = accesskit_native::ActionRequest {
            action: accesskit_native::Action::Default,
            target: accesskit_native::NodeId(id.0),
            data: None,
        };
        let request = serde_json::from_value(serde_json::to_value(request).unwrap()).unwrap();
        let mut input = egui::RawInput::default();
        input
            .events
            .push(egui::Event::AccessKitActionRequest(request));
        let mut clicked = false;
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                clicked = ui.button("Play").clicked();
            });
        });
        assert!(clicked);
    }
}
