using System;
using System.Reflection;
using NUnit.Framework;
using Struckout.Domain;
using Struckout.Unity;
using UnityEngine;
using UnityEngine.UI;
using Object = UnityEngine.Object;

namespace Struckout.Tests
{
    public class ComboHudTests
    {
        [Test]
        public void Added_duration_is_shown_in_the_remaining_time_label_for_score_popup_duration()
        {
            var root = new GameObject("UI");
            var label = new GameObject("RemainingTime", typeof(RectTransform), typeof(Text));
            try
            {
                label.transform.SetParent(root.transform, false);
                var service = root.AddComponent<UIService>();
                var text = label.GetComponent<Text>();
                typeof(UIService).GetField("_remainingTimeText", BindingFlags.Instance | BindingFlags.NonPublic)
                    ?.SetValue(service, text);
                typeof(UIService).GetField("_screen", BindingFlags.Instance | BindingFlags.NonPublic)
                    ?.SetValue(service, ProjectorScreen.Playing);
                service.StartGameClock(TimeSpan.FromSeconds(90));
                service.SetRemainingTime(TimeSpan.FromSeconds(70));
                service.SetTotalDuration(TimeSpan.FromSeconds(100));
                Assert.That(text.text, Is.EqualTo("TIME  01:10  +10s"));
                service.SetRemainingTime(TimeSpan.FromSeconds(80));
                Assert.That(text.text, Is.EqualTo("TIME  01:20  +10s"));
                typeof(UIService).GetField("_timeBonusUntil", BindingFlags.Instance | BindingFlags.NonPublic)
                    ?.SetValue(service, Time.realtimeSinceStartupAsDouble - 1);
                typeof(UIService).GetMethod("Update", BindingFlags.Instance | BindingFlags.NonPublic)
                    ?.Invoke(service, null);
                Assert.That(text.text, Is.EqualTo("TIME  01:20"));
            }
            finally { Object.DestroyImmediate(root); }
        }

        [TestCase(0u, false)]
        [TestCase(1u, false)]
        [TestCase(2u, true)]
        [TestCase(5u, true)]
        public void Combo_label_is_visible_only_from_two_hits(uint combo, bool visible)
        {
            var root = new GameObject("UI");
            var panel = new GameObject("Combo", typeof(RectTransform));
            var label = new GameObject("Label", typeof(RectTransform), typeof(Text));
            try
            {
                panel.transform.SetParent(root.transform, false);
                label.transform.SetParent(panel.transform, false);
                var service = root.AddComponent<UIService>();
                var text = label.GetComponent<Text>();
                typeof(UIService).GetField("_comboText", BindingFlags.Instance | BindingFlags.NonPublic)
                    ?.SetValue(service, text);
                service.SetCombo(combo);
                Assert.That(panel.activeSelf, Is.EqualTo(visible));
                Assert.That(text.text, Is.EqualTo($"{combo} COMBO"));
                service.SetCombo(0);
                Assert.That(panel.activeSelf, Is.False);
            }
            finally { Object.DestroyImmediate(root); }
        }
    }
}
