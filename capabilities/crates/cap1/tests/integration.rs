use cap1::run;

#[test]
fn test_add_positive() {
    assert_eq!(run(2, 3), 5);
}

#[test]
fn test_add_negative() {
    assert_eq!(run(-5, 7), 2);
}

#[test]
fn test_add_zeros() {
    assert_eq!(run(0, 0), 0);
}
