import unittest
import numpy as np
from betun.dubbing_timing import fit_status, place_complete_speech, trim_edge_silence

class DubbingClockTests(unittest.TestCase):
    def test_overrun_refuses_to_modify_destination(self):
        destination=np.zeros(200,dtype=np.float32)
        with self.assertRaises(ValueError):
            place_complete_speech(destination,np.ones(101),.5,1.5,100)
        self.assertEqual(float(destination.sum()),0.)

    def test_complete_speech_uses_source_clock_and_leaves_padding(self):
        destination=np.zeros(200,dtype=np.float32)
        place_complete_speech(destination,np.ones(90),.5,1.5,100)
        np.testing.assert_array_equal(destination[50:140],np.ones(90))
        self.assertEqual(float(destination[140:].sum()),0.)
        self.assertEqual(len(destination),200)

    def test_duration_policy_rejects_short_and_long(self):
        self.assertEqual(fit_status(5.1,5),'overrun_rewrite_required')
        self.assertEqual(fit_status(3,5),'too_short_rewrite_required')
        self.assertEqual(fit_status(4.7,5),'fits_with_natural_pause_padding')

    def test_outer_trim_retains_internal_pause(self):
        samples=np.concatenate([np.zeros(100),np.ones(100),np.zeros(100),np.ones(100),np.zeros(100)])
        trimmed=trim_edge_silence(samples,sample_rate=1000,pad_seconds=.02)
        self.assertEqual(len(trimmed),340)
        np.testing.assert_array_equal(trimmed[120:220],np.zeros(100))
