use std::fmt;
use serde_json;
use std::collections::{HashSet};
use eval::{Expr, to_value};

use failure::Error;

use config::{CellNo, PathLength};
use utility::S;

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum GvmEqn<'a> {
	Recv(&'a str),
	Send(&'a str),
	Xtnd(&'a str),
	Save(&'a str),
}

type GvmEqnType = String;
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct GvmEquation {
	recv_eqn: GvmEqnType,        // If true, add to traph and set "up" bit
	send_eqn: GvmEqnType,        // If true, add to traph
	save_eqn: GvmEqnType,		 // If true, save the message for future ports being connected
	xtnd_eqn: GvmEqnType,		 // If true, propagate message
	variables: Vec<GvmVariable>  // Local variables used in the two equations
}
// Sample GvmEquation: "hops < 7 || n_childen == 0",  associated variables vec!["hops", "n_children"]
impl GvmEquation {
	pub fn new(equations: HashSet<GvmEqn>, variables: Vec<GvmVariable>) -> GvmEquation { 
		let (mut recv, mut send, mut xtnd, mut save) = (S("false"), S("false"), S("false"), S("false")); 
		for eqn in equations.iter() {
			match *eqn {
				GvmEqn::Recv(s) => recv = S(s),
				GvmEqn::Send(s) => send = S(s),
				GvmEqn::Xtnd(s) => xtnd = S(s),
				GvmEqn::Save(s) => save = S(s),
			}
		}
		GvmEquation { recv_eqn: recv, send_eqn: send, 
			save_eqn: save, xtnd_eqn: xtnd, variables: variables }
	}
	pub fn get_variables(&self) -> &Vec<GvmVariable> { &self.variables }
	pub fn eval_recv(&self, params: &Vec<GvmVariable>) -> Result<bool, Error> {
		self.evaluate(&self.recv_eqn, params)
	}
	pub fn eval_send(&self, params: &Vec<GvmVariable>) -> Result<bool, Error> {
		self.evaluate(&self.send_eqn, params)
	}
	pub fn eval_save(&self, params: &Vec<GvmVariable>) -> Result<bool, Error> {
		self.evaluate(&self.save_eqn, params)
	}
	pub fn eval_xtnd(&self, params: &Vec<GvmVariable>) -> Result<bool, Error> {
		self.evaluate(&self.xtnd_eqn, params)
	}
	fn evaluate(&self, eqn: &GvmEqnType, params: &Vec<GvmVariable>) -> Result<bool, Error> {
		let mut expr = Expr::new(eqn.clone());
		for variable in params.iter() {
			let var_type = variable.get_type();
			let str_value = variable.get_value();
			match *var_type {
				GvmVariableType::CellNo => {
					let value = serde_json::from_str::<CellNo>(&str_value)?;
					expr = expr.clone().value(variable.get_value(), value);					
				},
				GvmVariableType::PathLength => {
					let value = serde_json::from_str::<PathLength>(&str_value)?;
					expr = expr.clone().value(variable.get_value(), value);
				},
			};
		}
		let result = expr.exec()?;
		Ok(result == to_value(true))
	}
}
impl fmt::Display for GvmEquation {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let mut s = format!("GVM: receive '{}', send '{}', extend '{}', save '{}', Variables:", 
			self.recv_eqn, self.send_eqn, self.xtnd_eqn, self.save_eqn);
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
		write!(f, "{}:{}", self.value, self.var_type) }
}
