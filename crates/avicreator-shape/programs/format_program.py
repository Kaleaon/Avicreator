"""Compact JSON formatting for authored shape programs."""

import json


def dump(v, ind=""):
    """JSON with short arrays and small objects on one line (matches the Studio formatter)."""

    def flat(x):
        return not isinstance(x, (dict, list))

    if isinstance(v, list):
        if all(flat(x) for x in v):
            return "[" + ", ".join(json.dumps(x) for x in v) + "]"
        return "[\n" + ",\n".join(ind + "  " + dump(x, ind + "  ") for x in v) + "\n" + ind + "]"
    if isinstance(v, dict):
        inner = [json.dumps(k) + ": " + dump(x, ind + "  ") for k, x in v.items()]
        one = "{" + ", ".join(inner) + "}"
        simple = all(
            flat(x)
            or (isinstance(x, list) and all(flat(y) or (isinstance(y, list) and all(flat(z) for z in y)) for y in x))
            for x in v.values()
        )
        if simple and len(one) + len(ind) <= 150:
            return one
        return "{\n" + ",\n".join(ind + "  " + s for s in inner) + "\n" + ind + "}"
    return json.dumps(v)
