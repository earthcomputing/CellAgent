use std::collections::HashMap;
use serde_json;
use eval::{eval, Expr, to_value};

#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct GvmEquation {
	equation: String,
	variables: GvmVariables
}
// Sample GvmEquation: "hops < 7 || n_childen == 0", ["hops", "n_children"]
impl GvmEquation {
	pub fn new(s: &str, variables: GvmVariables) -> GvmEquation { 
		GvmEquation { equation: s.to_string(), variables: variables }
	}
	pub fn evaluate(&self, params: HashMap<String,String>) -> Result<bool> {
		let mut expr = Expr::new(self.equation.clone());
		for (variable, value) in params {
			let test: bool = serde_json::from_str(&value).chain_err(|| ErrorKind::GvmEquationError)?;
			expr = expr.clone().value(variable,test);
		}
		let result = expr.exec().chain_err(|| ErrorKind::GvmEquationError)?;
		Ok(result == to_value(true))
	}
	pub fn get_variables(&self) -> &GvmVariables { &self.variables }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct GvmVariables {
	variables: Vec<String>
}
impl GvmVariables {
	pub fn new(strs: Vec<&str>) -> GvmVariables {
		let variables = strs.iter().map(|s| s.to_string()).collect();
		GvmVariables { variables: variables }
	}
	pub fn empty() -> GvmVariables {
		GvmVariables { variables: Vec::new() }
	}
	pub fn iter(&self) -> ::std::slice::Iter<String> { self.variables.iter() } 
}
error_chain! {
	foreign_links {
		Eval(::eval::Error);
		Convert(serde_json::Error);
	}
	errors {
		GvmEquationError
	}
}