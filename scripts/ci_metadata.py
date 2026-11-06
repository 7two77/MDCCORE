"""Shared complete CI configuration identity, separate from implementation bytes."""
import hashlib
from pathlib import Path


def ci_fingerprints(root):
    root = Path(root)
    files = [p for p in (root / ".github/workflows").glob("*") if p.suffix in (".yml", ".yaml")]
    files += list((root / ".github/ci").glob("*"))
    return {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(files) if p.is_file()}
