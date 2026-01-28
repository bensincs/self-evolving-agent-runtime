use cap_98549::{update_salary_from_db, UpdateSalaryInput};
use capability_common::EmployeeDatabase;

#[test]
fn integration_update_salary() {
    let mut db = EmployeeDatabase::default_database();
    let input = UpdateSalaryInput {
        employee_id: "EMP002".into(),
        base_salary: Some(123456),
        currency: Some("EUR".into()),
    };
    let resp = update_salary_from_db(&mut db, input).unwrap();
    assert_eq!(resp.base_salary, 123456);
    assert_eq!(resp.currency, "EUR");
}
