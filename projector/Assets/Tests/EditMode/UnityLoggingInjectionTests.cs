using System;
using System.Text.RegularExpressions;
using Microsoft.Extensions.Logging;
using NUnit.Framework;
using Sentry;
using Sentry.Extensibility;
using Struckout.Application.Interface;
using Struckout.Domain;
using Struckout.Domain.TargetDefinition;
using Struckout.Unity;
using Struckout.Unity.Logging;
using UnityEditor;
using UnityEngine;
using UnityEngine.TestTools;
using VContainer;
using VContainer.Unity;
using Object = UnityEngine.Object;
using Target = Struckout.Domain.TargetDefinition.Target;

namespace Struckout.Tests.Tests.EditMode
{
    public class UnityLoggingInjectionTests
    {
        [Test]
        public void Injects_scene_components_and_supplies_dynamic_target_logger_before_initialization()
        {
            var root = new GameObject("LoggingTest");
            var cameraObject = new GameObject("Camera");
            cameraObject.transform.SetParent(root.transform);
            var camera = cameraObject.AddComponent<FieldCamera>();
            var canvasObject = new GameObject("Canvas", typeof(RectTransform), typeof(Canvas));
            canvasObject.transform.SetParent(root.transform);
            var ui = root.AddComponent<UIService>();
            var fields = new SerializedObject(ui);
            fields.FindProperty("hudCanvas").objectReferenceValue = canvasObject.GetComponent<Canvas>();
            fields.FindProperty("displayFont").objectReferenceValue =
                AssetDatabase.LoadAssetAtPath<Font>("Assets/Fonts/Oxanium-VariableFont_wght.ttf");
            fields.FindProperty("resultFont").objectReferenceValue =
                AssetDatabase.LoadAssetAtPath<Font>("Assets/Fonts/NotoSansJP.ttf");
            fields.FindProperty("compassUI").objectReferenceValue =
                AssetDatabase.LoadAssetAtPath<Transform>("Assets/Prefab/CompassTarget.prefab");
            fields.ApplyModifiedPropertiesWithoutUndo();

            try
            {
                var builder = new ContainerBuilder();
                builder.RegisterInstance<IHub>(DisabledHub.Instance);
                builder.RegisterInstance<IExternalScopeProvider>(new LoggerExternalScopeProvider());
                builder.RegisterInstance(new NetworkSettings { machineId = 3 });
                builder.Register(typeof(ProjectorLogger<>), Lifetime.Singleton).As(typeof(ILogger<>));
                builder.RegisterInstance(new WorldCoordinateTransform());
                builder.RegisterInstance(new UIRoot(root.transform));
                builder.RegisterComponent(ui).As<IUIService>();
                builder.RegisterComponent(camera);
                LogAssert.Expect(LogType.Log, new Regex("\\[Struckout.Unity.FieldCamera\\] \\[machine_id=3\\] \\[Field\\]"));
                using var container = builder.Build();

                var target = new Target(Guid.NewGuid(),
                    new TargetGeometry(new TargetCoordinate(960, 540), TargetType.Compass, 200), TargetOwnership.Single);
                container.Resolve<IUIService>().AddTarget(target);
                var compass = root.GetComponentInChildren<CompassTargetUI>(true);
                Assert.That(compass, Is.Not.Null);
                Assert.That(compass.transform.localScale.x, Is.GreaterThan(0));

                var panel = new GameObject("Calibration").AddComponent<CollisionCalibrationPanel>();
                panel.transform.SetParent(root.transform);
                Assert.DoesNotThrow(() => panel.Construct(new CollisionCoordinateTransform(),
                    new CollisionCoordinateTransform(), container.Resolve<IUIService>(), "unused.json",
                    container.Resolve<ILogger<CollisionCalibrationPanel>>()));
                Assert.That(camera.GetComponent<Camera>().orthographic, Is.True);
            }
            finally
            {
                Object.DestroyImmediate(root);
            }
        }

        [Test]
        public void Dynamic_target_requires_logger_before_initialization()
        {
            var targetObject = new GameObject("Target");
            try
            {
                var component = targetObject.AddComponent<CompassTargetUI>();
                var target = new Target(Guid.NewGuid(),
                    new TargetGeometry(new TargetCoordinate(0, 0), TargetType.Compass, 200), TargetOwnership.Single);
                Assert.Throws<InvalidOperationException>(() => component.Initialize(target, new WorldCoordinateTransform()));
            }
            finally
            {
                Object.DestroyImmediate(targetObject);
            }
        }
    }
}
