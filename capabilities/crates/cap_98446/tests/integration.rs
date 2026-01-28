use cap_98446::greet;

#[test]
fn integration_greet() {
    let msg = greet();
    assert_eq!(msg, "Hello! How can I help you today?");
}
