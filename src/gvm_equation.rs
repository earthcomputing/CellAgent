use std::collections::HashMap;
use serde_json;
use eval::{eval, Expr, to_value};

type GvmEqn = String;
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct GvmEquation {
	recv_eqn: GvmEqn,        // If true, then send to cell agent
	send_eqn: GvmEqn,        // If true, then add to traph
	variables: GvmVariables  // Local variables used in the two equations
}
// Sample GvmEquation: "hops < 7 || n_childen == 0", ["hops", "n_children"]
impl GvmEquation {
	pub fn new(recv: &str, send: &str, variables: GvmVariables) -> GvmEquation { 
		GvmEquation { recv_eqn: recv.to_string(), send_eqn: send.to_string(), variables: variables }
	}
	pub fn get_recv_eqn(&self) -> &GvmEqn { &self.recv_eqn }
	pub fn get_send_eqn(&self) -> &GvmEqn { &self.send_eqn }
	pub fn eval_recv(&self, params: HashMap<String, String>) -> Result<bool> {
		self.evaluate(&self.recv_eqn, params)
	}
	pub fn eval_send(&self, params: HashMap<String, String>) -> Result<bool> {
		self.evaluate(&self.send_eqn, params)
	}
	fn evaluate(&self, eqn: &GvmEqn, params: HashMap<String,String>) -> Result<bool> {
		let mut expr = Expr::new(eqn.clone());
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