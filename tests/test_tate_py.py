"""Tests for tate-py bindings."""
from tate_py import Repo, diff_trees, merge_trees


def sample_tree():
    return {
        "kind": "root",
        "children": [
            {"kind": "server", "identity": "s1", "attributes": {"port": "8080"}},
            {"kind": "db", "identity": "d1", "attributes": {"url": "localhost"}},
        ],
    }


def modify_port(tree, port):
    import copy
    t = copy.deepcopy(tree)
    t["children"][0]["attributes"]["port"] = port
    return t


def modify_url(tree, url):
    import copy
    t = copy.deepcopy(tree)
    t["children"][1]["attributes"]["url"] = url
    return t


def test_commit_and_len():
    repo = Repo()
    assert len(repo) == 0
    v0 = repo.commit("initial", [], sample_tree())
    assert len(repo) == 1


def test_diff():
    repo = Repo()
    v0 = repo.commit("v0", [], sample_tree())
    v1 = repo.commit("v1", [v0], modify_port(sample_tree(), "9090"))
    edits = repo.diff(v0, v1)
    assert len(edits) > 0


def test_clean_merge():
    repo = Repo()
    v0 = repo.commit("base", [], sample_tree())
    v1 = repo.commit("port", [v0], modify_port(sample_tree(), "9090"))
    v2 = repo.commit("url", [v0], modify_url(sample_tree(), "prod"))
    merged, conflicts = repo.merge(v1, v2)
    assert conflicts == []


def test_conflict_merge():
    repo = Repo()
    v0 = repo.commit("base", [], sample_tree())
    v1 = repo.commit("9090", [v0], modify_port(sample_tree(), "9090"))
    v2 = repo.commit("3000", [v0], modify_port(sample_tree(), "3000"))
    merged, conflicts = repo.merge(v1, v2)
    assert len(conflicts) > 0


def test_log():
    repo = Repo()
    v0 = repo.commit("first", [], sample_tree())
    v1 = repo.commit("second", [v0], sample_tree())
    entries = repo.log()
    assert len(entries) == 2


def test_tree_at():
    repo = Repo()
    v0 = repo.commit("initial", [], sample_tree())
    tree = repo.tree_at(v0)
    assert tree["kind"] == "root"
    assert len(tree["children"]) == 2


def test_cherry_pick():
    repo = Repo()
    v0 = repo.commit("base", [], sample_tree())
    v1 = repo.commit("port", [v0], modify_port(sample_tree(), "9090"))
    v2 = repo.commit("url", [v0], modify_url(sample_tree(), "staging"))
    picked = repo.cherry_pick(v1, v2)
    assert picked is not None


def test_diff_trees():
    a = sample_tree()
    b = modify_port(sample_tree(), "9090")
    changes = diff_trees(a, b)
    assert len(changes) > 0


def test_diff_trees_no_change():
    a = sample_tree()
    changes = diff_trees(a, a)
    assert len(changes) == 0


def test_merge_trees_clean():
    base = sample_tree()
    ours = modify_port(sample_tree(), "9090")
    theirs = modify_url(sample_tree(), "prod")
    merged, n = merge_trees(base, ours, theirs)
    assert n == 0
    assert merged is not None


def test_structural_dangling_conflict():
    # Sheaf merge's defining capability: ours deletes Q while theirs moves C
    # under Q. C ends up present with parent Q absent -> a Dangling conflict,
    # invisible to the discrete per-field model.
    base = {"kind": "root", "children": [
        {"kind": "p", "identity": "P", "children": [
            {"kind": "c", "identity": "C", "attributes": {"value": "1"}}]},
        {"kind": "q", "identity": "Q"},
    ]}
    ours = {"kind": "root", "children": [
        {"kind": "p", "identity": "P", "children": [
            {"kind": "c", "identity": "C", "attributes": {"value": "1"}}]},
    ]}
    theirs = {"kind": "root", "children": [
        {"kind": "p", "identity": "P"},
        {"kind": "q", "identity": "Q", "children": [
            {"kind": "c", "identity": "C", "attributes": {"value": "1"}}]},
    ]}

    repo = Repo()
    b = repo.commit("base", [], base)
    o = repo.commit("ours", [b], ours)
    t = repo.commit("theirs", [b], theirs)
    merged, conflicts = repo.merge(o, t)

    dangling = [c for c in conflicts if c["kind"] == "Dangling"]
    assert len(dangling) == 1
    assert dangling[0]["identity"] == "C"
    assert dangling[0]["missing_parent"] == "Q"
