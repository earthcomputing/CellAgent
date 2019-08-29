use std::{cmp::max,
          collections::HashMap,
          fmt, fmt::Write,
          sync::{Arc, Mutex}
};

use actix_web::{web, Error, HttpRequest, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use serde_json::Value;

type Size = usize;

pub fn cell_geometry(path: &str, state: web::Data<AppGeometry>, record: web::Json<Value>)
                 -> Result<impl Responder, Error> {
    let app_geometry = state.get_ref();
    if let Some(body) = record.get("body") {
        let geo = serde_json::from_value::<GeoStruct>(body.clone())?;
        app_geometry.geometry.lock().unwrap().add(Id::new(geo.cell_number),
                                                  Location::new(geo.location));
    }
    Ok(HttpResponse::Ok().body(format!("{} adding {}\n{:?}\n", path, record, app_geometry)))
}
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
struct GeoStruct {
    cell_number: usize,
    location: [usize; 2]
}
#[derive(Debug, Default, Serialize)]
pub struct AppGeometry {
    geometry: Mutex<Geometry>
}

#[derive(Debug, Copy, Clone, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Id(Size);
impl Id {
    pub fn new(id: Size) -> Id { Id(id) }
}
impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Id {}", self.0)
    }
}
#[derive(Debug, Copy, Clone, Default, Eq, PartialEq, Serialize)]
pub struct Location { row: Size, col: Size }
impl Location {
    pub fn new(rowcol: [Size; 2]) -> Location { Location { row: rowcol[0], col: rowcol[1] } }
}
impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Row {} Col {}", self.row, self.col)
    }
}

type RowCol = HashMap<Size, Location>;

#[derive(Debug, Clone, Eq, PartialEq, Default, Serialize)]
pub struct Geometry {
    maxrow: Size,
    maxcol: Size,
    rowcol: RowCol
}
impl Geometry {
    pub fn add(&mut self, id: Id, rowcol: Location) { 
        self.maxrow = max(self.maxrow, rowcol.row);
        self.maxcol = max(self.maxrow, rowcol.col);
        self.rowcol.insert(id.0, rowcol); 
    }
}
impl fmt::Display for Geometry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = format!("\nCell layout");
        if self.rowcol.is_empty() {
            write!(f, "{}", "No cells")
        } else {
            write!(s, "\n\n    Cell  Row  Col")?;
            for (id, rowcol) in &self.rowcol {
                write!(s, "\n    {:4} {:4} {:4}", id, rowcol.row, rowcol.col)?;
            }
            write!(f, "{}\n", s)
        }
     }
}
