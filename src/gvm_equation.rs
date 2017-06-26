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
	pub fn evaluate(&self, params: HashMap<&str,&str>) -> Result<bool> {
		let mut expr = Expr::new(self.equation.clone());
		for (variable, value) in params {
			let test: bool = serde_json::from_str(value).chain_err(|| ErrorKind::GvmEquationError)?;
			expr = expr.clone().value(variable,test);
		}
		let result = serde_json::from_str(&expr.exec().
			chain_err(|| ErrorKind::GvmEquationError)?.
			to_string()).chain_err(|| ErrorKind::GvmEquationError)?;
		Ok(result)
	}
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct GvmVariables {
	variables: Vec<String>
}
impl GvmVariables {
	pub fn new(variables: Vec<String>) -> GvmVariables {
		GvmVariables { variables: variables }
	}
	pub fn empty() -> GvmVariables {
		GvmVariables { variables: Vec::new() }
	}
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