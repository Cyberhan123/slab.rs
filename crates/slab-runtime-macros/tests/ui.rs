#[test]
fn backend_handler_ui() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/backend_handler_ok.rs");
    cases.compile_fail("tests/ui/backend_handler_duplicate_route.rs");
    cases.compile_fail("tests/ui/backend_handler_wrong_arg_count.rs");
    cases.compile_fail("tests/ui/backend_handler_non_async.rs");
    cases.compile_fail("tests/ui/backend_handler_bare_peer_conflict.rs");
    cases.compile_fail("tests/ui/backend_handler_missing_peer_emitter.rs");
    cases.compile_fail("tests/ui/backend_handler_missing_constructor.rs");
    cases.compile_fail("tests/ui/backend_handler_typed_missing_result.rs");
    cases.compile_fail("tests/ui/backend_handler_wrong_arg_type.rs");
}
