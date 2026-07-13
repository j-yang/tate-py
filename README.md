# tate-py

Python bindings for [tate](https://github.com/j-yang/tate) — structured data
version control with identity-aware, sheaf-pushout merge.

## Install

```bash
pip install tate-py
```

## Usage

### In-app version control

```python
from tate_py import Repo

repo = Repo()

# Commit versions of your structured data.
v0 = repo.commit("initial", [], {
    "kind": "root",
    "children": [
        {"kind": "row", "identity": "1", "attributes": {"name": "Alice", "age": "30"}},
        {"kind": "row", "identity": "2", "attributes": {"name": "Bob", "age": "25"}},
    ]
})

# User A changes Alice's age.
v1 = repo.commit("Alice age -> 31", [v0], {
    "kind": "root",
    "children": [
        {"kind": "row", "identity": "1", "attributes": {"name": "Alice", "age": "31"}},
        {"kind": "row", "identity": "2", "attributes": {"name": "Bob", "age": "25"}},
    ]
})

# User B changes Bob's name (concurrent).
v2 = repo.commit("Bob -> Robert", [v0], {
    "kind": "root",
    "children": [
        {"kind": "row", "identity": "1", "attributes": {"name": "Alice", "age": "30"}},
        {"kind": "row", "identity": "2", "attributes": {"name": "Robert", "age": "25"}},
    ]
})

# Merge: disjoint rows -> clean (no false conflict).
merged, conflicts = repo.merge(v1, v2)
print(f"Conflicts: {conflicts}")  # []

# Diff: what changed? Each edit is a (identity, old, new) tuple.
for identity, old, new in repo.diff(v0, v1):
    print(f"  {identity}: {old} -> {new}")

# History.
for h, msg in repo.log():
    print(f"  {h}: {msg}")
```

### Conflicts

`repo.merge` runs the two-stage sheaf merge and returns the merged tree
plus a list of conflict dicts, each `{"kind", "identity", "missing_parent"}`:

- `Field` — both branches changed the same field (the only kind a discrete
  per-field model can see).
- `Dangling` — a present node ended up referencing a parent that was
  deleted on the other branch. This *structural* conflict is invisible to
  a discrete merge; `missing_parent` names the absent reference and the
  node is dropped from the merged tree.

```python
merged, conflicts = repo.merge(ours, theirs)
for c in conflicts:
    if c["kind"] == "Dangling":
        print(f"structural conflict at {c['identity']}: parent {c['missing_parent']} gone")
```

### Structural diff (no repo)

```python
from tate_py import diff_trees

changes = diff_trees(
    {"kind": "root", "children": [
        {"kind": "server", "identity": "s1", "attributes": {"port": "8080"}}]},
    {"kind": "root", "children": [
        {"kind": "server", "identity": "s1", "attributes": {"port": "9090"}}]},
)
```

### Merge (no repo)

```python
from tate_py import merge_trees

merged, num_conflicts = merge_trees(base_dict, ours_dict, theirs_dict)
```

## Data format

Trees are plain Python dicts:

```python
{
    "kind": "node type name",
    "identity": "unique id (optional, enables identity-aware merge)",
    "label": "human-readable label (optional)",
    "text": "text content (optional)",
    "attributes": {"key": "value", ...},
    "children": [...]
}
```

The `identity` field is the key to identity-aware merge: two versions that
change different rows (different identities) merge cleanly.

## License

MIT
