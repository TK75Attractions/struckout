import sys
import unittest
from datetime import datetime
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from run_pc1 import (CameraLink, PhoneState, camera_log_state, camera_status_line, camera_status_text,
                     parse_device_state)


class DeviceStateTests(unittest.TestCase):
    def test_locked(self) -> None:
        self.assertEqual(parse_device_state("mShowingLockscreen=true", "mWakefulness=Awake"), PhoneState.LOCKED)

    def test_unlocked(self) -> None:
        self.assertEqual(parse_device_state("mShowingLockscreen=false", "mWakefulness=Awake"), PhoneState.UNLOCKED)

    def test_asleep(self) -> None:
        self.assertEqual(parse_device_state("", "mWakefulness=Asleep"), PhoneState.ASLEEP)

    def test_unknown(self) -> None:
        self.assertEqual(parse_device_state("new Android window output", "power state unavailable"),
                         PhoneState.UNKNOWN)

    def test_guidance_only_for_known_locked_or_disconnected(self) -> None:
        self.assertIn("画面のロックを解除", camera_status_line("right", "serial", PhoneState.LOCKED))
        self.assertIsNone(camera_status_line("right", "serial", PhoneState.UNKNOWN))
        self.assertIn("USB デバッグを許可", camera_status_line("right", "serial", PhoneState.DISCONNECTED))


class CameraLogStateTests(unittest.TestCase):
    def test_connect_stats_disconnect_reconnect(self) -> None:
        links: dict[int, CameraLink] = {}
        # The log's own timestamps are UTC and must not be shown as the last received time.
        camera_log_state("2026-10-30T05:03:22.123Z INFO ball_tracker::detection_input camera connected camera_id=1",
                         links, datetime(2026, 10, 30, 14, 3, 22))
        camera_log_state('2026-10-30T05:03:23.000Z TRACE ball_tracker::video video analysis camera_id=1 fps="120.1" '
                         "delay_ms_p50=20 delay_ms_p95=40 max_delay_ms=50", links, datetime(2026, 10, 30, 14, 3, 23))
        stats = ("1", "120.1", "20", "40", "50")
        self.assertIn("120.1 fps", camera_status_text(1, stats, links[1]))
        self.assertIn("最終受信 14:03:23", camera_status_text(1, stats, links[1]))
        camera_log_state("2026-10-30T05:03:24.000Z INFO ball_tracker::detection_input camera disconnected "
                         "camera_id=1 error=ConnectionReset", links, datetime(2026, 10, 30, 14, 3, 24))
        self.assertEqual(camera_status_text(1, stats, links[1]), "camera_id 1 切断中 (最終受信 14:03:23)")
        camera_log_state("2026-10-30T05:03:25.000Z INFO ball_tracker::detection_input camera connected camera_id=1",
                         links, datetime(2026, 10, 30, 14, 3, 25))
        self.assertIn("120.1 fps", camera_status_text(1, stats, links[1]))

    def test_unrelated_line_is_ignored(self) -> None:
        links: dict[int, CameraLink] = {}
        camera_log_state("INFO ball_tracker detected collision", links, datetime(2026, 10, 30))
        self.assertEqual(links, {})


if __name__ == "__main__":
    unittest.main()
