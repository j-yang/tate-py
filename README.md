# tate-py

Python bindings for [tate](https://github.com/j-yang/tate) — structured data
version control with identity-aware merge.

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

# Diff: what changed?
edits = repo.diff(v0, v1)
for e in edits:
    print(f"  {e.location}: {e.old} -> {e.new}")

# History.
for h, msg in repo.log():
    print(f"  {h}: {msg}")
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
