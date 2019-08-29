use std::{cmp::max,
          collections::HashMap,
          fmt, fmt::Write,
          sync::{Mutex}
};

use actix_web::{web, Error, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use serde_json::Value;

type Size = usize;

pub fn cell_geometry(path: &str, state: web::Data<AppGeometry>, record: web::Json<Value>)
                 -> Result<impl Responder, Error> {
    let trace_body = record.get("body").expect("HelloMsg: bad trace record");
    let body: Body = serde_json::from_value(trace_body.clone())?;
    let name = body.cell_id.name;
    let location = body.location;
    let app_geometry = state.get_ref();
    app_geometry.geometry.lock().unwrap().add(CellID { name },
                                              Location { row: location[0], col: location[1] });
    Ok(HttpResponse::Ok().body(format!("{} adding {}\n{:?}\n", path, record, app_geometry)))
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Body {
    cell_id: CellID,
    location: [usize; 2]
}
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct CellID {
    name: String
}
#[derive(Debug, Default, Serialize)]
pub struct AppGeometry {
    geometry: Mutex<Geometry>
}
#[derive(Debug, Copy, Clone, Default, Eq, PartialEq, Serialize)]
pub struct Location { row: Size, col: Size }
impl Location {
    pub fn new(rowcol: [Size; 2]) -> Location { Location { row: rowcol[0], col: rowcol[1] } }
}

type RowCol = HashMap<String, Location>;

#[derive(Debug, Clone, Eq, PartialEq, Default, Serialize)]
pub struct Geometry {
    maxrow: Size,
    maxcol: Size,
    rowcol: RowCol
}
impl Geometry {
    pub fn add(&mut self, cell_id: CellID, rowcol: Location) {
        self.maxrow = max(self.maxrow, rowcol.row);
        self.maxcol = max(self.maxrow, rowcol.col);
        self.rowcol.insert(cell_id.name, rowcol);
    }
}
