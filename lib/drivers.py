import bpy  # pylint: disable=import-error

VARS_DRIVER = ("expression", "type", "use_self")
VARS_TARGET = ("context_property", "bone_target", "data_path", "rotation_mode", "transform_space", "transform_type")

cm_map = {}


class DriverException(Exception):
    pass


def id_to_cm(t):
    if t is None:
        return "none"
    try:
        return cm_map[t.name]
    except KeyError as e:
        raise DriverException(e.args[0])


def target_data(t):
    if t.id_type != "OBJECT":
        raise DriverException("Invalid target id_type " + t.id_type)
    result = {k: getattr(t, k) for k in VARS_TARGET}
    result["cm_id"] = id_to_cm(t.id)
    return result


def variables_data(item):
    return {v.name: {
            "type": v.type,
            "targets": [target_data(t) for t in v.targets],
        } for v in item}


def driver_data(d):
    result = {k: getattr(d, k) for k in VARS_DRIVER}
    result["variables"] = variables_data(d.variables)
    return result


def get_drivers(item):
    ad = item.animation_data
    if not ad:
        return {}
    return [{
            "data_path": d.data_path,
            "array_index": d.array_index,
            "driver": driver_data(d.driver),
        } for d in item.animation_data.drivers]


def driver_items(name, obj):
    d = [m for m in (
            (name, get_drivers(obj)),
            (name+".data", get_drivers(obj.data)),
        ) if m[1]]
    if obj.type == "MESH":
        item = get_drivers(obj.data.shape_keys)
        if item:
            d.append((name+".data.shape_keys", item))
    return d


def export(**args):
    try:
        for k, v in args.items():
            cm_map[v.name] = k
        return dict(it for m in (driver_items(k, v) for k, v in args.items()) for it in m)
    finally:
        cm_map.clear()


def clear_item(item):
    ad = item.animation_data
    if not ad:
        return
    for d in ad.drivers:
        item.driver_remove(d.data_path, d.array_index)


def clear_obj(obj):
    clear_item(obj)
    clear_item(obj.data)
    if obj.type == "MESH":
        clear_item(obj.data.shape_keys)


def cm_to_id(t: str):
    if t.lower() == "none":
        return None
    try:
        return cm_map[t]
    except KeyError as e:
        raise DriverException(e.args[0])


def fill_target(t, d):
    try:
        t.id_type = "OBJECT"
    except AttributeError:
        pass
    t.id = cm_to_id(d["cm_id"])
    for k in VARS_TARGET:
        v = d.get(k)
        if v is not None:
            setattr(t, k, v)


def fill_variable(t, d):
    t.type = d["type"]
    dt = d["targets"]
    if len(t.targets) != len(dt):
        raise DriverException("Target count mismatch")
    for dst, src in zip(t.targets, dt):
        fill_target(dst, src)


def fill_driver(t, d):
    for k in VARS_DRIVER:
        v = d.get(k)
        if v is not None:
            setattr(t, k, v)
    for k, v in d["variables"].items():
        var = t.variables.new()
        var.name = k
        fill_variable(var, v)


def name_to_obj(name: str):
    parts = name.split(".")
    result = cm_to_id(parts[0])
    for k in parts[1:]:
        result = getattr(result, k)
    return result


def dimport(d: dict, overwrite: bool, **args):
    try:
        for k, v in args.items():
            cm_map[k] = v
        for k, v in d.items():
            t = name_to_obj(k)
            if not t:
                raise DriverException("Invalid object " + k)
            for drv in v:
                try:
                    if overwrite:
                        fc = t.driver_remove(drv["data_path"], drv["array_index"])
                    fc = t.driver_add(drv["data_path"], drv["array_index"])
                except TypeError:
                    if overwrite:
                        fc = t.driver_remove(drv["data_path"])
                    fc = t.driver_add(drv["data_path"])
                fill_driver(fc.driver, drv["driver"])
    finally:
        cm_map.clear()


def setup_shapekey_drivers(obj):
    """Establish live driver links between CharMorph UI properties and shape key weights."""
    if obj is None or getattr(obj, "data", None) is None:
        return
    keys = getattr(obj.data, "shape_keys", None)
    if keys is None or getattr(keys, "key_blocks", None) is None:
        return

    for sk in keys.key_blocks:
        if sk.name.startswith(("L1_", "L2_", "L4_")):
            parts = sk.name.split("_")
            prop_name = parts[-1] if len(parts) >= 2 else sk.name
            pname = f"cmorph_L2_{prop_name}"
            data_dict = obj.data if isinstance(obj.data, dict) or hasattr(obj.data, "__setitem__") else obj
            try:
                if pname not in data_dict:
                    data_dict[pname] = float(getattr(sk, "value", 0.0))
            except Exception:
                pass

            try:
                if hasattr(sk, "driver_add"):
                    fcurve = sk.driver_add("value")
                    drv = getattr(fcurve, "driver", None)
                    if drv is not None:
                        drv.type = 'AVERAGE'
                        if not getattr(drv, "variables", None):
                            var = drv.variables.new()
                        else:
                            var = drv.variables[0]
                        var.name = "val"
                        if hasattr(var, "targets") and var.targets:
                            var.targets[0].id_type = 'OBJECT'
                            var.targets[0].id = obj
                            var.targets[0].data_path = f'data["{pname}"]'
            except Exception:
                pass


def mute_inactive_shape_keys(obj):
    """Mute inactive shape key evaluation groups to optimize viewport playback performance."""
    if obj is None or getattr(obj, "data", None) is None:
        return
    keys = getattr(obj.data, "shape_keys", None)
    if keys is None or getattr(keys, "key_blocks", None) is None:
        return

    ref_key = getattr(keys, "reference_key", None)
    for sk in keys.key_blocks:
        if sk is ref_key or getattr(sk, "name", "") == "Basis":
            sk.mute = False
            continue
        val = float(getattr(sk, "value", 0.0))
        if val == 0.0 or abs(val) < 1e-5:
            sk.mute = True
        else:
            sk.mute = False

