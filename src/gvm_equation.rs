use std::collections::HashMap;
use std::fmt;
use serde_json;
use eval::{eval, Expr, to_value};

use config::CellNo;

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
	pub fn eval_recv(&self, params: &HashMap<GvmVariable, String>) -> Result<bool> {
		self.evaluate(&self.recv_eqn, params)
	}
	pub fn eval_send(&self, params: &HashMap<GvmVariable, String>) -> Result<bool> {
		self.evaluate(&self.send_eqn, params)
	}
	fn evaluate(&self, eqn: &GvmEqn, params: &HashMap<GvmVariable,String>) -> Result<bool> {
		let mut expr = Expr::new(eqn.clone());
		for (variable, value) in params.iter() {
			let var_type = variable.get_type();
			let str_value = variable.get_value();
			let value = match *var_type {
				GvmVariableType::CellNo => {
					let val: CellNo = serde_json::from_str(&str_value).chain_err(|| ErrorKind::GvmEquationError)?;
				}
			};
			expr = expr.clone().value(variable.get_value(), value);
		}
		let result = expr.exec().chain_err(|| ErrorKind::GvmEquationError)?;
		Ok(result == to_value(true))
	}
	pub fn get_variables(&self) -> &GvmVariables { &self.variables }
}
impl fmt::Display for GvmEquation {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "GvmEquation: receive {}, send {}, variables {}", 
			self.recv_eqn, self.send_eqn, self.variables)
	}
}
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum GvmVariableType {
	CellNo
}
impl fmt::Display for GvmVariableType {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let s = match *self {
			GvmVariableType::CellNo => "CellNo"
		};
		write!(f, "Type {}", s)
	}
}
#[derive(Debug, Clone, Eq, PartialEq,Hash, Serialize, Deserialize)]
pub struct GvmVariable { var_type: GvmVariableType, value: String }
impl GvmVariable {
	pub fn new<T: fmt::Display>(var_type: GvmVariableType, value: T) -> GvmVariable {
		GvmVariable { var_type: var_type, value: value.to_string() }
	}
	pub fn get_type(&self) -> &GvmVariableType { &self.var_type }
	pub fn get_value(&self) -> String { self.value.clone() }
}
impl fmt::Display for GvmVariable { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		write!(f, "{}, value {}", self.var_type, self.value) }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct GvmVariables {
	variables: Vec<GvmVariable>, // e.g., ("CellNo", "hops") 
}
impl GvmVariables {
	pub fn new() -> GvmVariables {
		GvmVariables { variables: Vec::new() }
	}
	pub fn add(&mut self, var: GvmVariable) { self.variables.push(var); }
	pub fn get_variables(&self) -> &Vec<GvmVariable> { &self.variables }
}
impl fmt::Display for GvmVariables {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let mut s = format!("Gvm Variables: ");
		for variable in &self.variables {
			s = s + &format!("{} ", variable);
		}
		write!(f, "{}", s)
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