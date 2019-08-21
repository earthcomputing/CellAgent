use std::{cmp::max,
          collections::HashMap,
          fmt, fmt::Write
};

use actix_web::{Error, HttpRequest, HttpResponse, Responder, FromRequest};
use serde::Serialize;

type Size = usize;
#[derive(Debug, Copy, Clone, Default, Eq, PartialEq, Serialize)]
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
    pub fn new(row: Size, col: Size) -> Location { Location { row, col } }
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
    pub fn limits(&self) -> Location { Location::new(self.maxrow, self.maxcol) }
    pub fn location(&self, id: Size) -> Option<&Location> { self.rowcol.get(&id) }
}

impl Responder for Geometry {
    type Error = Error;
    type Future =Result<HttpResponse, Error>;
    
    fn respond_to(self, _req: &HttpRequest) -> Self::Future {
        let body = serde_json::to_string(&self)?;
        Ok(HttpResponse::Ok()
            .content_type("application/json")
            .body(body))
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