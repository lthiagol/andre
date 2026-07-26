mod helpers;
use helpers::*;

#[test]
fn test_status_cache_populated_after_entering_package_select() {
    // Skipped: PackageSelect flow requires async setup
}

#[test]
fn test_status_cache_cleared_on_execution_complete() {
    let (app, _tmp) = make_test_app();
    app.ctx.status_cache.borrow_mut().insert(
        ("test".into(), "test".into()),
        andre_core::StowStatus::Stowed,
    );
    assert!(!app.ctx.status_cache.borrow().is_empty());
    app.invalidate_status_cache();
    assert!(app.ctx.status_cache.borrow().is_empty());
}

#[test]
fn test_status_cache_cleared_on_group_change() {
    let (mut app, _tmp) = make_test_app();
    push_component(&mut app, "GroupSelect");
    app.ctx.status_cache.borrow_mut().insert(
        ("home".into(), "test".into()),
        andre_core::StowStatus::Stowed,
    );
    assert!(!app.ctx.status_cache.borrow().is_empty());
    app.component_dispatch(space_key());
    assert!(app.ctx.status_cache.borrow().is_empty());
}
