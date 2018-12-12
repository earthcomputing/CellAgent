use std::{fmt, collections::{HashSet}};

use eval::{Expr, to_value};
use serde_json;
use failure::{Error, ResultExt};

use crate::config::{CellNo, PathLength};
use crate::utility::S;

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
    send_eqn: GvmEqnType,        // If true, add to set maySend true in routing table entry
    save_eqn: GvmEqnType,        // If true, save the message for future traph updates
    xtnd_eqn: GvmEqnType,        // If false, turn off all ports in routing table entry
    variables: Vec<GvmVariable>  // Local variables used in the equations
}
// Sample GvmEquation: "hops < 7 || n_childen == 0",  associated variables vec!["hops", "n_children"]
impl GvmEquation {
    pub fn new(equations: &HashSet<GvmEqn<'_>>, variables: Vec<GvmVariable>) -> GvmEquation {
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
            save_eqn: save, xtnd_eqn: xtnd, variables: variables.clone() }
    }
    pub fn get_variables(&self) -> &[GvmVariable] { &self.variables }
    pub fn eval_recv(&self, params: &[GvmVariable]) -> Result<bool, Error> {
        self.evaluate(&self.recv_eqn, params)
    }
    pub fn eval_send(&self, params: &[GvmVariable]) -> Result<bool, Error> {
        self.evaluate(&self.send_eqn, params)
    }
    pub fn eval_save(&self, params: &[GvmVariable]) -> Result<bool, Error> {
        self.evaluate(&self.save_eqn, params)
    }
    pub fn eval_xtnd(&self, params: &[GvmVariable]) -> Result<bool, Error> {
        self.evaluate(&self.xtnd_eqn, params)
    }
    fn evaluate(&self, eqn: &GvmEqnType, params: &[GvmVariable]) -> Result<bool, Error> {
        let mut expr = Expr::new(eqn.clone());
        for variable in params.iter() {
            let var_type = variable.get_var_type();
            let var_name = S(variable.get_var_name());
            let str_val = S(variable.get_value());
            match *var_type {
                GvmVariableType::CellNo => {
                    let value = serde_json::from_str::<CellNo>(&str_val).context(GvmEquationError::Deserialize { func_name: "evaluate", var_type: var_type.clone(), expr: S(str_val) })?;
                    expr = expr.value(var_name, *value);
                },
                GvmVariableType::PathLength => {
                    let value = serde_json::from_str::<PathLength>(&str_val).context(GvmEquationError::Deserialize { func_name: "evaluate", var_type: var_type.clone(), expr: S(str_val) })?;
                    expr = expr.value(var_name, *value.0);
                    //println!("GvmEquation: expr {:?}, variable {}, second {}, result {:?}", expr, variable, *value.0, expr.exec());
                },
            }
        }
        let result = expr.exec().context(GvmEquationError::Eval { func_name: "evaluate", eqn: self.clone() })?;
        Ok(result == to_value(true))
    }
}
impl fmt::Display for GvmEquation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
    var_name: String,
    value:    String
}
impl GvmVariable {
    pub fn new(var_type: GvmVariableType, var_name: &str) -> GvmVariable {
        GvmVariable { var_type, var_name: var_name.to_string(), value: S("") }
    }
    pub fn get_var_type(&self) -> &GvmVariableType { &self.var_type }
    pub fn get_var_name(&self) -> &String { &self.var_name }
    pub fn get_value(&self)    -> &String { &self.value }
    pub fn set_value(&mut self, value: String) { self.value = value; }
}
impl fmt::Display for GvmVariable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{} = {}", self.var_type, self.var_name, self.value) }
}
#[derive(Debug, Fail)]
pub enum GvmEquationError {
    #[fail(display = "GvmEquationError::Eval {}: Equation {}", func_name, eqn)]
    Eval { func_name: &'static str, eqn: GvmEquation },
    #[fail(display = "GvmEquationError::Deserialize {}: Problem deserializing {} {}", func_name, var_type, expr)]
    Deserialize { func_name: &'static str, var_type: GvmVariableType, expr: String }
}
