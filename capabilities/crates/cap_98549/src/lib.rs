use capability_common::{EmployeeDatabase, CapabilityError};
use serde::{Serialize, Deserialize};

#[derive(Debug, Deserialize)]
pub struct UpdateSalaryInput {
    pub employee_id: String,
    pub base_salary: Option<u32>,
    pub currency: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UpdateSalaryResponse {
    pub employee_id: String,
    pub base_salary: u32,
    pub currency: String,
}

pub fn update_salary_from_db(db: &mut EmployeeDatabase, input: UpdateSalaryInput) -> Result<UpdateSalaryResponse, CapabilityError> {
    let emp = db.employees.iter_mut().find(|e| e.employee_id == input.employee_id)
        .ok_or_else(|| CapabilityError::new("Employee not found"))?;
    if let Some(s) = input.base_salary { emp.salary.base_salary = s; }
    if let Some(c) = input.currency { emp.salary.currency = c; }
    Ok(UpdateSalaryResponse {
        employee_id: emp.employee_id.clone(),
        base_salary: emp.salary.base_salary,
        currency: emp.salary.currency.clone(),
    })
}

pub fn update_salary(input: UpdateSalaryInput) -> Result<UpdateSalaryResponse, CapabilityError> {
    let mut db = EmployeeDatabase::load();
    update_salary_from_db(&mut db, input)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_update_salary() {
        let mut db = EmployeeDatabase::default_database();
        let input = UpdateSalaryInput{ employee_id: "EMP001".into(), base_salary: Some(200000), currency: None};
        let resp = update_salary_from_db(&mut db, input).unwrap();
        assert_eq!(resp.base_salary, 200000);
    }
}
