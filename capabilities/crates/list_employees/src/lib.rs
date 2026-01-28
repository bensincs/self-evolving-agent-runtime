use capability_common::EmployeeDatabase;
use serde::Serialize;

/// Summary info for an employee.
#[derive(Debug, Clone, Serialize)]
pub struct EmployeeSummary {
    pub employee_id: String,
    pub name: String,
    pub department: String,
    pub job_title: String,
}

/// List all employees with basic info from a database.
pub fn list_employees_from_db(db: &EmployeeDatabase) -> Vec<EmployeeSummary> {
    db.employees
        .iter()
        .map(|emp| EmployeeSummary {
            employee_id: emp.employee_id.clone(),
            name: format!("{} {}", emp.profile.first_name, emp.profile.last_name),
            department: emp.profile.department.clone(),
            job_title: emp.profile.job_title.clone(),
        })
        .collect()
}

/// List all employees with basic info (loads from file).
pub fn list_employees() -> Vec<EmployeeSummary> {
    let db = EmployeeDatabase::load();
    list_employees_from_db(&db)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_employees() {
        let db = EmployeeDatabase::default_database();
        let employees = list_employees_from_db(&db);
        assert_eq!(employees.len(), 3);
    }

    #[test]
    fn test_list_employees_contains_john() {
        let db = EmployeeDatabase::default_database();
        let employees = list_employees_from_db(&db);
        let john = employees.iter().find(|e| e.employee_id == "EMP001");
        assert!(john.is_some());
        assert_eq!(john.unwrap().name, "John Smith");
    }
}
