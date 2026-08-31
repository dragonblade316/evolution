#!/usr/bin/env python3
"""Stream CopperList JSON exports into Rerun."""

from __future__ import annotations

import argparse
import json
import math
import re
import sys
import tomllib
from collections.abc import Iterator
from dataclasses import dataclass
from pathlib import Path
from typing import Any, TextIO

import rerun as rr
import rerun.blueprint as rrb


@dataclass(frozen=True)
class OutputMapping:
    name: str
    kind: str = "generic"
    dimensions: tuple[float, float, float] = (0.5, 0.5, 0.2)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Visualize JSON emitted by Copper's extract-copperlists command"
    )
    parser.add_argument("json", type=Path, help="CopperList JSON stream, or '-' for stdin")
    parser.add_argument("--mapping", type=Path, help="Optional output mapping TOML")
    output = parser.add_mutually_exclusive_group()
    output.add_argument("--save", type=Path, metavar="OUTPUT.RRD")
    output.add_argument("--spawn", action="store_true", help="Spawn the viewer (default)")
    parser.add_argument("--limit", type=int, help="Only process the first N CopperLists")
    return parser.parse_args()


def iter_json_values(stream: TextIO, chunk_size: int = 1024 * 1024) -> Iterator[Any]:
    """Decode whitespace-separated JSON values without loading the whole file."""
    decoder = json.JSONDecoder()
    buffer = ""
    eof = False
    started = False
    while True:
        if not eof and len(buffer) < chunk_size:
            chunk = stream.read(chunk_size)
            if chunk:
                buffer += chunk
            else:
                eof = True

        buffer = buffer.lstrip()
        if not started and buffer and not buffer.startswith("{"):
            # cu29-export currently writes an informational line to stdout before
            # the first JSON object. Ignore only that preamble; once JSON starts,
            # malformed content remains an error.
            object_start = buffer.find("{")
            if object_start >= 0:
                buffer = buffer[object_start:]
            elif eof:
                raise ValueError("CopperList JSON stream contains no JSON objects")
            else:
                buffer = ""
                continue
        if not buffer:
            if eof:
                return
            continue

        try:
            value, end = decoder.raw_decode(buffer)
        except json.JSONDecodeError:
            if eof:
                raise ValueError("truncated or invalid CopperList JSON stream") from None
            chunk = stream.read(chunk_size)
            if chunk:
                buffer += chunk
            else:
                eof = True
            continue

        yield value
        started = True
        buffer = buffer[end:]


def load_config(path: Path | None) -> tuple[dict[str, OutputMapping], list[str]]:
    if path is None:
        return {}, ["copper/outputs/slot_0/payload/**"]
    with path.open("rb") as stream:
        raw = tomllib.load(stream)
    mappings: dict[str, OutputMapping] = {}
    for key, value in raw.get("outputs", {}).items():
        dimensions = tuple(value.get("dimensions", [0.5, 0.5, 0.2]))
        if len(dimensions) != 3 or not all(float(v) > 0 for v in dimensions):
            raise ValueError(f"outputs.{key}.dimensions must contain three positive values")
        kind = value.get("kind", "generic")
        if kind not in {"generic", "pose", "point"}:
            raise ValueError(f"outputs.{key}.kind must be generic, pose, or point")
        mappings[str(key)] = OutputMapping(
            name=value.get("name", f"slot_{key.replace('.', '_')}"),
            kind=kind,
            dimensions=tuple(float(v) for v in dimensions),
        )
    time_series = raw.get("viewer", {}).get("time_series", [])
    if not isinstance(time_series, list) or not all(isinstance(v, str) for v in time_series):
        raise ValueError("viewer.time_series must be a list of entity path strings")
    return mappings, time_series


def send_blueprint(time_series: list[str]) -> None:
    rr.send_blueprint(
        rrb.Blueprint(
            rrb.Horizontal(
                rrb.Spatial3DView(
                    origin="copper/outputs",
                    contents=["copper/outputs/**"],
                    name="Copper spatial data",
                ),
                rrb.TimeSeriesView(
                    origin="copper/outputs",
                    contents=time_series,
                    name="Selected signals",
                ),
                column_shares=[1, 1],
            ),
            auto_views=False,
        )
    )


def safe_segment(value: str) -> str:
    result = re.sub(r"[^A-Za-z0-9_-]+", "_", value).strip("_")
    return result or "unnamed"


def flatten_messages(messages: list[Any]) -> Iterator[tuple[str, dict[str, Any]]]:
    for slot, value in enumerate(messages):
        if isinstance(value, list):
            for output, message in enumerate(value):
                if isinstance(message, dict):
                    yield f"{slot}.{output}", message
        elif isinstance(value, dict):
            yield str(slot), value


def tov_nanos(tov: Any) -> int | None:
    if isinstance(tov, dict):
        if isinstance(tov.get("Time"), int):
            return tov["Time"]
        value = tov.get("Range")
        if isinstance(value, dict) and isinstance(value.get("start"), int):
            return value["start"]
        if isinstance(value, list) and value and isinstance(value[0], int):
            return value[0]
    return None


def matrix4(value: Any) -> list[list[float]] | None:
    if not isinstance(value, list) or len(value) != 4:
        return None
    if not all(isinstance(row, list) and len(row) == 4 for row in value):
        return None
    if not all(isinstance(item, (int, float)) for row in value for item in row):
        return None
    return [[float(item) for item in row] for row in value]


def log_spatial(entity: str, payload: Any, mapping: OutputMapping) -> bool:
    matrix = matrix4(payload)
    if matrix is None:
        return False
    translation = [matrix[0][3], matrix[1][3], matrix[2][3]]
    if mapping.kind == "point":
        rr.log(entity, rr.Points3D([translation]))
        return True
    if mapping.kind == "pose":
        rotation = [row[:3] for row in matrix[:3]]
        rr.log(entity, rr.Transform3D(translation=translation, mat3x3=rotation))
        length, width, height = mapping.dimensions
        rr.log(
            f"{entity}/robot",
            rr.Boxes3D(half_sizes=[[length / 2, width / 2, height / 2]]),
        )
        return True
    return False


def log_generic(entity: str, value: Any) -> None:
    if value is None:
        return
    if isinstance(value, bool):
        rr.log(entity, rr.Scalars(float(value)))
    elif isinstance(value, (int, float)) and math.isfinite(float(value)):
        rr.log(entity, rr.Scalars(float(value)))
    elif isinstance(value, str):
        rr.log(entity, rr.TextLog(value))
    elif isinstance(value, dict):
        for key, child in value.items():
            log_generic(f"{entity}/{safe_segment(str(key))}", child)
    elif isinstance(value, list):
        for index, child in enumerate(value):
            log_generic(f"{entity}/{index}", child)
    else:
        rr.log(entity, rr.TextLog(json.dumps(value, separators=(",", ":"))))


def process(stream: TextIO, mappings: dict[str, OutputMapping], limit: int | None) -> int:
    count = 0
    for copperlist in iter_json_values(stream):
        if not isinstance(copperlist, dict) or not isinstance(copperlist.get("msgs"), list):
            raise ValueError("expected a CopperList object containing a msgs array")
        copperlist_id = int(copperlist.get("id", count))
        rr.set_time("copperlist", sequence=copperlist_id)

        for key, message in flatten_messages(copperlist["msgs"]):
            payload = message.get("payload")
            if payload is None:
                continue
            tov = tov_nanos(message.get("tov"))
            if tov is not None:
                rr.set_time("tov", duration=1e-9 * tov)

            mapping = mappings.get(key, OutputMapping(name=f"slot_{key.replace('.', '_')}"))
            entity = f"copper/outputs/{safe_segment(mapping.name)}"
            if not log_spatial(entity, payload, mapping):
                log_generic(f"{entity}/payload", payload)

        count += 1
        if limit is not None and count >= limit:
            break
    return count


def main() -> int:
    args = parse_args()
    mappings, time_series = load_config(args.mapping)
    rr.init("copper-json-rerun")
    if args.save:
        rr.save(args.save)
    else:
        rr.spawn()
    send_blueprint(time_series)

    stream = sys.stdin if str(args.json) == "-" else args.json.open(encoding="utf-8")
    try:
        count = process(stream, mappings, args.limit)
    finally:
        if stream is not sys.stdin:
            stream.close()
    print(f"visualized {count} CopperLists", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
