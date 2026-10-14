"""Unit tests for the cases no test vector can isolate.

    python3 -m unittest discover verify/python
"""

import os
import tempfile
import unittest

import posum_verify as pv


class Bands(unittest.TestCase):
    def check(self, content, k, n):
        with tempfile.NamedTemporaryFile("wb", delete=False) as f:
            f.write(content.encode("ascii"))
        try:
            pv.verify_bands(f.name, k, n)
        finally:
            os.unlink(f.name)

    # G_2(31): every m <= 30 has a vertex of degree at most 1.
    G2_31 = ("1 - 2 vertex=1 degree=0\n3 - 7 vertex=1 degree=1\n8 - 13 vertex=2 degree=1\n"
             "14 - 16 vertex=8 degree=1\n17 - 19 vertex=16 degree=1\n20 - 30 vertex=18 degree=1\n")

    def test_g2_31_is_valid(self):
        self.check(self.G2_31, 2, 31)

    def test_single_value_written_as_range(self):
        dash = self.G2_31.replace("20 - 30", "20 - 20 vertex=18 degree=1\n21 - 30", 1)
        with self.assertRaisesRegex(pv.Invalid, "not increasing"):
            self.check(dash, 2, 31)

    def test_degree_zero(self):
        self.assertEqual(pv.parse_band("1 - 2 vertex=1 degree=0", 1), (1, 2, 1, 0))
        with self.assertRaises(pv.Invalid):
            pv.parse_band("1 - 2 vertex=1 degree=00", 1)
        with self.assertRaises(pv.Invalid):
            pv.parse_band("1 vertex=0 degree=0", 1)

    def test_trailing_space(self):
        with self.assertRaises(pv.Invalid):
            pv.parse_band("1 - 2 vertex=1 degree=0 ", 1)


class Powers(unittest.TestCase):
    def test_exact(self):
        for k in range(2, 12):
            for b in (2, 3, 10**6 + 3):
                self.assertTrue(pv.is_power(b**k, k))
                self.assertFalse(pv.is_power(b**k + 1, k))
                self.assertFalse(pv.is_power(b**k - 1, k))


if __name__ == "__main__":
    unittest.main()
