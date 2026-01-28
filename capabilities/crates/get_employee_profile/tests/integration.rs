use capability_common::EmployeeDatabase;
use get_employee_profile::get_profile_from_db;

#[test]
fn test_get_profile_maria() {
    let db = EmployeeDatabase::default_database();
    let profile = get_profile_from_db(&db, "EMP002").unwrap();
    assert_eq!(profile.first_name, "Maria");
    assert_eq!(profile.last_name, "Garcia");
    assert_eq!(profile.department, "Marketing");
    assert_eq!(profile.job_title, "Marketing Manager");
}

#[test]
fn test_get_profile_david() {
    let db = EmployeeDatabase::default_database();
    let profile = get_profile_from_db(&db, "EMP003").unwrap();
    assert_eq!(profile.first_name, "David");
    assert_eq!(profile.last_name, "Chen");
    assert_eq!(profile.department, "Finance");
}
