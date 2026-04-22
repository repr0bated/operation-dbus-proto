/*
 * Copyright 2022, The Cozo Project Authors. Licensed under MIT/Apache-2.0/BSD-3-Clause.
 */

use cozo::Db;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;

#[pyclass]
struct CozoDbPy {
    db: Option<Db>,
}

#[pymethods]
impl CozoDbPy {
    #[new]
    fn new(path: &str) -> PyResult<Self> {
        match Db::new(path) {
            Ok(db) => Ok(Self { db: Some(db) }),
            Err(err) => Err(PyException::new_err(format!("{:?}", err)))
        }
    }
    pub fn run_query(&self, py: Python<'_>, query: &str, params: &str) -> String {
        if let Some(db) = &self.db {
            py.allow_threads(|| db.run_script_str(query, params))
        } else {
            r##"{"ok":false,"message":"database closed"}"##.to_string()
        }
    }
    pub fn close(&mut self) -> bool {
        self.db.take().is_some()
    }
}

#[pymodule]
fn cozo_embedded(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add_class::<CozoDbPy>()?;
    Ok(())
}
