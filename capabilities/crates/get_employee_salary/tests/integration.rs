use capability_common::EmployeeDatabase;
use get_employee_salary::get_salary_from_db;

#[test]
fn test_get_salary_maria() {
    let db = EmployeeDatabase::default_database();
    let salary = get_salary_from_db(&db, "EMP002").unwrap();
    assert_eq!(salary.currency, "USD");
    assert_eq!(salary.bonus_target_percent, 20);
}

#[test]
fn test_get_salary_david() {
    let db = EmployeeDatabase::default_database();
    let salary = get_salary_from_db(&db, "EMP003").unwrap();
    assert_eq!(salary.base_salary, 85000);
    assert_eq!(salary.bonus_target_percent, 10);
}
