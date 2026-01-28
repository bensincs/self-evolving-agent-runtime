use capability_common::{CapabilityError, EmployeeDatabase, SalaryDetails};

/// Get the salary details for an employee by ID from a database.
pub fn get_salary_from_db(db: &EmployeeDatabase, employee_id: &str) -> Result<SalaryDetails, CapabilityError> {
    db.find_employee(employee_id)
        .map(|emp| emp.salary.clone())
        .ok_or_else(|| CapabilityError::new(format!("Employee not found: {}", employee_id)))
}

/// Get the salary details for an employee by ID (loads from file).
pub fn get_salary(employee_id: &str) -> Result<SalaryDetails, CapabilityError> {
    let db = EmployeeDatabase::load();
    get_salary_from_db(&db, employee_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_salary_john() {
        let db = EmployeeDatabase::default_database();
        let salary = get_salary_from_db(&db, "EMP001").unwrap();
        assert_eq!(salary.currency, "USD");
        assert!(salary.base_salary > 0);
    }

    #[test]
    fn test_get_salary_not_found() {
        let db = EmployeeDatabase::default_database();
        let result = get_salary_from_db(&db, "INVALID");
        assert!(result.is_err());
    }
}
