//! Python bindings for tate — structured data version control.
//!
//! Provides:
//! - `Repo`: in-app version control for structured data (commit, diff, merge)
//! - `diff_trees`: structural diff of two trees
//! - `merge_trees`: 3-way merge of trees
//!
//! Data is passed as Python dicts, converted to/from tate's TreeNode internally.

use pyo3::prelude::*;
use pyo3::exceptions::PyValueError;
use tate::repo::{Repo, Hash};
use tate::tree::{TreeNode, tree_diff, tree_merge};

// ─── dict ↔ TreeNode conversion ──────────────────────────────

fn dict_to_tree(value: &PyAny) -> PyResult<TreeNode> {
    if let Ok(d) = value.extract::<&pyo3::types::PyDict>() {
        let mut node = TreeNode::new(
            d.get_item("kind")?.and_then(|v| v.extract().ok()).unwrap_or("root")
        );
        if let Some(id) = d.get_item("identity")? {
            if !id.is_none() {
                node = node.with_identity(id.extract()?);
            }
        }
        if let Some(label) = d.get_item("label")? {
            if !label.is_none() {
                node = node.with_label(label.extract()?);
            }
        }
        if let Some(text) = d.get_item("text")? {
            if !text.is_none() {
                node = node.with_text(text.extract()?);
            }
        }
        if let Some(attrs) = d.get_item("attributes")? {
            if let Ok(attr_dict) = attrs.extract::<&pyo3::types::PyDict>() {
                for (k, v) in attr_dict.iter() {
                    node = node.with_attr(k.extract()?, v.extract()?);
                }
            }
        }
        if let Some(children) = d.get_item("children")? {
            if let Ok(children_list) = children.extract::<&pyo3::types::PyList>() {
                for child in children_list.iter() {
                    node = node.with_child(dict_to_tree(child)?);
                }
            }
        }
        Ok(node)
    } else {
        Err(PyValueError::new_err("expected a dict"))
    }
}

fn tree_to_dict(py: Python, node: &TreeNode) -> PyResult<PyObject> {
    let dict = pyo3::types::PyDict::new(py);

    dict.set_item("kind", &node.kind)?;

    if let Some(ref id) = node.identity {
        dict.set_item("identity", id)?;
    }

    if !node.label.is_empty() {
        dict.set_item("label", &node.label)?;
    }

    if !node.text.is_empty() {
        dict.set_item("text", &node.text)?;
    }

    if !node.attributes.is_empty() {
        let attr_dict = pyo3::types::PyDict::new(py);
        for (k, v) in &node.attributes {
            attr_dict.set_item(k, v)?;
        }
        dict.set_item("attributes", attr_dict)?;
    }

    if !node.children.is_empty() {
        let children: Vec<PyObject> = node.children.iter()
            .map(|c| tree_to_dict(py, c))
            .collect::<PyResult<_>>()?;
        dict.set_item("children", children)?;
    }

    Ok(dict.into())
}

// ─── Repo class ───────────────────────────────────────────────

/// In-app version control for structured data.
///
/// Example::
///
///     from tate_py import Repo
///
///     repo = Repo()
///     v0 = repo.commit("initial", [], {"kind": "root", "children": [
///         {"kind": "server", "identity": "s1", "attributes": {"port": "8080"}},
///     ]})
///     v1 = repo.commit("port change", [v0], {"kind": "root", "children": [
///         {"kind": "server", "identity": "s1", "attributes": {"port": "9090"}},
///     ]})
///
///     patch = repo.diff(v0, v1)
///     print(f"{len(patch['edits'])} edits")
///
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

    /// Commit a version. Returns the commit hash (int).
    fn commit(
        &mut self,
        message: &str,
        parents: Vec<u64>,
        tree: &PyAny,
    ) -> PyResult<u64> {
        let node = dict_to_tree(tree)?;
        Ok(self.inner.commit(message, &parents, &node))
    }

    /// Diff two commits. Returns a list of edits.
    fn diff(&self, a: u64, b: u64) -> Vec<PyEdit> {
        let patch = self.inner.diff(a, b);
        patch.edits.iter().map(|(loc, edit)| PyEdit {
            location: loc.clone(),
            old: edit.old.as_ref().map(|v| format!("{:?}", v)),
            new: edit.new.as_ref().map(|v| format!("{:?}", v)),
        }).collect()
    }

    /// Three-way merge of two commits.
    /// Returns (merged_tree, conflicts) where conflicts is a list of conflict dicts.
    fn merge(&mut self, ours: u64, theirs: u64, py: Python) -> PyResult<(PyObject, Vec<PyObject>)> {
        let result = self.inner.merge(ours, theirs);
        let merged_tree = self.inner.tree(result.merged_section);
        let tree_dict = tree_to_dict(py, &merged_tree)?;

        let conflicts: Vec<PyObject> = result.conflicts.iter().map(|c| {
            let d = pyo3::types::PyDict::new(py);
            d.set_item("location", c.location.join("/")).unwrap_or(());
            d.into()
        }).collect();

        Ok((tree_dict, conflicts))
    }

    /// Cherry-pick a change onto a target commit. Returns merged tree.
    fn cherry_pick(&mut self, src: u64, dst: u64, py: Python) -> PyResult<PyObject> {
        let hash = self.inner.cherry_pick(src, dst)
            .map_err(|e| PyValueError::new_err(format!("{:?}", e)))?;
        let tree = self.inner.tree(hash);
        tree_to_dict(py, &tree)
    }

    /// Revert a change on a target commit. Returns reverted tree.
    fn revert(&mut self, target: u64, dst: u64, py: Python) -> PyResult<PyObject> {
        let hash = self.inner.revert(target, dst)
            .map_err(|e| PyValueError::new_err(format!("{:?}", e)))?;
        let tree = self.inner.tree(hash);
        tree_to_dict(py, &tree)
    }

    /// Commit history (list of (hash, message) tuples).
    fn log(&self) -> Vec<(u64, String)> {
        self.inner.log(None).into_iter()
            .map(|(h, c)| (h, c.message.clone()))
            .collect()
    }

    /// Get the tree for a commit as a dict.
    fn tree_at(&self, commit: u64, py: Python) -> PyResult<PyObject> {
        let tree = self.inner.tree(commit);
        tree_to_dict(py, &tree)
    }

    /// Number of commits.
    #[getter]
    fn len(&self) -> usize {
        self.inner.len()
    }

    fn __repr__(&self) -> String {
        format!("Repo(commits={})", self.inner.len())
    }
}

/// One edit in a diff.
#[pyclass(name = "Edit")]
#[derive(Clone)]
struct PyEdit {
    location: Vec<String>,
    old: Option<String>,
    new: Option<String>,
}

#[pymethods]
impl PyEdit {
    #[getter]
    fn location(&self) -> Vec<String> {
        self.location.clone()
    }

    #[getter]
    fn old(&self) -> Option<String> {
        self.old.clone()
    }

    #[getter]
    fn new(&self) -> Option<String> {
        self.new.clone()
    }

    fn __repr__(&self) -> String {
        format!("Edit(location={:?}, old={:?}, new={:?})", self.location, self.old, self.new)
    }
}

// ─── Module-level functions ───────────────────────────────────

/// Structural diff of two trees (passed as dicts).
#[pyfunction]
fn diff_trees(a: &PyAny, b: &PyAny, py: Python) -> PyResult<Vec<PyObject>> {
    let tree_a = dict_to_tree(a)?;
    let tree_b = dict_to_tree(b)?;
    let diff = tree_diff(&tree_a, &tree_b);

    diff.changes.iter().map(|c| {
        let d = pyo3::types::PyDict::new(py);
        d.set_item("kind", format!("{:?}", c.kind))?;
        d.set_item("elem_type", &c.elem_type)?;
        d.set_item("id", &c.id)?;
        Ok::<_, pyo3::PyErr>(d.into())
    }).collect()
}

/// 3-way merge of three trees (passed as dicts).
#[pyfunction]
fn merge_trees(base: &PyAny, ours: &PyAny, theirs: &PyAny, py: Python) -> PyResult<(PyObject, usize)> {
    let base_tree = dict_to_tree(base)?;
    let ours_tree = dict_to_tree(ours)?;
    let theirs_tree = dict_to_tree(theirs)?;

    let result = tree_merge(&base_tree, &ours_tree, &theirs_tree);
    let merged = tree_to_dict(py, &result.tree)?;
    Ok((merged, result.conflicts.len()))
}

// ─── Module init ──────────────────────────────────────────────

#[pymodule]
fn tate_py(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyRepo>()?;
    m.add_class::<PyEdit>()?;

    m.add_function(wrap_pyfunction!(diff_trees, m)?)?;
    m.add_function(wrap_pyfunction!(merge_trees, m)?)?;

    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    Ok(())
}
