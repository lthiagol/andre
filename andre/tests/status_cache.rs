mod helpers;
use helpers::*;

use andre::app::App;
use andre::components::execute::ExecuteComponent;
use andre::components::MainMenuComponent;
use andre::execute::ExecutionUpdate;

#[test]
fn test_status_cache_populated_after_entering_packages() {
    let (mut app, _tmp) = make_test_app();
    app.component_dispatch(key_char('2'));
    app.update_components();
    assert!(app.ctx.status_cache.borrow().get("test").is_some());
}

#[test]
fn test_status_cache_cleared_on_execution_complete() {
    let (app, _tmp) = make_test_app();
    app.ctx
        .status_cache
        .borrow_mut()
        .insert("test".into(), andre_core::StowStatus::Stowed);
    assert!(!app.ctx.status_cache.borrow().is_empty());
    app.invalidate_status_cache();
    assert!(app.ctx.status_cache.borrow().is_empty());
}

#[test]
fn test_marking_a_package_keeps_the_status_cache() {
    let (mut app, _tmp) = make_test_app();
    push_component(&mut app, "Packages");
    app.update_components();
    app.ctx
        .status_cache
        .borrow_mut()
        .insert("test".into(), andre_core::StowStatus::Stowed);
    // Marking is a UI action: the cached status is kept, not read again from disk.
    app.component_dispatch(space_key());
    assert_eq!(
        app.ctx.status_cache.borrow().get("test"),
        Some(andre_core::StowStatus::Stowed)
    );
}

/// The stowed count shown on the main menu, read from its detail facts.
fn menu_stowed_fact(app: &App) -> String {
    let menu = app.component_stack[0]
        .as_any()
        .downcast_ref::<MainMenuComponent>()
        .expect("main menu at the bottom of the stack");
    menu.detail()
        .facts
        .iter()
        .find(|(label, _)| *label == "stowed")
        .map(|(_, value)| value.clone())
        .expect("stowed fact")
}

#[test]
fn test_menu_stowed_count_is_unknown_after_an_execution_finishes() {
    let (mut app, _tmp) = make_test_app();
    app.ctx
        .status_cache
        .borrow_mut()
        .insert("test".into(), andre_core::StowStatus::Stowed);
    app.update_components();
    assert_eq!(menu_stowed_fact(&app), "1 of 1");

    // An execution runs to completion on its own screen, then the user returns to the menu.
    let (tx, rx) = tokio::sync::mpsc::channel(1);
    tx.try_send(ExecutionUpdate::Complete).unwrap();
    app.component_stack
        .push(Box::new(ExecuteComponent::new(rx, 1, Vec::new())));
    app.update_components();
    app.component_dispatch(enter_key());
    app.update_components();

    assert_eq!(menu_stowed_fact(&app), "\u{2013}");
}
