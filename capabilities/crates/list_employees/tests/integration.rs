use capability_common::EmployeeDatabase;
use list_employees::list_employees_from_db;

#[test]
fn test_list_has_all_departments() {
    let db = EmployeeDatabase::default_database();
    let employees = list_employees_from_db(&db);
    let departments: Vec<&str> = employees.iter().map(|e| e.department.as_str()).collect();

    assert!(departments.contains(&"Engineering"));
    assert!(departments.contains(&"Marketing"));
    assert!(departments.contains(&"Finance"));
}

#[test]
fn test_list_employee_ids() {
    let db = EmployeeDatabase::default_database();
    let employees = list_employees_from_db(&db);
    let ids: Vec<&str> = employees.iter().map(|e| e.employee_id.as_str()).collect();

    assert!(ids.contains(&"EMP001"));
    assert!(ids.contains(&"EMP002"));
    assert!(ids.contains(&"EMP003"));
}
