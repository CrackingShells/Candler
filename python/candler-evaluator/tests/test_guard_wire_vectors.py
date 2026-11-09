from pydantic import ValidationError

from candler_evaluator._models.evaluator_message import Message


def accepted(path):
    """Whether the generated envelope model takes the file's one message, as the loop does."""
    try:
        Message.model_validate_json(path.read_text(), strict=True)
    except ValidationError:
        return False
    return True


def test_models_decide_the_wire_vectors(vectors):
    """Guards: R10 — the Python wire models accept every valid shared vector and refuse every invalid one, so Rust and Python agree on the envelope. Oracle: the hand-written WIRE1 vector files under crates/candler-schemas/tests/fixtures/rpc, read from their one shared copy."""
    valid = sorted((vectors / "valid").glob("*.json"))
    invalid = sorted((vectors / "invalid").glob("*.json"))
    # Both sets must exist, or a missing directory would pass this test for nothing.
    assert len(valid) >= 9 and len(invalid) >= 6
    assert [p.name for p in valid if not accepted(p)] == []
    assert [p.name for p in invalid if accepted(p)] == []
