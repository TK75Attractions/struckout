using System;
using System.Reflection;
using NUnit.Framework;
using Struckout.Domain;
using Struckout.Domain.TargetDefinition;
using Struckout.Unity;
using UnityEditor;
using UnityEngine;
using Object = UnityEngine.Object;

namespace Struckout.Tests
{
    public class PocketWatchUITests
    {
        [Test]
        public void Supplied_layers_align_with_collision_dial_and_replacement_keeps_game_progress()
        {
            var face = AssetDatabase.LoadAssetAtPath<Sprite>("Assets/Art/StruckOut/pocket_watch_face.png");
            var hand = AssetDatabase.LoadAssetAtPath<Sprite>("Assets/Art/StruckOut/pocket_watch_hand.png");
            Assert.That(face, Is.Not.Null);
            Assert.That(hand, Is.Not.Null);
            var clock = new GameClock();
            clock.Start(TimeSpan.FromSeconds(90), Time.realtimeSinceStartupAsDouble - 45);
            var world = new WorldCoordinateTransform();
            var target = new Target(Guid.NewGuid(),
                new TargetGeometry(new TargetCoordinate(960, 540), TargetType.PocketWatch, 200), TargetOwnership.Single);
            var first = new GameObject("FirstWatch").AddComponent<PocketWatchUI>();
            var replacement = new GameObject("ReplacementWatch").AddComponent<PocketWatchUI>();
            try
            {
                foreach (var watch in new[] { first, replacement })
                {
                    watch.Configure(face, hand, clock);
                    watch.Initialize(target, world);
                    var faceLayer = watch.transform.Find("Face").GetComponent<SpriteRenderer>();
                    var handLayer = watch.transform.Find("Hand");
                    Assert.That(watch.transform.position, Is.EqualTo(Vector3.zero));
                    Assert.That(faceLayer.bounds.size.x * 930 / 1254, Is.EqualTo(2).Within(0.001));
                    Assert.That(handLayer.position, Is.EqualTo(watch.transform.position));
                    Assert.That(Mathf.DeltaAngle(handLayer.localEulerAngles.z, 180), Is.EqualTo(0).Within(1));
                }
                clock.Correct(TimeSpan.FromSeconds(67.5), Time.realtimeSinceStartupAsDouble);
                typeof(PocketWatchUI).GetMethod("Update", BindingFlags.Instance | BindingFlags.NonPublic)
                    ?.Invoke(replacement, null);
                Assert.That(Mathf.DeltaAngle(replacement.transform.Find("Hand").localEulerAngles.z, 270),
                    Is.EqualTo(0).Within(1));
            }
            finally
            {
                Object.DestroyImmediate(first.gameObject);
                Object.DestroyImmediate(replacement.gameObject);
            }
        }
    }
}
