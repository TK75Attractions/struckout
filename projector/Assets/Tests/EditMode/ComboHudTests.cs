using System.Reflection;
using NUnit.Framework;
using Struckout.Unity;
using UnityEngine;
using UnityEngine.UI;
using Object = UnityEngine.Object;

namespace Struckout.Tests
{
    public class ComboHudTests
    {
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
