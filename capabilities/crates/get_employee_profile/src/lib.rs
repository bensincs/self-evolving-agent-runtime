use capability_common::{CapabilityError, EmployeeDatabase, EmployeeProfile};

/// Get the profile for an employee by ID from a database.
pub fn get_profile_from_db(
    db: &EmployeeDatabase,
    employee_id: &str,
) -> Result<EmployeeProfile, CapabilityError> {
    db.find_employee(employee_id)
        .map(|emp| emp.profile.clone())
        .ok_or_else(|| CapabilityError::new(format!("Employee not found: {}", employee_id)))
}

/// Get the profile for an employee by ID (loads from file).
pub fn get_profile(employee_id: &str) -> Result<EmployeeProfile, CapabilityError> {
    let db = EmployeeDatabase::load();
    get_profile_from_db(&db, employee_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_profile_john() {
        let db = EmployeeDatabase::default_database();
        let profile = get_profile_from_db(&db, "EMP001").unwrap();
        assert_eq!(profile.first_name, "John");
        assert_eq!(profile.last_name, "Smith");
        assert_eq!(profile.department, "Engineering");
    }

    #[test]
    fn test_get_profile_not_found() {
        let db = EmployeeDatabase::default_database();
        let result = get_profile_from_db(&db, "INVALID");
        assert!(result.is_err());
    }
}
