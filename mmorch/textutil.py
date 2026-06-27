"""textutil — shared text helpers. Dedups the code-fence extractor that was copy-pasted
(identical shape, only the language tag differed) across code_loop/speedup/autoresearch/
project_loop/enrich/rubric_loop. One place to change = the DRY fix."""
from __future__ import annotations

import re

# any language tag (python/json/...) or none, then the fenced body up to the next ```
_FENCE = re.compile(r"```(?:[a-zA-Z0-9_+.-]*)?\s*(.*?)```", re.DOTALL)


def extract_fence(text: str) -> str:
    """First fenced ```...``` block (any language tag), stripped. Whole text if no fence."""
    t = text or ""
    m = _FENCE.search(t)
    return (m.group(1) if m else t).strip()


if __name__ == "__main__":
    assert extract_fence("x ```python\nprint(1)\n``` y") == "print(1)"
    assert extract_fence("```json\n{\"a\":1}\n```") == '{"a":1}'
    assert extract_fence("```\nraw\n```") == "raw"
    assert extract_fence("no fence here") == "no fence here"
    assert extract_fence("") == ""
    assert extract_fence(None) == ""  # type: ignore[arg-type]
    print("textutil OK — fence extraction (python/json/none/raw/empty)")
