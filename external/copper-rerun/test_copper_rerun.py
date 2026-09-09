import io
import unittest

from copper_rerun import iter_json_values, load_config, matrix4, tov_nanos, transform_parts


class CopperRerunTests(unittest.TestCase):
    def test_streams_consecutive_json_objects(self):
        values = list(iter_json_values(io.StringIO('{"id": 1}\n{"id": 2}'), chunk_size=4))
        self.assertEqual(values, [{"id": 1}, {"id": 2}])

    def test_ignores_copper_exporter_preamble(self):
        stream = io.StringIO('Extracting copperlists with format: json\n{"id": 1}')
        self.assertEqual(list(iter_json_values(stream, chunk_size=8)), [{"id": 1}])

    def test_parses_copper_tov_variants(self):
        self.assertEqual(tov_nanos({"Time": 42}), 42)
        self.assertEqual(tov_nanos({"Range": {"start": 7, "end": 9}}), 7)
        self.assertIsNone(tov_nanos("None"))

    def test_recognizes_only_numeric_4x4_matrices(self):
        identity = [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]]
        self.assertEqual(matrix4(identity), [[float(v) for v in row] for row in identity])
        self.assertIsNone(matrix4([[1, 0], [0, 1]]))

    def test_extracts_column_major_transform_parts(self):
        matrix = [
            [0.0, 1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [2.0, 3.0, 4.0, 1.0],
        ]
        translation, rotation = transform_parts(matrix)
        self.assertEqual(translation, [2.0, 3.0, 4.0])
        self.assertEqual(rotation, [column[:3] for column in matrix[:3]])

    def test_loads_selected_time_series(self):
        mappings, time_series = load_config(
            __import__("pathlib").Path(__file__).parent / "testdata" / "sample.toml"
        )
        self.assertEqual(mappings["0"].name, "drive")
        self.assertEqual(time_series, ["copper/outputs/drive/payload/speed"])


if __name__ == "__main__":
    unittest.main()
