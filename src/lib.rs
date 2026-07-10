//! Python bindings for tate — structured data version control.

use pyo3::prelude::*;
use pyo3::exceptions::PyValueError;
use pyo3::types::{PyDict, PyList};
use tate::repo::Repo;
use tate::tree::{TreeNode, tree_diff, tree_merge};

fn py_to_tree(obj: &Bound<'_, PyAny>) -> PyResult<TreeNode> {
    let dict = obj.downcast::<PyDict>()?;

    let kind: String = match dict.get_item("kind")? {
        Some(v) => v.extract().unwrap_or_else(|_| "root".to_string()),
        None => "root".to_string(),
    };
    let mut node = TreeNode::new(&kind);

    if let Some(id) = dict.get_item("identity")? {
        if !id.is_none() {
            node = node.with_identity(id.extract::<String>()?);
        }
    }
    if let Some(label) = dict.get_item("label")? {
        if !label.is_none() {
            node = node.with_label(label.extract::<String>()?);
        }
    }
    if let Some(text) = dict.get_item("text")? {
        if !text.is_none() {
            node = node.with_text(text.extract::<String>()?);
        }
    }
    if let Some(attrs) = dict.get_item("attributes")? {
        if let Ok(ad) = attrs.downcast::<PyDict>() {
            for (k, v) in ad.iter() {
                let key: String = k.extract()?;
                let val: String = v.extract()?;
                node = node.with_attr(key, val);
            }
        }
    }
    if let Some(children) = dict.get_item("children")? {
        if let Ok(cl) = children.downcast::<PyList>() {
            for child in cl.iter() {
                node = node.with_child(py_to_tree(&child)?);
            }
        }
    }
    Ok(node)
}

fn tree_to_py(py: Python<'_>, node: &TreeNode) -> PyResult<PyObject> {
    let d = PyDict::new_bound(py);

    d.set_item("kind", &node.kind)?;

    if let Some(ref id) = node.identity {
        d.set_item("identity", id)?;
    }
    if !node.label.is_empty() {
        d.set_item("label", &node.label)?;
    }
    if !node.text.is_empty() {
        d.set_item("text", &node.text)?;
    }

    if !node.attributes.is_empty() {
        let ad = PyDict::new_bound(py);
        for (k, v) in &node.attributes {
            ad.set_item(k, v)?;
        }
        d.set_item("attributes", ad)?;
    }

    if !node.children.is_empty() {
        let children: Vec<PyObject> = node.children.iter()
            .map(|c| tree_to_py(py, c))
            .collect::<PyResult<_>>()?;
        d.set_item("children", children)?;
    }

    Ok(d.into_any().into())
}

#[pyclass(name = "Repo")]
struct PyRepo {
    inner: Repo,
}

#[pymethods]
impl PyRepo {
    #[new]
    fn new() -> Self {
        PyRepo { inner: Repo::new() }
    }

    fn commit(&mut self, message: &str, parents: Vec<u64>, tree: &Bound<'_, PyAny>) -> PyResult<u64> {
        let node = py_to_tree(tree)?;
        Ok(self.inner.commit(message, &parents, &node))
    }

    fn diff(&self, a: u64, b: u64) -> Vec<(Vec<String>, Option<String>, Option<String>)> {
        let patch = self.inner.diff(a, b);
        patch.edits.iter().map(|(loc, edit)| {
            (loc.clone(), edit.old.as_ref().map(|v| format!("{:?}", v)),
             edit.new.as_ref().map(|v| format!("{:?}", v)))
        }).collect()
    }

    fn merge(&mut self, ours: u64, theirs: u64, py: Python<'_>) -> PyResult<(PyObject, Vec<String>)> {
        let result = self.inner.merge(ours, theirs);
        // merged_section is a section hash, not a commit hash.
        // Convert that section to a tree directly.
        let merged = self.inner.section_as_tree(result.merged_section);
        let tree_dict = tree_to_py(py, &merged)?;
        let conflicts: Vec<String> = result.conflicts.iter()
            .map(|c| c.location.join("/")).collect();
        Ok((tree_dict, conflicts))
    }

    fn cherry_pick(&mut self, src: u64, dst: u64, py: Python<'_>) -> PyResult<PyObject> {
        let hash = self.inner.cherry_pick(src, dst)
            .map_err(|e| PyValueError::new_err(format!("{:?}", e)))?;
        tree_to_py(py, &self.inner.section_as_tree(hash))
    }

    fn revert(&mut self, target: u64, dst: u64, py: Python<'_>) -> PyResult<PyObject> {
        let hash = self.inner.revert(target, dst)
            .map_err(|e| PyValueError::new_err(format!("{:?}", e)))?;
        tree_to_py(py, &self.inner.section_as_tree(hash))
    }

    fn log(&self) -> Vec<(u64, String)> {
        self.inner.log(None).into_iter()
            .map(|(h, c)| (h, c.message.clone()))
            .collect()
    }

    fn tree_at(&self, commit: u64, py: Python<'_>) -> PyResult<PyObject> {
        tree_to_py(py, &self.inner.tree(commit))
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn __repr__(&self) -> String {
        format!("Repo(commits={})", self.inner.len())
    }
}

#[pyfunction]
fn diff_trees(a: &Bound<'_, PyAny>, b: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Vec<PyObject>> {
    let tree_a = py_to_tree(a)?;
    let tree_b = py_to_tree(b)?;
    let diff = tree_diff(&tree_a, &tree_b);

    diff.changes.iter().map(|c| {
        let d = PyDict::new_bound(py);
        d.set_item("kind", format!("{:?}", c.kind))?;
        d.set_item("elem_type", &c.elem_type)?;
        d.set_item("id", &c.id)?;
        Ok::<_, pyo3::PyErr>(d.into_any().into())
    }).collect()
}

#[pyfunction]
fn merge_trees(
    base: &Bound<'_, PyAny>,
    ours: &Bound<'_, PyAny>,
    theirs: &Bound<'_, PyAny>,
    py: Python<'_>,
) -> PyResult<(PyObject, usize)> {
    let base_tree = py_to_tree(base)?;
    let ours_tree = py_to_tree(ours)?;
    let theirs_tree = py_to_tree(theirs)?;
    let result = tree_merge(&base_tree, &ours_tree, &theirs_tree);
    let merged = tree_to_py(py, &result.tree)?;
    Ok((merged, result.conflicts.len()))
}

#[pymodule]
fn tate_py(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyRepo>()?;
    m.add_function(wrap_pyfunction!(diff_trees, m)?)?;
    m.add_function(wrap_pyfunction!(merge_trees, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
