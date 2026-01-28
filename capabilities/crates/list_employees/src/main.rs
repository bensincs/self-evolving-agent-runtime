use capability_common::serde_json::Value;
use list_employees::list_employees;

fn main() {
    capability_common::run(|_input: Value| {
        Ok(list_employees())
    });
}
