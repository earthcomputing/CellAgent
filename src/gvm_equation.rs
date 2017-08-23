use std::collections::HashMap;
use std::fmt;
use serde_json;
use eval::{eval, Expr, to_value};

use config::{CellNo, PathLength};

type GvmEqn = String;
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct GvmEquation {
	recv_eqn: GvmEqn,        // If true, add to traph and set "up" bit
	send_eqn: GvmEqn,        // If true, add to traph
	save_eqn: GvmEqn,		 // If true, save the message for future ports being connected
	xtnd_eqn: GvmEqn,		 // If true, propagate message
	variables: Vec<GvmVariable>  // Local variables used in the two equations
}
// Sample GvmEquation: "hops < 7 || n_childen == 0", ["hops", "n_children"]
impl GvmEquation {
	pub fn new(recv: &str, send: &str, save: &str, xtnd: &str, variables: Vec<GvmVariable>) -> GvmEquation { 
		GvmEquation { recv_eqn: recv.to_string(), send_eqn: send.to_string(), 
			save_eqn: save.to_string(), xtnd_eqn: xtnd.to_string(), variables: variables }
	}
	pub fn get_variables(&self) -> &Vec<GvmVariable> { &self.variables }
	pub fn eval_recv(&self, params: &Vec<GvmVariable>) -> Result<bool> {
		self.evaluate(&self.recv_eqn, params)
	}
	pub fn eval_send(&self, params: &Vec<GvmVariable>) -> Result<bool> {
		self.evaluate(&self.send_eqn, params)
	}
	pub fn eval_save(&self, params: &Vec<GvmVariable>) -> Result<bool> {
		self.evaluate(&self.save_eqn, params)
	}
	pub fn eval_xtnd(&self, params: &Vec<GvmVariable>) -> Result<bool> {
		self.evaluate(&self.xtnd_eqn, params)
	}
	fn evaluate(&self, eqn: &GvmEqn, params: &Vec<GvmVariable>) -> Result<bool> {
		let mut expr = Expr::new(eqn.clone());
		for variable in params.iter() {
			let var_type = variable.get_type();
			let str_value = variable.get_value();
			match *var_type {
				GvmVariableType::CellNo => {
					let value = serde_json::from_str::<CellNo>(&str_value).chain_err(|| ErrorKind::GvmEquationError)?;
					expr = expr.clone().value(variable.get_value(), value);					
				},
				GvmVariableType::PathLength => {
					let value = serde_json::from_str::<PathLength>(&str_value).chain_err(|| ErrorKind::GvmEquationError)?;
					expr = expr.clone().value(variable.get_value(), value);
				},
			};
		}
		let result = expr.exec().chain_err(|| ErrorKind::GvmEquationError)?;
		Ok(result == to_value(true))
	}
}
impl fmt::Display for GvmEquation {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let mut s = format!("Gvm: receive {}, send {}, extend {}, save {}, Variables:", 
			self.recv_eqn, self.send_eqn, self.save_eqn, self.xtnd_eqn);
		for variable in self.variables.iter() {
			s = s + &format!(" {} ", variable);
		}
		write!(f, "{}", s)
	}
}
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum GvmVariableType {
	CellNo,
	PathLength
}
impl fmt::Display for GvmVariableType {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let s = match *self {
			GvmVariableType::CellNo => "CellNo",
			GvmVariableType::PathLength => "PathLength",
		};
		write!(f, "{}", s)
	}
}
#[derive(Debug, Clone, Eq, PartialEq,Hash, Serialize, Deserialize)]
pub struct GvmVariable { 
	var_type: GvmVariableType, 
	value: String 
}
impl GvmVariable {
	pub fn new<T: fmt::Display>(var_type: GvmVariableType, value: T) -> GvmVariable {
		GvmVariable { var_type: var_type, value: value.to_string() }
	}
	pub fn get_type(&self) -> &GvmVariableType { &self.var_type }
	pub fn get_value(&self) -> String { self.value.clone() }
}
impl fmt::Display for GvmVariable { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		write!(f, "{}::{}", self.value, self.var_type) }
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