#[cfg(test)]
const CONTROL_ROLES: [gpui::Role; 9] = [
    gpui::Role::Button,
    gpui::Role::Switch,
    gpui::Role::CheckBox,
    gpui::Role::RadioButton,
    gpui::Role::Tab,
    gpui::Role::ComboBox,
    gpui::Role::MenuItem,
    gpui::Role::Link,
    gpui::Role::TabList,
];

#[cfg(test)]
pub(crate) fn unnamed_controls(window: &gpui::Window) -> Vec<String> {
    let mut problems: Vec<String> = window
        .last_a11y_unexposed_clicks()
        .iter()
        .map(|location| match location {
            Some(location) => format!("clickable without a role at {location}"),
            None => "clickable without a role".to_string(),
        })
        .collect();
    let provenance: serde_json::Value = window
        .debug_a11y_tree_json()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default();
    let element_of = |id: gpui::accesskit::NodeId| {
        provenance["nodes"]
            .as_object()
            .and_then(|nodes| {
                nodes
                    .values()
                    .find(|node| node["accesskit_id"] == id.0.to_string())
            })
            .and_then(|node| node["element_id"].as_str())
            .unwrap_or("an element without an id")
            .to_string()
    };
    if let Some(update) = window.last_a11y_tree_update() {
        problems.extend(
            update
                .nodes
                .iter()
                .filter(|(_, node)| {
                    (node.supports_action(gpui::AccessibleAction::Click)
                        || CONTROL_ROLES.contains(&node.role()))
                        && node.label().is_none_or(|label| label.trim().is_empty())
                })
                .map(|(id, node)| format!("{:?} {} has no name", node.role(), element_of(*id))),
        );
    }
    problems.sort();
    problems.dedup();
    problems
}

#[cfg(test)]
pub(crate) fn missing_ids(window: &gpui::Window, ids: &[&str]) -> Vec<String> {
    let present: Vec<&str> = window
        .last_a11y_tree_update()
        .map(|update| {
            update
                .nodes
                .iter()
                .filter(|(_, node)| node.label().is_some_and(|label| !label.trim().is_empty()))
                .filter_map(|(_, node)| node.author_id())
                .collect()
        })
        .unwrap_or_default();
    ids.iter()
        .filter(|id| !present.contains(id))
        .map(|id| format!("no named control with the id {id}"))
        .collect()
}

#[cfg(test)]
pub(crate) fn unnamed_controls_after_drawing(cx: &mut gpui::VisualTestContext) -> Vec<String> {
    cx.update(|window, _| window.activate_a11y_for_test());
    cx.run_until_parked();
    cx.update(|window, _| unnamed_controls(window))
}

#[cfg(test)]
pub(crate) fn assert_every_control_is_named(cx: &mut gpui::VisualTestContext, ids: &[&str]) {
    cx.update(|window, _| window.activate_a11y_for_test());
    cx.run_until_parked();
    let problems = cx.update(|window, _| {
        let mut problems = unnamed_controls(window);
        problems.extend(missing_ids(window, ids));
        problems
    });
    assert!(
        problems.is_empty(),
        "{} controls are missing from the accessibility tree or have no name:\n{}",
        problems.len(),
        problems.join("\n")
    );
}
