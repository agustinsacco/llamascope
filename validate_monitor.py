import unittest
import time
import json
from unittest.mock import patch, MagicMock
from monitor import TelemetryEngine

class TestInferenceEngine(unittest.TestCase):
    def setUp(self):
        self.engine = TelemetryEngine()

    @patch('monitor.subprocess.run')
    def test_gpu_data_parsing(self, mock_run):
        # Mock rocm-smi output
        mock_gpu = MagicMock(returncode=0)
        mock_gpu.stdout = json.dumps({"card0": {"GPU use (%)": "85", "Temperature (Sensor edge) (C)": "65", "Current Socket Graphics Package Power (W)": "120"}})
        
        # Mock free -b output (128GB system)
        mock_free = MagicMock(returncode=0)
        mock_free.stdout = "               total        used        free      shared  buff/cache   available\nMem:     132570296320 64424509440 2163511296     3686400 27338461184 28223115264\n"
        
        mock_run.side_effect = [mock_gpu, mock_free]

        data = self.engine.get_gpu_data()
        self.assertIsNotNone(data)
        self.assertEqual(data['usage'], 85.0)
        self.assertAlmostEqual(data['mem_total'], 123.46, places=1)

    @patch('requests.Session.get')
    def test_inference_tps_logic(self, mock_get):
        # Mock llama responses
        mock_get.side_effect = [
            MagicMock(json=lambda: [{"id": 0, "is_processing": True, "id_task": 1, "n_ctx": 1024, "next_token": [{"n_decoded": 100, "n_remain": 900}]}]),
            MagicMock(json=lambda: {"queue_size": 0})
        ]

        # First poll at time T
        with patch('time.time', return_value=100.0):
            self.engine.get_inference_data()
        
        # Second poll at time T+1.0 with 50 more tokens
        with patch('time.time', return_value=101.0):
            mock_get.side_effect = [
                MagicMock(json=lambda: [{"id": 0, "is_processing": True, "id_task": 1, "n_ctx": 1024, "next_token": [{"n_decoded": 150, "n_remain": 850}]}]),
                MagicMock(json=lambda: {"queue_size": 0})
            ]
            data = self.engine.get_inference_data()
            self.assertAlmostEqual(data['slots'][0]['tps'], 50.0, places=1)

if __name__ == "__main__":
    unittest.main()
